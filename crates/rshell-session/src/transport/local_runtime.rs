use std::{io::Write, thread::JoinHandle, time::Duration};

use portable_pty::{Child, MasterPty, PtySize};
use rshell_core::{ExitStatus, SessionFailure, TerminalSize};
use tokio::sync::mpsc;

use crate::{TransportError, TransportEvent};

use super::local_reader::ReaderEvent;

const CHILD_POLL_INTERVAL: Duration = Duration::from_millis(10);
const SHUTDOWN_GRACE: Duration = Duration::from_millis(250);
const FORCE_EXIT_GRACE: Duration = Duration::from_millis(250);
const READER_JOIN_GRACE: Duration = Duration::from_millis(500);

pub(super) struct LocalRuntime {
    master: Option<Box<dyn MasterPty>>,
    #[cfg(unix)]
    process_group: Option<i32>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn Child + Send + Sync>>,
    reader_rx: mpsc::Receiver<ReaderEvent>,
    reader_thread: Option<JoinHandle<()>>,
    pending_exit: Option<ExitStatus>,
    reader_eof: bool,
    exit_emitted: bool,
    shut_down: bool,
}

impl LocalRuntime {
    pub(super) fn new(
        master: Box<dyn MasterPty>,
        writer: Box<dyn Write + Send>,
        child: Box<dyn Child + Send + Sync>,
        reader_rx: mpsc::Receiver<ReaderEvent>,
        reader_thread: JoinHandle<()>,
    ) -> Self {
        #[cfg(unix)]
        let process_group = master.process_group_leader();
        Self {
            master: Some(master),
            #[cfg(unix)]
            process_group,
            writer: Some(writer),
            child: Some(child),
            reader_rx,
            reader_thread: Some(reader_thread),
            pending_exit: None,
            reader_eof: false,
            exit_emitted: false,
            shut_down: false,
        }
    }

    pub(super) fn process_id(&self) -> Option<u32> {
        self.child.as_ref().and_then(|child| child.process_id())
    }

    pub(super) async fn next_event(&mut self) -> Result<TransportEvent, TransportError> {
        if self.exit_emitted || self.shut_down {
            return Err(pty_error());
        }
        loop {
            match self.reader_rx.try_recv() {
                Ok(event) => {
                    if let Some(event) = self.handle_reader_event(event)? {
                        return Ok(event);
                    }
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => self.reader_eof = true,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }

            self.poll_child()?;
            if self.reader_eof
                && let Some(status) = self.pending_exit.take()
            {
                self.join_reader()?;
                self.exit_emitted = true;
                return Ok(TransportEvent::Exit(status));
            }

            if self.reader_eof {
                tokio::time::sleep(CHILD_POLL_INTERVAL).await;
            } else {
                match tokio::time::timeout(CHILD_POLL_INTERVAL, self.reader_rx.recv()).await {
                    Ok(Some(event)) => {
                        if let Some(event) = self.handle_reader_event(event)? {
                            return Ok(event);
                        }
                    }
                    Ok(None) => self.reader_eof = true,
                    Err(_) => {}
                }
            }
        }
    }

    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let writer = self.writer.as_mut().ok_or_else(pty_error)?;
        writer.write_all(bytes).map_err(|_| pty_error())?;
        writer.flush().map_err(|_| pty_error())
    }

    pub(super) fn resize(&mut self, size: TerminalSize) -> Result<(), TransportError> {
        let size = checked_pty_size(size)?;
        self.master
            .as_ref()
            .ok_or_else(pty_error)?
            .resize(size)
            .map_err(|_| pty_error())
    }

