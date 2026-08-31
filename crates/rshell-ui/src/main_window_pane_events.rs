use rshell_core::UiCommand;

use crate::{ConnectionEditorMsg, MainWindow, PaneHostOutput, main_window_commands::CommandSource};

impl MainWindow {
    pub(crate) fn handle_pane_host(&mut self, output: PaneHostOutput) {
        match output {
            PaneHostOutput::Command(command) => {
                super::geometry::observe(self.startup_probe.as_ref(), command.as_ref());
                if matches!(command.as_ref(), UiCommand::Session { .. }) {
                    self.smoke_state.terminal_commands =
                        self.smoke_state.terminal_commands.saturating_add(1);
                }
                self.observe_smoke_terminal_command(&command);
                self.dispatch(*command, CommandSource::PaneHost);
            }
            PaneHostOutput::EditConnection(connection) => {
                if let Some(profile) = self.view_model.catalog.connections.get(&connection) {
                    self.open_editor(ConnectionEditorMsg::OpenEdit(Box::new(profile.clone())));
                }
            }
            PaneHostOutput::Error(message) => {
                self.status = message.into();
                self.fail_smoke("pane_handler_rejected");
            }
            PaneHostOutput::ActiveTab(tab) => self.smoke_state.active_tab = Some(tab),
            PaneHostOutput::RenderedSession(session) => {
                if self.smoke_state.pane_host_geometry_session != session {
                    self.smoke_state.pane_host_geometry_session = None;
                }
                self.smoke_state.pane_host_session = session;
            }
            PaneHostOutput::GeometryReady(session) => {
                self.smoke_state.pane_host_geometry_session = Some(session);
            }
            PaneHostOutput::ClipboardWritten { bytes } => {
                self.smoke_state.clipboard_writes =
                    self.smoke_state.clipboard_writes.saturating_add(1);
                self.smoke_state.clipboard_bytes = Some(bytes);
                self.observe_smoke_clipboard_write(bytes);
            }
        }
    }
}
