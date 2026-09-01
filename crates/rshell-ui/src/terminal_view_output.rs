use relm4::ComponentSender;
use rshell_core::UiCommand;

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
        Err(value) => error(value, sender),
    }
}

pub(crate) fn geometry(
    result: Result<Option<UiCommand>, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    optional(result, sender);
}

pub(crate) fn result(
    result: Result<UiCommand, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(value) => {
            let _ = command(value, sender);
        }
        Err(value) => error(value, sender),
    }
}

pub(crate) fn command(command: UiCommand, sender: &ComponentSender<TerminalView>) -> bool {
    sender
        .output(TerminalViewOutput::Command(Box::new(command)))
        .is_ok()
}

fn error(error: TerminalViewError, sender: &ComponentSender<TerminalView>) {
    let _ = sender.output(TerminalViewOutput::Error(error));
}