    pub(super) async fn shutdown(&mut self) -> Result<(), TransportError> {
        if self.shut_down {
            return Ok(());
        }
        self.reader_rx.close();
        self.writer.take();

        let deadline = tokio::time::Instant::now() + SHUTDOWN_GRACE;
        let mut exited = self.child_has_exited().unwrap_or(false);
        while !exited && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(CHILD_POLL_INTERVAL).await;
            match self.child_has_exited() {
                Ok(child_exited) => exited = child_exited,
                Err(_) => break,
            }
        }
        if !exited {
            #[cfg(unix)]
            {
                let _ = self.signal_process_group(libc::SIGHUP);
            }
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
            }
            let deadline = tokio::time::Instant::now() + FORCE_EXIT_GRACE;
            while !exited && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(CHILD_POLL_INTERVAL).await;
                match self.child_has_exited() {
                    Ok(child_exited) => exited = child_exited,
                    Err(_) => break,
                }
            }
        }

        #[cfg(unix)]
        {
            let _ = self.signal_process_group(libc::SIGKILL);
        }

        self.master.take();
        let reader_joined = self.join_reader_bounded().await.is_ok();
        let child_exited = exited || self.child_has_exited().unwrap_or(false);
        self.child.take();
        self.shut_down = true;
        if child_exited && reader_joined {
            Ok(())
        } else {
            Err(pty_error())
        }
    }

    fn handle_reader_event(
        &mut self,
        event: ReaderEvent,
    ) -> Result<Option<TransportEvent>, TransportError> {
        match event {
            ReaderEvent::Output(bytes) => Ok(Some(TransportEvent::Output(bytes))),
            ReaderEvent::Eof => {
                self.reader_eof = true;
                Ok(None)
            }
            ReaderEvent::Failure => Err(pty_error()),
        }
    }

    fn poll_child(&mut self) -> Result<(), TransportError> {
        if self.pending_exit.is_some() {
            return Ok(());
        }
        let Some(child) = self.child.as_mut() else {
            return Err(pty_error());
        };
        if let Some(status) = child.try_wait().map_err(|_| pty_error())? {
            self.pending_exit = Some(exit_status(status));
            self.writer.take();
            self.master.take();
        }
        Ok(())
    }

    fn child_has_exited(&mut self) -> Result<bool, TransportError> {
        let Some(child) = self.child.as_mut() else {
            return Ok(true);
        };
        child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(|_| pty_error())
    }

    fn join_reader(&mut self) -> Result<(), TransportError> {
        if let Some(thread) = self.reader_thread.take() {
            thread.join().map_err(|_| pty_error())?;
        }
        Ok(())
    }

    async fn join_reader_bounded(&mut self) -> Result<(), TransportError> {
        let Some(thread) = self.reader_thread.take() else {
            return Ok(());
        };
        let deadline = tokio::time::Instant::now() + READER_JOIN_GRACE;
        while !thread.is_finished() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(CHILD_POLL_INTERVAL).await;
        }
        if !thread.is_finished() {
            return Err(pty_error());
        }
        thread.join().map_err(|_| pty_error())
    }

    #[cfg(unix)]
    fn signal_process_group(&self, signal: libc::c_int) -> Result<(), TransportError> {
        let Some(process_group) = self.process_group.filter(|group| *group > 1) else {
            return Ok(());
        };
        let result = unsafe { libc::kill(-process_group, signal) };
        if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(pty_error())
        }
    }
}

impl Drop for LocalRuntime {
    fn drop(&mut self) {
        self.reader_rx.close();
        self.writer.take();
        #[cfg(unix)]
        let _ = self.signal_process_group(libc::SIGHUP);
        if let Some(child) = self.child.as_mut()
            && !matches!(child.try_wait(), Ok(Some(_)))
        {
            let _ = child.kill();
            let deadline = std::time::Instant::now() + FORCE_EXIT_GRACE;
            while std::time::Instant::now() < deadline {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(CHILD_POLL_INTERVAL);
            }
        }
        #[cfg(unix)]
        let _ = self.signal_process_group(libc::SIGKILL);
        self.master.take();
        if let Some(thread) = self.reader_thread.take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
    }
}

pub(super) fn checked_pty_size(size: TerminalSize) -> Result<PtySize, TransportError> {
    if size.cols == 0 || size.rows == 0 {
        return Err(TransportError::new(SessionFailure::Validation));
    }
    #[cfg(windows)]
    if size.cols > i16::MAX as u16 || size.rows > i16::MAX as u16 {
        return Err(TransportError::new(SessionFailure::Validation));
    }
    Ok(PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: u16::try_from(size.pixel_width)
            .map_err(|_| TransportError::new(SessionFailure::Validation))?,
        pixel_height: u16::try_from(size.pixel_height)
            .map_err(|_| TransportError::new(SessionFailure::Validation))?,
    })
}

fn exit_status(status: portable_pty::ExitStatus) -> ExitStatus {
    ExitStatus {
        code: i32::try_from(status.exit_code()).ok(),
        success: status.success(),
    }
}

fn pty_error() -> TransportError {
    TransportError::new(SessionFailure::Pty)
}
