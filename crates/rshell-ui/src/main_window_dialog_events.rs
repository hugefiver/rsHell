use gtk::prelude::*;
use relm4::gtk;
use rshell_core::UiCommand;

use crate::{
    ImportDialogMsg, ImportDialogOutput, InteractionDialogOutput, MainWindow, SettingsWindowMsg,
    SettingsWindowOutput, main_window_commands::CommandSource,
    main_window_dialogs::DialogCommandSource,
};

impl MainWindow {
    pub(crate) fn handle_settings(&mut self, output: SettingsWindowOutput) {
        match output {
            SettingsWindowOutput::Command(command) => {
                if self.dispatch(*command, CommandSource::Settings) {
                    self.pending_dialog = Some(DialogCommandSource::Settings);
                    self.status = "Saving terminal settings".into();
                }
            }
            SettingsWindowOutput::Closed => self.status = "Ready".into(),
        }
    }

    pub(crate) fn handle_import(&mut self, output: ImportDialogOutput) {
        match output {
            ImportDialogOutput::Command(command) => {
                if self.dispatch(*command, CommandSource::Import) {
                    self.pending_dialog = Some(DialogCommandSource::Import);
                    self.status = "Import operation in progress".into();
                }
            }
            ImportDialogOutput::Closed => self.status = "Ready".into(),
            ImportDialogOutput::StateChanged(state) => self.observe_smoke_import(state),
        }
    }

    pub(crate) fn handle_interaction(&mut self, output: InteractionDialogOutput) {
        match output {
            InteractionDialogOutput::Command(command) => {
                let pending = match command.as_ref() {
                    UiCommand::Respond {
                        session,
                        interaction,
                        ..
                    } => Some((*session, *interaction)),
                    _ => None,
                };
                self.pending_interaction = pending;
                if self.dispatch(*command, CommandSource::Interaction) {
                    self.status = "Sending secure response".into();
                } else {
                    self.pending_interaction = None;
                }
            }
            InteractionDialogOutput::CopyDiagnostics(diagnostics) => {
                if let Some(display) = gtk::gdk::Display::default() {
                    display.clipboard().set_text(&diagnostics);
                    self.status = "Host-key diagnostics copied".into();
                }
            }
            InteractionDialogOutput::Closed => self.status = "Ready".into(),
            InteractionDialogOutput::StateChanged(state) => self.observe_smoke_interaction(state),
        }
    }

    pub(crate) fn open_settings(&self) {
        self.send_settings(SettingsWindowMsg::Open);
    }

    pub(crate) fn open_import(&self) {
        self.send_import(ImportDialogMsg::Open);
    }
}
