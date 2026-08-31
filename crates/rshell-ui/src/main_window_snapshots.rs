use gtk::prelude::Cast;
use relm4::{ComponentController, gtk};
use rshell_core::{
    ErrorPaneView, SessionFailure, SessionId, SessionState, SessionUiEvent, WorkspaceState,
};

use crate::{
    ConnectionEditorMsg, ConnectionSidebarMsg, MainWindow, PaneHostMsg, SessionTabBarMsg,
    ShellLayout, pane_host_refresh::active_terminals_changed,
};

impl MainWindow {
    pub(crate) fn apply_shell_layout(&mut self, width: i32) {
        let sidebar: gtk::Widget = self.sidebar.widget().clone().upcast();
        self.shell.apply(ShellLayout::for_width(width), &sidebar);
        self.send_sidebar(self.smoke_state.sidebar_selection.map_or(
            ConnectionSidebarMsg::RefreshPresentation,
            ConnectionSidebarMsg::SelectConnection,
        ));
    }

    pub(crate) fn replace_view_model(&mut self, view_model: rshell_core::AppViewModel) {
        if self.authoritative_view
            && !authoritative_revision_is_newer(self.view_model.revision, view_model.revision)
        {
            return;
        }
        let incoming_active = view_model.workspace.active_tab;
        let incoming_is_new =
            incoming_active.is_some_and(|tab| self.view_model.workspace.tab(tab).is_err());
        let local_was_removed = self
            .smoke_state
            .active_tab
            .is_some_and(|tab| view_model.workspace.tab(tab).is_err());
        if incoming_is_new || local_was_removed {
            self.smoke_state.active_tab = incoming_active;
        }
        if self.view_model.catalog != view_model.catalog {
            self.send_sidebar(ConnectionSidebarMsg::SetCatalog(view_model.catalog.clone()));
        }
        if self.view_model.terminal_profiles != view_model.terminal_profiles {
            self.send_editor(ConnectionEditorMsg::SetTerminalProfiles(
                view_model.terminal_profiles.clone(),
            ));
        }
        if self.view_model.workspace != view_model.workspace {
            self.send_tab(SessionTabBarMsg::SetWorkspace(view_model.workspace.clone()));
        }
        if active_terminals_changed(&self.view_model, &view_model) {
            self.send_pane(PaneHostMsg::SetViewModel(Box::new(view_model.clone())));
        }
        if let Some(probe) = &self.startup_probe {
            probe.observe_view_model(&view_model);
        }
        self.view_model = view_model;
    }

    pub(crate) fn replace_workspace(&mut self, workspace: WorkspaceState) {
        let sessions = workspace
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.session_ids())
            .collect::<Vec<_>>();
        let panes = workspace
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.pane_ids())
            .collect::<Vec<_>>();
        let incoming = workspace
            .tabs
            .iter()
            .flat_map(|tab| {
                let mut leaves = Vec::new();
                tab.pane_tree
                    .visit_leaves(&mut |pane, session| leaves.push((pane, session)));
                leaves
            })
            .collect::<Vec<_>>();
        for (pane, session) in incoming {
            let previous = self.view_model.workspace.tabs.iter().find_map(|tab| {
                tab.pane_tree
                    .session_id(pane)
                    .ok()
                    .and_then(|session| session)
            });
            if previous != session {
                self.view_model.pane_launches.remove(&pane);
            }
        }
        self.view_model
            .session_states
            .retain(|session, _| sessions.contains(session));
        self.view_model
            .latest_frames
            .retain(|session, _| sessions.contains(session));
        self.view_model
            .error_panes
            .retain(|session, _| sessions.contains(session));
        self.view_model
            .display_recovery
            .retain(|session, _| sessions.contains(session));
        self.view_model
            .pane_launches
            .retain(|pane, _| panes.contains(pane));
        self.view_model.workspace = workspace.clone();
        self.send_tab(SessionTabBarMsg::SetWorkspace(workspace));
        self.send_pane(PaneHostMsg::SetViewModel(Box::new(self.view_model.clone())));
    }
}

pub(crate) fn authoritative_revision_is_newer(current: u64, incoming: u64) -> bool {
    incoming > current
}

pub(crate) fn update_session_snapshot(
    view: &mut rshell_core::AppViewModel,
    session: SessionId,
    event: &SessionUiEvent,
) {
    match event {
        SessionUiEvent::State(state) => {
            view.session_states.insert(session, *state);
        }
        SessionUiEvent::Frame(frame) => {
            view.latest_frames.insert(session, frame.clone());
        }
        SessionUiEvent::RecoveryChanged(Some(notice)) => {
            view.display_recovery.insert(session, *notice);
        }
        SessionUiEvent::RecoveryChanged(None) => {
            view.display_recovery.remove(&session);
        }
        SessionUiEvent::Exited(_) => {
            view.display_recovery.remove(&session);
            view.session_states.insert(session, SessionState::Exited);
        }
        SessionUiEvent::Failed(failure) => {
            view.display_recovery.remove(&session);
            record_error(view, session, *failure, "session failed");
        }
        SessionUiEvent::Crashed(_) => {
            view.display_recovery.remove(&session);
            record_error(
                view,
                session,
                SessionFailure::Crashed,
                "session actor crashed",
            );
        }
        _ => {}
    }
}

pub(crate) fn session_is_bound(view: &rshell_core::AppViewModel, session: SessionId) -> bool {
    view.workspace
        .tabs
        .iter()
        .any(|tab| tab.pane_tree.session_ids().contains(&session))
}

fn record_error(
    view: &mut rshell_core::AppViewModel,
    session: SessionId,
    failure: SessionFailure,
    diagnostic: &'static str,
) {
    let state = if failure == SessionFailure::Crashed {
        SessionState::Crashed
    } else {
        SessionState::Failed
    };
    view.session_states.insert(session, state);
    let host = view.workspace.tabs.iter().find_map(|tab| {
        let mut pane = None;
        tab.pane_tree.visit_leaves(&mut |candidate, bound| {
            if bound == Some(session) {
                pane = Some(candidate);
            }
        });
        pane.and_then(|pane| view.pane_launches.get(&pane))
            .and_then(|target| target.host())
            .map(str::to_owned)
    });
    view.error_panes.insert(
        session,
        ErrorPaneView {
            failure,
            diagnostic,
            host,
            timestamp_unix_seconds: unix_timestamp(),
        },
    );
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::authoritative_revision_is_newer;

    #[test]
    fn authoritative_views_reject_equal_and_stale_revisions() {
        assert!(!authoritative_revision_is_newer(7, 7));
        assert!(!authoritative_revision_is_newer(7, 6));
        assert!(authoritative_revision_is_newer(7, 8));
    }
}
