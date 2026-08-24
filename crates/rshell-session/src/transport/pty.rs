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
    let pty_size = checked_pty_size(size)?;
    let pair = native_pty_system()
        .openpty(pty_size)
        .map_err(|_| TransportError::new(SessionFailure::Pty))?;
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|_| TransportError::new(spawn_failure))?;
    drop(pair.slave);

    let reader = match pair.master.try_clone_reader() {
        Ok(reader) => reader,
        Err(_) => return Err(cleanup_failed_launch(&mut *child)),
    };
    let writer = match pair.master.take_writer() {
        Ok(writer) => writer,
        Err(_) => return Err(cleanup_failed_launch(&mut *child)),
    };
    let (reader_rx, reader_thread) = match spawn_reader(reader) {
        Ok(reader) => reader,
        Err(_) => return Err(cleanup_failed_launch(&mut *child)),
    };
    Ok(LocalRuntime::new(
        pair.master,
        writer,
        child,
        reader_rx,
        reader_thread,
    ))
}

fn cleanup_failed_launch(child: &mut dyn Child) -> TransportError {
    let _ = child.kill();
    let _ = child.wait();
    TransportError::new(SessionFailure::Pty)
}
