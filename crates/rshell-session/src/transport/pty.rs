use portable_pty::{Child, CommandBuilder, native_pty_system};
use rshell_core::{SessionFailure, TerminalSize};

use crate::TransportError;

use super::{
    local_reader::spawn_reader,
    local_runtime::{LocalRuntime, checked_pty_size},
};

/// Starts a command in a PTY and hands its lifecycle to the shared runtime.
pub(super) fn spawn_pty_runtime(
    command: CommandBuilder,
    size: TerminalSize,
    spawn_failure: SessionFailure,
) -> Result<LocalRuntime, TransportError> {
    spawn_pty_runtime_impl(
        command,
        size,
        spawn_failure,
        #[cfg(windows)]
        None,
    )
}

fn spawn_pty_runtime_impl(
    command: CommandBuilder,
    size: TerminalSize,
    spawn_failure: SessionFailure,
    #[cfg(windows)] injected_failure: Option<InjectedContainmentFailure>,
) -> Result<LocalRuntime, TransportError> {
    let pty_size = checked_pty_size(size)?;
    #[cfg(windows)]
    let _ = &spawn_failure;
    #[cfg(windows)]
    if injected_failure == Some(InjectedContainmentFailure::JobCreation) {
        return Err(TransportError::new(SessionFailure::Pty));
    }
    #[cfg(windows)]
    let mut process_job = rshell_platform::WindowsProcessJob::new()
        .map_err(|_| TransportError::new(SessionFailure::Pty))?;
    #[cfg(windows)]
    if injected_failure == Some(InjectedContainmentFailure::JobConfiguration) {
        let _ = process_job.terminate();
        return Err(TransportError::new(SessionFailure::Pty));
    }
    let pair = native_pty_system()
        .openpty(pty_size)
        .map_err(|_| TransportError::new(SessionFailure::Pty))?;
    #[cfg(windows)]
    if matches!(
        injected_failure,
        Some(InjectedContainmentFailure::AttributeUpdate)
    ) {
        let _ = process_job.terminate();
        return Err(TransportError::new(SessionFailure::Pty));
    }
    #[cfg(windows)]
    let command = if injected_failure == Some(InjectedContainmentFailure::CreateProcess) {
        CommandBuilder::new(std::env::temp_dir().join(format!(
            "rshell-missing-task4-create-process-{}.exe",
            std::process::id()
        )))
    } else {
        command
    };
    #[cfg(windows)]
    let spawned = pair
        .slave
        .spawn_command_in_job(command, process_job.as_borrowed_handle());
    #[cfg(not(windows))]
    let spawned = pair.slave.spawn_command(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(_) => {
            #[cfg(windows)]
            {
                let _ = process_job.terminate();
                return Err(TransportError::new(SessionFailure::Pty));
            }
            #[cfg(not(windows))]
            return Err(TransportError::new(spawn_failure));
        }
    };
    drop(pair.slave);

    let reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => {
            return Err(cleanup_failed_launch(
                &mut *child,
                #[cfg(windows)]
                &mut process_job,
            ));
        }
    };
    let writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => {
            return Err(cleanup_failed_launch(
                &mut *child,
                #[cfg(windows)]
                &mut process_job,
            ));
        }
    };
    let (reader_rx, reader_thread) = match spawn_reader(reader) {
        Ok(reader) => reader,
        Err(_) => {
            return Err(cleanup_failed_launch(
                &mut *child,
                #[cfg(windows)]
                &mut process_job,
            ));
        }
    };
    Ok(LocalRuntime::new(
        pair.master,
        writer,
        child,
        reader_rx,
        reader_thread,
        #[cfg(windows)]
        process_job,
    ))
}

fn cleanup_failed_launch(
    child: &mut dyn Child,
    #[cfg(windows)] process_job: &mut rshell_platform::WindowsProcessJob,
) -> TransportError {
    #[cfg(windows)]
    let _ = process_job.terminate();
    let _ = child.kill();
    let _ = child.wait();
    TransportError::new(SessionFailure::Pty)
}

#[cfg(windows)]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InjectedContainmentFailure {
    JobCreation,
    JobConfiguration,
    AttributeUpdate,
    CreateProcess,
}

#[cfg(all(test, windows))]
fn spawn_pty_runtime_with_failure(
    command: CommandBuilder,
    size: TerminalSize,
    failure: InjectedContainmentFailure,
) -> Result<LocalRuntime, TransportError> {
    spawn_pty_runtime_impl(command, size, SessionFailure::Pty, Some(failure))
}

#[cfg(all(test, windows))]
mod tests {
    use super::{InjectedContainmentFailure, spawn_pty_runtime_with_failure};
    use portable_pty::CommandBuilder;
    use rshell_core::{SessionFailure, TerminalSize};

    #[test]
    fn creation_time_containment_failure_returns_no_runtime() {
        for failure in [
            InjectedContainmentFailure::JobCreation,
            InjectedContainmentFailure::JobConfiguration,
            InjectedContainmentFailure::AttributeUpdate,
            InjectedContainmentFailure::CreateProcess,
        ] {
            let result = spawn_pty_runtime_with_failure(
                CommandBuilder::new("cmd.exe"),
                TerminalSize {
                    cols: 80,
                    rows: 24,
                    pixel_width: 800,
                    pixel_height: 480,
                    dpi: 96,
                },
                failure,
            );
            let error = match result {
                Ok(_) => panic!("injected failure must publish no runtime or child PID"),
                Err(error) => error,
            };
            assert_eq!(error.failure(), SessionFailure::Pty);
        }
    }
}
