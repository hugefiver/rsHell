use std::{collections::BTreeMap, fmt};

use rshell_core::{
    AppViewModel, ErrorPaneView, PaneId, SessionFailure, SessionId, SessionState, SessionUiEvent,
    TabId, UiPortError,
};

use crate::{PaneProjection, SessionPaneViewModel};

pub struct PaneHostModel {
    view_model: AppViewModel,
    active_tab: Option<TabId>,
    active_panes: BTreeMap<TabId, PaneId>,
    status: Option<String>,
    startup_probe: Option<crate::StartupProbe>,
}

impl fmt::Debug for PaneHostModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PaneHostModel")
            .field("tab_count", &self.view_model.workspace.tabs.len())
            .field("active_tab", &self.active_tab)
            .field("active_panes", &self.active_panes)
            .field("status", &self.status)
            .finish()
    }
}

impl PaneHostModel {
    pub fn new(view_model: AppViewModel) -> Self {
        let active_tab = view_model.workspace.active_tab;
        let active_panes = view_model
            .workspace
            .tabs
            .iter()
            .map(|tab| (tab.id, tab.active_pane))
            .collect();
        Self {
            view_model,
            active_tab,
            active_panes,
            status: None,
            startup_probe: None,
        }
    }

    pub(crate) fn with_startup_probe(mut self, probe: Option<crate::StartupProbe>) -> Self {
        self.startup_probe = probe;
        self
    }

    pub fn replace_view_model(&mut self, view_model: AppViewModel) {
        let known_tabs = self
            .view_model
            .workspace
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>();
        let incoming_active = view_model.workspace.active_tab;
        if incoming_active.is_some_and(|tab| !known_tabs.contains(&tab))
            || self
                .active_tab
                .is_none_or(|active| view_model.workspace.tab(active).is_err())
        {
            self.active_tab = incoming_active;
        }
        let mut active_panes = BTreeMap::new();
        for tab in &view_model.workspace.tabs {
            let active = self
                .active_panes
                .get(&tab.id)
                .copied()
                .filter(|pane| tab.pane_tree.contains_pane(*pane))
                .unwrap_or(tab.active_pane);
            active_panes.insert(tab.id, active);
        }
        self.active_panes = active_panes;
        self.view_model = view_model;
        self.status = None;
    }

    pub fn view_model(&self) -> &AppViewModel {
        &self.view_model
    }

    pub fn active_tab(&self) -> Option<TabId> {
        self.active_tab
    }

    pub fn active_pane(&self, tab: TabId) -> Option<PaneId> {
        self.active_panes.get(&tab).copied()
    }

    pub fn activate_tab(&mut self, tab: TabId) -> bool {
        if self.view_model.workspace.tab(tab).is_err() {
            return false;
        }
        self.active_tab = Some(tab);
        true
    }

    pub fn activate_pane(&mut self, pane: PaneId) -> bool {
        let Some(tab) = self
            .view_model
            .workspace
            .tabs
            .iter()
            .find(|tab| tab.pane_tree.contains_pane(pane))
        else {
            return false;
        };
        self.active_tab = Some(tab.id);
        self.active_panes.insert(tab.id, pane);
        true
    }

    pub fn pane(&self, pane: PaneId) -> Option<SessionPaneViewModel> {
        SessionPaneViewModel::from_app(&self.view_model, pane)
    }

    pub fn projection(&self, tab: TabId) -> Option<PaneProjection> {
        let tree = &self.view_model.workspace.tab(tab).ok()?.pane_tree;
        Some(PaneProjection::from_app(&self.view_model, tree))
    }

    pub fn apply_session_event(&mut self, session: SessionId, event: SessionUiEvent) -> bool {
        if !self.session_is_bound(session) {
            return false;
        }
        match event {
            SessionUiEvent::State(state) => {
                self.view_model.session_states.insert(session, state);
            }
            SessionUiEvent::Frame(frame) => {
                self.view_model.latest_frames.insert(session, frame);
            }
            SessionUiEvent::Exited(_) => {
                self.view_model
                    .session_states
                    .insert(session, SessionState::Exited);
            }
            SessionUiEvent::Failed(failure) => {
                self.record_error(session, failure, "session failed");
            }
            SessionUiEvent::Crashed(_) => {
                self.record_error(session, SessionFailure::Crashed, "session actor crashed");
            }
            SessionUiEvent::Search(_)
            | SessionUiEvent::Copy(_)
            | SessionUiEvent::InteractionRequired(_) => {}
        }
        true
    }

    pub fn command_rejected(&mut self, error: UiPortError) {
        self.status = Some(error.to_string());
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn observe_frame(&self, frame: &rshell_core::RenderFrame) {
        if let Some(probe) = &self.startup_probe {
            probe.observe_render_frame(frame);
        }
    }

    fn session_is_bound(&self, session: SessionId) -> bool {
        self.view_model.workspace.tabs.iter().any(|tab| {
            tab.pane_tree
                .session_ids()
                .into_iter()
                .any(|candidate| candidate == session)
        })
    }

    fn record_error(
        &mut self,
        session: SessionId,
        failure: SessionFailure,
        diagnostic: &'static str,
    ) {
        self.view_model.session_states.insert(
            session,
            if failure == SessionFailure::Crashed {
                SessionState::Crashed
            } else {
                SessionState::Failed
            },
        );
        let host = self.view_model.workspace.tabs.iter().find_map(|tab| {
            let mut found = None;
            tab.pane_tree.visit_leaves(&mut |pane, candidate| {
                if candidate == Some(session) {
                    found = self
                        .view_model
                        .pane_launches
                        .get(&pane)
                        .and_then(|target| target.host())
                        .map(str::to_owned);
                }
            });
            found
        });
        self.view_model.error_panes.insert(
            session,
            ErrorPaneView {
                failure,
                diagnostic,
                host,
                timestamp_unix_seconds: unix_timestamp(),
            },
        );
    }
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
