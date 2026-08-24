use relm4::ComponentController;
use rshell_core::{UiCommand, UiPortError};

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
        match dispatch(&self.command_port, command) {
            Ok(()) => true,
            Err(error) => {
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
