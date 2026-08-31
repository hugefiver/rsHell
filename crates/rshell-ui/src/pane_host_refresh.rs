use std::collections::BTreeSet;

use rshell_core::{AppViewModel, SessionId};

pub(crate) fn projection_changed(current: &AppViewModel, incoming: &AppViewModel) -> bool {
    let current_tab = current.workspace.active_tab();
    let incoming_tab = incoming.workspace.active_tab();
    if current_tab != incoming_tab {
        return true;
    }
    let sessions = current_tab
        .into_iter()
        .flat_map(|tab| tab.pane_tree.session_ids())
        .chain(
            incoming_tab
                .into_iter()
                .flat_map(|tab| tab.pane_tree.session_ids()),
        )
        .collect::<BTreeSet<_>>();
    if sessions.iter().any(|session| {
        current.session_states.get(session) != incoming.session_states.get(session)
            || current.display_recovery.get(session) != incoming.display_recovery.get(session)
            || current.error_panes.get(session) != incoming.error_panes.get(session)
    }) {
        return true;
    }
    let panes = current_tab
        .into_iter()
        .flat_map(|tab| tab.pane_tree.pane_ids())
        .chain(
            incoming_tab
                .into_iter()
                .flat_map(|tab| tab.pane_tree.pane_ids()),
        )
        .collect::<BTreeSet<_>>();
    panes
        .iter()
        .any(|pane| current.pane_launches.get(pane) != incoming.pane_launches.get(pane))
}

pub(crate) fn session_is_active(view: &AppViewModel, session: SessionId) -> bool {
    view.workspace.active_tab().is_some_and(|tab| {
        tab.pane_tree
            .session_ids()
            .into_iter()
            .any(|candidate| candidate == session)
    })
}

pub(crate) fn active_terminals_changed(current: &AppViewModel, incoming: &AppViewModel) -> bool {
    if projection_changed(current, incoming)
        || current.catalog != incoming.catalog
        || current.settings != incoming.settings
        || current.terminal_profiles != incoming.terminal_profiles
    {
        return true;
    }
    let Some(active) = incoming.workspace.active_tab() else {
        return false;
    };
    active
        .pane_tree
        .session_ids()
        .into_iter()
        .any(|session| current.latest_frames.get(&session) != incoming.latest_frames.get(&session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rshell_core::{AppBootstrapState, AppViewModel, SessionState};

    #[test]
    fn revision_only_updates_do_not_rebuild_the_pane_projection() {
        let pane = rshell_core::PaneId::new();
        let session = rshell_core::SessionId::new();
        let tab = rshell_core::TabId::new_v4();
        let mut current = AppViewModel::from(AppBootstrapState {
            catalog: Default::default(),
            settings: Default::default(),
            terminal_profiles: Vec::new(),
        });
        current.workspace.tabs.push(rshell_core::TabState {
            id: tab,
            title: "active".into(),
            pane_tree: rshell_core::PaneTree::with_session(pane, session),
            active_pane: pane,
        });
        current.workspace.active_tab = Some(tab);
        let mut incoming = current.clone();
        incoming.revision = 1;
        assert!(!projection_changed(&current, &incoming));

        incoming
            .session_states
            .insert(session, SessionState::Connected);
        assert!(projection_changed(&current, &incoming));
    }

    #[test]
    fn inactive_state_updates_do_not_rebuild_the_active_projection() {
        let pane = rshell_core::PaneId::new();
        let active_session = rshell_core::SessionId::new();
        let tab = rshell_core::TabId::new_v4();
        let mut current = AppViewModel::from(AppBootstrapState {
            catalog: Default::default(),
            settings: Default::default(),
            terminal_profiles: Vec::new(),
        });
        current.workspace.tabs.push(rshell_core::TabState {
            id: tab,
            title: "active".into(),
            pane_tree: rshell_core::PaneTree::with_session(pane, active_session),
            active_pane: pane,
        });
        current.workspace.active_tab = Some(tab);
        let mut incoming = current.clone();
        incoming
            .session_states
            .insert(rshell_core::SessionId::new(), SessionState::Exited);
        assert!(!projection_changed(&current, &incoming));
    }

    #[test]
    fn inactive_frames_do_not_resynchronize_visible_terminals() {
        let pane = rshell_core::PaneId::new();
        let active_session = rshell_core::SessionId::new();
        let inactive_session = rshell_core::SessionId::new();
        let tab = rshell_core::TabId::new_v4();
        let mut current = AppViewModel::from(AppBootstrapState {
            catalog: Default::default(),
            settings: Default::default(),
            terminal_profiles: Vec::new(),
        });
        current.workspace.tabs.push(rshell_core::TabState {
            id: tab,
            title: "active".into(),
            pane_tree: rshell_core::PaneTree::with_session(pane, active_session),
            active_pane: pane,
        });
        current.workspace.active_tab = Some(tab);
        let mut incoming = current.clone();
        incoming.latest_frames.insert(
            inactive_session,
            std::sync::Arc::new(rshell_core::RenderFrame {
                generation: 1,
                size: rshell_core::TerminalSize {
                    cols: 80,
                    rows: 24,
                    pixel_width: 0,
                    pixel_height: 0,
                    dpi: 96,
                },
                viewport_top: 0,
                rows: std::sync::Arc::from([]),
                cursor: None,
                title: String::new(),
                display_modes: Default::default(),
                alternate_screen: false,
                mouse_reporting: false,
            }),
        );
        assert!(!active_terminals_changed(&current, &incoming));
    }
}
