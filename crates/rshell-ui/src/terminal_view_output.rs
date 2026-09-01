use relm4::ComponentSender;
use rshell_core::{SessionUiCommand, UiCommand};

use super::{TerminalView, TerminalViewOutput};
use crate::TerminalViewError;

pub(crate) fn optional(
    result: Result<Option<UiCommand>, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(Some(value)) => {
            let _ = command(value, sender);
        }
        Ok(None) => {}
        Err(value) => {
            let _ = error(value, sender);
        }
    }
}

pub(crate) fn geometry(
    result: Result<Option<UiCommand>, TerminalViewError>,
    startup_probe: Option<&crate::StartupProbe>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(Some(value)) => {
            let size = match &value {
                UiCommand::Session {
                    command: SessionUiCommand::Resize(size),
                    ..
                } => Some(*size),
                _ => None,
            };
            if command(value, sender)
                && let (Some(probe), Some(size)) = (startup_probe, size)
            {
                probe.observe_terminal_geometry(size);
            }
        }
        Ok(None) => {}
        Err(value) => {
            let _ = error(value, sender);
        }
    }
}

pub(crate) fn result(
    result: Result<UiCommand, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(value) => {
            let _ = command(value, sender);
        }
        Err(value) => {
            let _ = error(value, sender);
        }
    }
}

pub(crate) fn command(command: UiCommand, sender: &ComponentSender<TerminalView>) -> bool {
    sender
        .output(TerminalViewOutput::Command(Box::new(command)))
        .is_ok()
}

fn error(error: TerminalViewError, sender: &ComponentSender<TerminalView>) -> bool {
    sender.output(TerminalViewOutput::Error(error)).is_ok()
}
