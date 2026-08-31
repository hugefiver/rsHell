use gtk::prelude::*;
use relm4::{ComponentController, gtk};
use rshell_core::{AppEvent, SessionUiEvent, UiCommand};

use crate::{
    ConnectionEditorMsg, ConnectionEditorOutput, ConnectionSidebarMsg, ConnectionSidebarOutput,
    ImportDialogMsg, InteractionDialogMsg, MainWindow, ModalKind, ModalRequest, PaneHostMsg,
    PaneHostOutput, SessionTabBarOutput, SettingsWindowMsg,
    main_window_commands::CommandSource,
    main_window_dialogs::DialogCommandSource,
    main_window_snapshots::{session_is_bound, update_session_snapshot},
    session_diagnostics::emit_session_failure,
};

impl MainWindow {
    pub(crate) fn new_local_tab(&mut self) {
        self.dispatch(UiCommand::NewLocalTab, CommandSource::TabBar);
    }

    pub(crate) fn handle_sidebar(&mut self, output: ConnectionSidebarOutput) {
        let closes_drawer = output.closes_navigation_drawer();
        match output {
            ConnectionSidebarOutput::Command(command) => {
                self.dispatch(command, CommandSource::Sidebar);
            }
            ConnectionSidebarOutput::Connect(connection) => {
                self.send_pane(PaneHostMsg::Connect { connection });
            }
            ConnectionSidebarOutput::OpenCreate(group) => {
                self.open_editor(ConnectionEditorMsg::OpenCreate(group));
            }
            ConnectionSidebarOutput::OpenEdit(profile) => {
                self.open_editor(ConnectionEditorMsg::OpenEdit(Box::new(profile)));
            }
            ConnectionSidebarOutput::SelectionChanged(selection) => {
                self.smoke_state.sidebar_selection = selection;
            }
        }
        if closes_drawer {
            self.shell
                .close_navigation_drawer(self.sidebar.widget().upcast_ref());
        }
    }

    pub(crate) fn handle_editor(&mut self, output: ConnectionEditorOutput) {
        match output {
            ConnectionEditorOutput::Command(command) => {
                if self.dispatch(*command, CommandSource::Editor) {
                    self.editor_command_pending = true;
                    self.status = "Saving connection".into();
                }
            }
            ConnectionEditorOutput::Closed => {
                self.handle_modal(ModalRequest::Close(ModalKind::ConnectionEditor));
                self.status = "Ready".into();
            }
            ConnectionEditorOutput::StateChanged(state) => self.observe_smoke_editor(*state),
        }
    }

    pub(crate) fn handle_tab_bar(&mut self, output: SessionTabBarOutput) {
        match output {
            SessionTabBarOutput::Command(command) => {
                self.dispatch(*command, CommandSource::TabBar);
            }
            SessionTabBarOutput::ActivateTab(tab) => self.send_pane(PaneHostMsg::ActivateTab(tab)),
        }
    }

