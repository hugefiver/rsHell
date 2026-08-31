use gtk::gdk;
use relm4::ComponentSender;
use rshell_core::{ConnectionId, PaneId, UiCommand};
use rshell_platform::ClipboardPolicy;

use crate::{PaneAction, PaneHost, PaneHostModel, PaneHostOutput};

pub(crate) fn connect_active(
    model: &PaneHostModel,
    connection: ConnectionId,
    sender: &ComponentSender<PaneHost>,
) {
    let Some(tab) = model.active_tab() else {
        let _ = sender.output(PaneHostOutput::Error("no active tab"));
        return;
    };
    let Some(pane) = model.active_pane(tab) else {
        let _ = sender.output(PaneHostOutput::Error("no active pane"));
        return;
    };
    let _ = sender.output(PaneHostOutput::Command(Box::new(UiCommand::Connect {
        pane,
        connection,
    })));
}

pub(crate) fn handle_action(
    model: &PaneHostModel,
    clipboard: &gdk::Clipboard,
    pane_id: PaneId,
    action: PaneAction,
    sender: &ComponentSender<PaneHost>,
) {
    let Some(pane) = model.pane(pane_id) else {
        return;
    };
    match action {
        PaneAction::EditConnection => {
            if let Some(connection) = pane.connection_id() {
                let _ = sender.output(PaneHostOutput::EditConnection(connection));
            }
        }
        PaneAction::CopyDiagnostics => {
            let Some(diagnostics) = pane.diagnostics() else {
                return;
            };
            match ClipboardPolicy::normalize_text(&diagnostics) {
                Ok(diagnostics) => clipboard.set_text(&diagnostics),
                Err(_) => {
                    let _ = sender.output(PaneHostOutput::Error("diagnostics copy was rejected"));
                }
            }
        }
        other => {
            if let Some(command) = other.command(pane_id, pane.session()) {
                let _ = sender.output(PaneHostOutput::Command(Box::new(command)));
            }
        }
    }
}
