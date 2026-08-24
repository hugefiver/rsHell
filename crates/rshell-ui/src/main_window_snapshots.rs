use rshell_core::{
    ErrorPaneView, SessionFailure, SessionId, SessionState, SessionUiEvent, WorkspaceState,
};

use crate::{ConnectionEditorMsg, ConnectionSidebarMsg, MainWindow, PaneHostMsg, SessionTabBarMsg};

impl MainWindow {
    pub(crate) fn replace_view_model(&mut self, view_model: rshell_core::AppViewModel) {
        if self.authoritative_view && view_model.revision < self.view_model.revision {
            return;
        }
        self.send_sidebar(ConnectionSidebarMsg::SetCatalog(view_model.catalog.clone()));
        self.send_editor(ConnectionEditorMsg::SetTerminalProfiles(
            view_model.terminal_profiles.clone(),
        ));
        self.send_tab(SessionTabBarMsg::SetWorkspace(view_model.workspace.clone()));
        self.send_pane(PaneHostMsg::SetViewModel(Box::new(view_model.clone())));
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
            .pane_launches
            .retain(|pane, _| panes.contains(pane));
        self.view_model.workspace = workspace.clone();
        self.send_tab(SessionTabBarMsg::SetWorkspace(workspace));
        self.send_pane(PaneHostMsg::SetViewModel(Box::new(self.view_model.clone())));
    }
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
        SessionUiEvent::Exited(_) => {
            view.session_states.insert(session, SessionState::Exited);
        }
        SessionUiEvent::Failed(failure) => {
            record_error(view, session, *failure, "session failed");
        }
        SessionUiEvent::Crashed(_) => {
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