    pub(crate) fn handle_pane_host(&mut self, output: PaneHostOutput) {
        match output {
            PaneHostOutput::Command(command) => {
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
            PaneHostOutput::ClipboardWritten { bytes } => {
                self.smoke_state.clipboard_writes =
                    self.smoke_state.clipboard_writes.saturating_add(1);
                self.smoke_state.clipboard_bytes = Some(bytes);
                self.observe_smoke_clipboard_write(bytes);
            }
        }
    }

    pub(crate) fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::CatalogChanged(catalog) => {
                self.smoke_state.catalog_changes =
                    self.smoke_state.catalog_changes.saturating_add(1);
                if !self.authoritative_view {
                    self.view_model.catalog = catalog;
                }
                self.send_sidebar(ConnectionSidebarMsg::SetCatalog(
                    self.view_model.catalog.clone(),
                ));
                if self.editor_command_pending {
                    self.editor_command_pending = false;
                    self.send_editor(ConnectionEditorMsg::CommandAccepted);
                }
                self.status = "Connections updated".into();
            }
            AppEvent::WorkspaceChanged(workspace) if !self.authoritative_view => {
                self.replace_workspace(workspace);
            }
            AppEvent::Session { session, event } => {
                self.observe_smoke_session_event(session, &event);
                emit_session_failure(&event);
                if !session_is_bound(&self.view_model, session) {
                    return;
                }
                let terminal = matches!(
                    &event,
                    SessionUiEvent::Exited(_)
                        | SessionUiEvent::Failed(_)
                        | SessionUiEvent::Crashed(_)
                );
                if !self.authoritative_view {
                    update_session_snapshot(&mut self.view_model, session, &event);
                    self.send_pane(PaneHostMsg::SessionEvent { session, event });
                } else if matches!(
                    event,
                    SessionUiEvent::Search(_)
                        | SessionUiEvent::Copy(_)
                        | SessionUiEvent::InteractionRequired(_)
                ) {
                    self.send_pane(PaneHostMsg::SessionEvent { session, event });
                }
                if terminal {
                    if self
                        .pending_interaction
                        .is_some_and(|(pending_session, _)| pending_session == session)
                    {
                        self.pending_interaction = None;
                    }
                    self.send_interaction(InteractionDialogMsg::DismissSession(session));
                }
            }
            AppEvent::SettingsChanged(settings) => {
                if !self.authoritative_view {
                    self.view_model.settings = settings.clone();
                }
                self.send_settings(SettingsWindowMsg::SettingsAccepted(settings));
                self.pending_dialog = None;
                self.status = "Settings updated".into();
            }
            AppEvent::TerminalProfilesChanged(profiles) => {
                if !self.authoritative_view {
                    self.view_model.terminal_profiles = profiles.clone();
                }
                self.send_editor(ConnectionEditorMsg::SetTerminalProfiles(
                    self.view_model.terminal_profiles.clone(),
                ));
                self.send_pane(PaneHostMsg::SetViewModel(Box::new(self.view_model.clone())));
                self.send_settings(SettingsWindowMsg::ProfilesAccepted(profiles));
                self.pending_dialog = None;
                self.status = "Terminal profile updated".into();
            }
            AppEvent::ImportPreview(preview) => {
                self.observe_smoke_import_preview(&preview);
                self.send_import(ImportDialogMsg::Preview(preview));
                self.pending_dialog = None;
                self.status = "Import preview ready".into();
            }
            AppEvent::ImportCompleted(report) => {
                self.smoke_state.import_completions =
                    self.smoke_state.import_completions.saturating_add(1);
                self.observe_smoke_import_completed(report);
                self.send_import(ImportDialogMsg::Completed(report));
                self.pending_dialog = None;
                self.status = "Import complete".into();
            }
            AppEvent::ImportCancelled(preview) => {
                self.smoke_state.import_cancellations =
                    self.smoke_state.import_cancellations.saturating_add(1);
                self.observe_smoke_import_cancelled(preview);
                self.send_import(ImportDialogMsg::Cancelled(preview));
                self.pending_dialog = None;
                self.status = "Import cancelled".into();
            }
            AppEvent::InteractionRequired { session, request } => {
                self.open_interaction(InteractionDialogMsg::Open { session, request });
                self.status = "User interaction required".into();
            }
            AppEvent::InteractionResponded {
                session,
                interaction,
            } => {
                self.smoke_state.interaction_responses =
                    self.smoke_state.interaction_responses.saturating_add(1);
                self.smoke_state.last_interaction_response = Some(interaction);
                if self.pending_interaction == Some((session, interaction)) {
                    self.pending_interaction = None;
                }
                self.send_interaction(InteractionDialogMsg::ResponseAccepted(interaction));
                self.status = "Secure response accepted".into();
            }
            AppEvent::OperationFailed(failure) => {
                self.status = failure.context.into();
                if self.editor_command_pending {
                    self.editor_command_pending = false;
                    self.send_editor(ConnectionEditorMsg::OperationFailed(
                        failure.category,
                        failure.context,
                    ));
                }
                match self.pending_dialog.take() {
                    Some(DialogCommandSource::Settings) => {
                        self.send_settings(SettingsWindowMsg::OperationFailed(failure.context));
                    }
                    Some(DialogCommandSource::Import) => {
                        self.send_import(ImportDialogMsg::OperationFailed(failure));
                    }
                    None => {}
                }
                if let Some((_, interaction)) = self.pending_interaction.take() {
                    self.send_interaction(InteractionDialogMsg::OperationFailed(
                        interaction,
                        failure.context,
                    ));
                }
            }
            AppEvent::SearchResults(results) => {
                self.status = format!("{} matching connections", results.len());
            }
            AppEvent::ShutdownComplete => {
                self.smoke_state.shutdown_complete = true;
                self.status = "Shutdown complete".into();
            }
            _ => {}
        }
    }
}
