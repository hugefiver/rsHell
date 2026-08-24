use crate::{
    AppError, AppEvent, AppFailure, AppFailureCategory, ErrorPaneView, InteractionId,
    InteractionResponse, PaneId, RecoveryAction, SessionFailure, SessionId, SessionUiCommand,
    SessionUiEvent, WorkspaceError,
};

use super::runtime::{CommandLoop, InternalEvent};

impl CommandLoop {
    pub(super) async fn session_command(
        &self,
        session: SessionId,
        command: SessionUiCommand,
        report_missing: bool,
    ) -> bool {
        if !self.session_is_bound(session) {
            if report_missing {
                self.fail(unknown_session()).await;
            }
            return false;
        }
        if let Err(error) = self.dependencies.sessions.command(session, command).await {
            self.fail(Self::session_failure(error, None)).await;
            return false;
        }
        true
    }

    pub(super) async fn respond(
        &self,
        session: SessionId,
        interaction: InteractionId,
        response: InteractionResponse,
    ) {
        if self
            .session_command(
                session,
                SessionUiCommand::Respond {
                    interaction,
                    response,
                },
                true,
            )
            .await
        {
            self.emit(AppEvent::InteractionResponded {
                session,
                interaction,
            })
            .await;
        }
    }

    pub(super) async fn close_pane(&mut self, pane: PaneId) {
        let Some((tab_index, session)) = self.pane_location(pane) else {
            self.fail(invalid_workspace()).await;
            return;
        };
        let candidate = self.view_model.workspace.tabs[tab_index]
            .pane_tree
            .clone()
            .close(pane);
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(WorkspaceError::LastPane) => {
                let tab = self.view_model.workspace.tabs[tab_index].id;
                self.close_tab(tab).await;
                return;
            }
            Err(_) => {
                self.fail(invalid_workspace()).await;
                return;
            }
        };
        if let Some(session) = session
            && !self.shutdown_bound_session(session).await
        {
            return;
        }
        let active_pane = candidate.pane_ids()[0];
        self.view_model.workspace.tabs[tab_index].pane_tree = candidate;
        self.view_model.workspace.tabs[tab_index].active_pane = active_pane;
        self.view_model.pane_launches.remove(&pane);
        self.workspace_changed().await;
    }

    pub(super) async fn close_tab(&mut self, tab: uuid::Uuid) {
        let Some(index) = self
            .view_model
            .workspace
            .tabs
            .iter()
            .position(|candidate| candidate.id == tab)
        else {
            self.fail(invalid_workspace()).await;
            return;
        };
        let sessions = self.view_model.workspace.tabs[index]
            .pane_tree
            .session_ids();
        let mut stopped = Vec::new();
        for session in &sessions {
            match self.dependencies.sessions.shutdown(*session).await {
                Ok(()) => stopped.push(*session),
                Err(error) => {
                    for stopped in stopped {
                        self.stop_session_forwarders(stopped);
                        self.view_model
                            .session_states
                            .insert(stopped, crate::SessionState::Exited);
                        self.view_model.error_panes.remove(&stopped);
                    }
                    self.publish_view();
                    self.fail(Self::session_failure(error, None)).await;
                    return;
                }
            }
        }
        for session in sessions {
            self.clear_session_artifacts(session);
        }
        let removed = self.view_model.workspace.tabs.remove(index);
        self.view_model.workspace.active_tab = self
            .view_model
            .workspace
            .tabs
            .get(index.min(self.view_model.workspace.tabs.len().saturating_sub(1)))
            .map(|tab| tab.id);
        for pane in removed.pane_tree.pane_ids() {
            self.view_model.pane_launches.remove(&pane);
        }
        self.workspace_changed().await;
    }

    pub(super) async fn forward(&mut self, forwarded: InternalEvent) {
        let (session, event) = match forwarded {
            InternalEvent::Frame(session, frame) => {
                if !self.forwarders.contains_key(&session) || !self.session_is_bound(session) {
                    return;
                }
                self.view_model.latest_frames.insert(session, frame.clone());
                self.publish_view();
                (session, SessionUiEvent::Frame(frame))
            }
            InternalEvent::Session(session, event) => {
                if !self.forwarders.contains_key(&session) || !self.session_is_bound(session) {
                    return;
                }
                (session, sanitize_event(event))
            }
        };
        let completed = matches!(
            &event,
            SessionUiEvent::Exited(_) | SessionUiEvent::Failed(_) | SessionUiEvent::Crashed(_)
        );
        match &event {
            SessionUiEvent::State(state) => {
                self.view_model.session_states.insert(session, *state);
                self.publish_view();
            }
            SessionUiEvent::InteractionRequired(request) => {
                self.emit(AppEvent::InteractionRequired {
                    session,
                    request: request.clone(),
                })
                .await;
            }
            SessionUiEvent::Failed(failure) => {
                self.view_model
                    .session_states
                    .insert(session, crate::SessionState::Failed);
                self.retain_error_pane(session, *failure, "session failed");
            }
            SessionUiEvent::Crashed(_) => {
                self.view_model
                    .session_states
                    .insert(session, crate::SessionState::Crashed);
                self.retain_error_pane(session, SessionFailure::Crashed, "session actor crashed");
            }
            SessionUiEvent::Exited(_) => {
                self.view_model
                    .session_states
                    .insert(session, crate::SessionState::Exited);
                self.publish_view();
            }
            _ => {}
        }
        self.emit(AppEvent::Session { session, event }).await;
        if completed && self.unbind_session(session) {
            self.publish_view();
        }
    }

    pub(super) async fn finish_shutdown(&mut self) -> Result<(), AppError> {
        self.cancel_all_imports().await;
        let session_shutdown = self
            .dependencies
            .sessions
            .shutdown_all()
            .await
            .map_err(AppError::SessionShutdown);
        for handles in self.forwarders.values() {
            for handle in handles {
                handle.abort();
            }
        }
        self.forwarders.clear();
        let _ = self.events.try_send(AppEvent::ShutdownComplete);
        session_shutdown
    }

    fn session_is_bound(&self, session: SessionId) -> bool {
        self.view_model
            .workspace
            .tabs
            .iter()
            .any(|tab| tab.pane_tree.session_ids().contains(&session))
    }

    pub(super) fn retain_error_pane(
        &mut self,
        session: SessionId,
        failure: SessionFailure,
        diagnostic: &'static str,
    ) {
        let host = self
            .pane_for_session(session)
            .and_then(|pane| self.view_model.pane_launches.get(&pane))
            .and_then(|target| target.host())
            .map(str::to_owned);
        self.view_model.error_panes.insert(
            session,
            ErrorPaneView {
                failure,
                diagnostic,
                host,
                timestamp_unix_seconds: unix_timestamp(),
            },
        );
        self.publish_view();
    }

    fn pane_for_session(&self, session: SessionId) -> Option<PaneId> {
        self.view_model.workspace.tabs.iter().find_map(|tab| {
            let mut found = None;
            tab.pane_tree.visit_leaves(&mut |pane, candidate| {
                if candidate == Some(session) {
                    found = Some(pane);
                }
            });
            found
        })
    }
}

fn sanitize_event(event: SessionUiEvent) -> SessionUiEvent {
    match event {
        SessionUiEvent::Crashed(_) => SessionUiEvent::Crashed("session actor crashed".into()),
        other => other,
    }
}

fn unknown_session() -> AppFailure {
    AppFailure::retryable(
        AppFailureCategory::Validation,
        "session is no longer available",
        RecoveryAction::Retry,
    )
}

fn invalid_workspace() -> AppFailure {
    AppFailure::retryable(
        AppFailureCategory::Validation,
        "workspace operation is invalid",
        RecoveryAction::Retry,
    )
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
