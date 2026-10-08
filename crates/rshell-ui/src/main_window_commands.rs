use relm4::ComponentController;
use rshell_core::{NewLocalTabSubmitError, UiCommand, UiPortError};

use crate::{
    ConnectionEditorMsg, ConnectionSidebarMsg, ImportDialogMsg, InteractionDialogMsg, MainWindow,
    PaneHostMsg, SessionTabBarMsg, SettingsWindowMsg, command_port::dispatch,
};

#[derive(Clone, Copy)]
pub(crate) enum CommandSource {
    Sidebar,
    Editor,
    TabBar,
    PaneHost,
    Settings,
    Import,
    Interaction,
}

impl MainWindow {
    pub(crate) fn dispatch(&mut self, command: UiCommand, source: CommandSource) -> bool {
        let result = if matches!(command, UiCommand::NewLocalTab) {
            let preparation = self.pane_host.model().prepare_new_local_focus();
            let intent = preparation.begin(self.modal.open_kind().is_none());
            match self.command_port.try_new_local_tab_with_completion() {
                Ok(receipt) => {
                    intent.receive(receipt);
                    Ok(())
                }
                Err(NewLocalTabSubmitError::Unsupported) => {
                    intent.cancel();
                    dispatch(&self.command_port, command)
                }
                Err(NewLocalTabSubmitError::Rejected(error)) => {
                    intent.cancel();
                    Err(error)
                }
            }
        } else {
            dispatch(&self.command_port, command)
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                self.cancel_terminal_focus();
                self.status = error.to_string();
                self.reject(source, error);
                if !self
                    .smoke
                    .as_ref()
                    .is_some_and(crate::smoke_driver_state::SmokeDriver::shutdown_sent)
                {
                    self.fail_smoke("command_rejected");
                }
                false
            }
        }
    }

    pub(crate) fn cancel_terminal_focus(&self) {
        let focus = self.pane_host.model().focus_handle();
        focus.cancel();
    }

    fn reject(&self, source: CommandSource, error: UiPortError) {
        match source {
            CommandSource::Sidebar => {
                self.send_sidebar(ConnectionSidebarMsg::CommandRejected(error))
            }
            CommandSource::Editor => self.send_editor(ConnectionEditorMsg::CommandRejected(error)),
            CommandSource::TabBar => self.send_tab(SessionTabBarMsg::CommandRejected(error)),
            CommandSource::PaneHost => self.send_pane(PaneHostMsg::CommandRejected(error)),
            CommandSource::Settings => {
                self.send_settings(SettingsWindowMsg::CommandRejected(error))
            }
            CommandSource::Import => self.send_import(ImportDialogMsg::CommandRejected(error)),
            CommandSource::Interaction => {
                if let Some((_, interaction)) = self.pending_interaction {
                    self.send_interaction(InteractionDialogMsg::CommandRejected(
                        interaction,
                        error,
                    ));
                }
            }
        }
    }

    pub(crate) fn send_sidebar(&self, message: ConnectionSidebarMsg) {
        let _ = self.sidebar.sender().send(message);
    }

    pub(crate) fn send_editor(&self, message: ConnectionEditorMsg) {
        let _ = self.editor.sender().send(message);
    }

    pub(crate) fn send_tab(&self, message: SessionTabBarMsg) {
        let _ = self.tab_bar.sender().send(message);
    }

    pub(crate) fn send_pane(&self, message: PaneHostMsg) {
        let _ = self.pane_host.sender().send(message);
    }

    pub(crate) fn send_settings(&self, message: SettingsWindowMsg) {
        let _ = self.dialogs.settings.sender().send(message);
    }

    pub(crate) fn send_import(&self, message: ImportDialogMsg) {
        let _ = self.dialogs.import.sender().send(message);
    }

    pub(crate) fn send_interaction(&self, message: InteractionDialogMsg) {
        let _ = self.dialogs.interaction.sender().send(message);
    }
}
