use std::sync::Arc;

use rshell_core::{
    AppBootstrapState, AppViewModel, ErrorPaneView, KeyBinding, KeyCode, KeyModifiers, PaneId,
    PaneLaunchTarget, PaneTree, RenderFrame, SessionFailure, SessionId, SessionState,
    SessionUiEvent, SplitAxis, TabId, TabState, TerminalProfile, TerminalSize, UiCommand,
    UiPortError, WorkspaceState,
};
use rshell_ui::{
    PaneAction, PaneHostModel, PanePageKind, PaneProjection, SessionPaneViewModel,
    SessionTabBarAction,
};

#[test]
fn two_tabs_with_nested_axes_keep_independent_active_panes() {
    let fixture = workspace_fixture();
    let model = PaneHostModel::new(fixture.view.clone());

    assert_eq!(fixture.view.workspace.tabs.len(), 2);
    assert_eq!(model.active_pane(fixture.first_tab), Some(fixture.h_pane));
    assert_eq!(
        model.active_pane(fixture.second_tab),
        Some(fixture.other_pane)
    );
    assert_eq!(
        model.projection(fixture.first_tab).unwrap().axes(),
        vec![SplitAxis::Horizontal, SplitAxis::Vertical]
    );
    assert_eq!(
        model.projection(fixture.second_tab).unwrap().axes(),
        vec![SplitAxis::Vertical]
    );
}

#[test]
fn reconnect_and_retry_use_application_retry_pane() {
    assert!(matches!(
        SessionTabBarAction::NewLocalTab.command(),
        UiCommand::NewLocalTab
    ));
    let pane = PaneId::new();
    assert!(matches!(
        PaneAction::SplitHorizontal.command(pane, None),
        Some(UiCommand::Split {
            pane: target,
            axis: SplitAxis::Horizontal
        }) if target == pane
    ));
    assert!(matches!(
        PaneAction::SplitVertical.command(pane, None),
        Some(UiCommand::Split {
            pane: target,
            axis: SplitAxis::Vertical
        }) if target == pane
    ));
    assert!(matches!(
        PaneAction::Retry.command(pane, None),
        Some(UiCommand::RetryPane(target)) if target == pane
    ));
    assert!(matches!(
        PaneAction::Reconnect.command(pane, None),
        Some(UiCommand::RetryPane(target)) if target == pane
    ));
    assert!(matches!(
        PaneAction::Close.command(pane, None),
        Some(UiCommand::ClosePane(target)) if target == pane
    ));
}

#[test]
fn every_session_state_has_an_explicit_page_and_status() {
    let fixture = workspace_fixture();
    let pane = fixture.h_pane;
    let session = fixture.h_session;
    let mut view = fixture.view;
    let cases = [
        (SessionState::Created, PanePageKind::Pending, "Created"),
        (
            SessionState::Connecting,
            PanePageKind::Pending,
            "Connecting",
        ),
        (
            SessionState::AwaitingHostKey,
            PanePageKind::Pending,
            "Awaiting host key",
        ),
        (
            SessionState::AwaitingAuthentication,
            PanePageKind::Pending,
            "Awaiting authentication",
        ),
        (SessionState::Connected, PanePageKind::Terminal, "Connected"),
        (
            SessionState::Reconnecting,
            PanePageKind::Pending,
            "Reconnecting",
        ),
        (SessionState::Closing, PanePageKind::Pending, "Closing"),
        (SessionState::Exited, PanePageKind::Status, "Exited"),
        (SessionState::Failed, PanePageKind::Error, "Failed"),
        (SessionState::Crashed, PanePageKind::Error, "Crashed"),
    ];

    for (state, page, label) in cases {
        view.session_states.insert(session, state);
        let pane_view = SessionPaneViewModel::from_app(&view, pane).unwrap();
        assert_eq!(pane_view.page(), page, "state {state:?}");
        assert_eq!(pane_view.status_label(), label);
    }
}

#[test]
fn frames_route_only_to_the_bound_session_and_stale_retry_events_are_ignored() {
    let fixture = workspace_fixture();
    let pane = fixture.h_pane;
    let old = fixture.h_session;
    let mut model = PaneHostModel::new(fixture.view.clone());
    let other_frame = frame(7, "other");
    assert!(model.apply_session_event(fixture.v_session, SessionUiEvent::Frame(other_frame)));
    assert!(model.pane(pane).unwrap().frame().is_none());

    let new = SessionId::new();
    let mut replacement = fixture.view;
    replacement.workspace.tabs[0]
        .pane_tree
        .replace_session(pane, Some(new))
        .unwrap();
    replacement.session_states.remove(&old);
    replacement
        .session_states
        .insert(new, SessionState::Connected);
    replacement.latest_frames.remove(&old);
    replacement.error_panes.remove(&old);
    model.replace_view_model(replacement);

    assert!(!model.apply_session_event(old, SessionUiEvent::Frame(frame(8, "stale"))));
    assert!(model.apply_session_event(new, SessionUiEvent::Frame(frame(9, "fresh"))));
    assert_eq!(model.pane(pane).unwrap().frame().unwrap().generation, 9);
}

#[test]
fn error_actions_are_ordered_and_diagnostics_are_strictly_redacted() {
    let fixture = workspace_fixture();
    let pane = fixture.connection_pane;
    let session = fixture.connection_session;
    let mut view = fixture.view;
    view.session_states.insert(session, SessionState::Failed);
    view.error_panes.insert(
        session,
        ErrorPaneView {
            failure: SessionFailure::Authentication,
            diagnostic: "session failed",
            host: Some("safe.example.test".into()),
            timestamp_unix_seconds: 1_785_632_400,
        },
    );
    let pane_view = SessionPaneViewModel::from_app(&view, pane).unwrap();

    assert_eq!(
        pane_view.actions(),
        vec![
            PaneAction::Retry,
            PaneAction::EditConnection,
            PaneAction::CopyDiagnostics,
            PaneAction::Close,
        ]
    );
    let diagnostic = pane_view.diagnostics().unwrap();
    assert!(diagnostic.contains("category: authentication"));
    assert!(diagnostic.contains("host: safe.example.test"));
    assert!(diagnostic.contains("timestamp: 1785632400"));
    assert!(diagnostic.contains("error: session failed"));
    for forbidden in [
        "password",
        "credential",
        "identity_file",
        "C:\\",
        "raw error",
    ] {
        assert!(
            !diagnostic
                .to_ascii_lowercase()
                .contains(&forbidden.to_ascii_lowercase())
        );
    }

    let local = SessionPaneViewModel::from_app(&view, fixture.h_pane).unwrap();
    assert!(!local.actions().contains(&PaneAction::EditConnection));
}

#[test]
fn local_activation_is_independent_and_replacement_is_not_optimistic() {
    let fixture = workspace_fixture();
    let mut model = PaneHostModel::new(fixture.view.clone());
    model.activate_tab(fixture.second_tab);
    model.activate_pane(fixture.other_second_pane);
    assert_eq!(model.active_tab(), Some(fixture.second_tab));
    assert_eq!(
        model.active_pane(fixture.second_tab),
        Some(fixture.other_second_pane)
    );
    assert_eq!(model.view_model().workspace.tabs.len(), 2);

    let mut replacement = fixture.view;
    let new_pane = PaneId::new();
    let new_session = SessionId::new();
    let new_tab = TabId::new_v4();
    replacement.workspace.tabs.push(TabState {
        id: new_tab,
        title: "New local".into(),
        pane_tree: PaneTree::with_session(new_pane, new_session),
        active_pane: new_pane,
    });
    replacement.workspace.active_tab = Some(new_tab);
    replacement
        .pane_launches
        .insert(new_pane, PaneLaunchTarget::Local);
    replacement
        .session_states
        .insert(new_session, SessionState::Created);
    model.replace_view_model(replacement);
    assert_eq!(model.active_tab(), Some(new_tab));
}

#[test]
fn command_rejection_is_total_and_sanitized() {
    let mut model = PaneHostModel::new(workspace_fixture().view);
    model.command_rejected(UiPortError::Busy);
    assert_eq!(model.status(), Some("application is busy"));
    model.command_rejected(UiPortError::Closed);
    assert_eq!(model.status(), Some("application command port is closed"));
    assert!(!format!("{model:?}").contains("secret"));
}

#[test]
fn pane_profile_and_connection_bindings_shadow_app_bindings() {
    let mut fixture = workspace_fixture();
    let chord = KeyModifiers {
        control: true,
        ..KeyModifiers::default()
    };
    fixture.view.terminal_profiles[0].settings.key_bindings =
        vec![key_binding(KeyCode::F(2), chord, "new_tab")];
    fixture.view.settings.key_bindings = vec![
        key_binding(KeyCode::F(2), chord, "split_vertical"),
        key_binding(KeyCode::F(3), chord, "clear_scrollback"),
    ];
    let connection_id = SessionPaneViewModel::from_app(&fixture.view, fixture.connection_pane)
        .unwrap()
        .connection_id()
        .unwrap();
    fixture
        .view
        .catalog
        .connections
        .get_mut(&connection_id)
        .unwrap()
        .terminal_overrides
        .key_bindings = Some(vec![key_binding(KeyCode::F(2), chord, "clear_scrollback")]);

    let local = SessionPaneViewModel::from_app(&fixture.view, fixture.h_pane)
        .unwrap()
        .resolved_profile(&fixture.view)
        .unwrap();
    assert_eq!(
        local
            .key_bindings
            .iter()
            .map(|binding| binding.action.as_str())
            .collect::<Vec<_>>(),
        ["new_tab", "clear_scrollback"]
    );
    let connection = SessionPaneViewModel::from_app(&fixture.view, fixture.connection_pane)
        .unwrap()
        .resolved_profile(&fixture.view)
        .unwrap();
    assert_eq!(
        connection
            .key_bindings
            .iter()
            .map(|binding| binding.action.as_str())
            .collect::<Vec<_>>(),
        ["clear_scrollback", "clear_scrollback"]
    );
}

fn key_binding(code: KeyCode, modifiers: KeyModifiers, action: &str) -> KeyBinding {
    KeyBinding {
        code,
        modifiers,
        action: action.to_owned(),
    }
}

fn frame(generation: u64, title: &str) -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation,
        size: TerminalSize {
            cols: 80,
            rows: 24,
            pixel_width: 720,
            pixel_height: 432,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([]),
        cursor: None,
        title: title.into(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

struct Fixture {
    view: AppViewModel,
    first_tab: TabId,
    second_tab: TabId,
    h_pane: PaneId,
    h_session: SessionId,
    v_session: SessionId,
    connection_pane: PaneId,
    connection_session: SessionId,
    other_pane: PaneId,
    other_second_pane: PaneId,
}

fn workspace_fixture() -> Fixture {
    let h_pane = PaneId::new();
    let h_session = SessionId::new();
    let v_pane = PaneId::new();
    let v_session = SessionId::new();
    let connection_pane = PaneId::new();
    let connection_session = SessionId::new();
    let other_pane = PaneId::new();
    let other_session = SessionId::new();
    let other_second_pane = PaneId::new();
    let other_second_session = SessionId::new();
    let first_tab = TabId::new_v4();
    let second_tab = TabId::new_v4();
    let mut first_tree = PaneTree::with_session(h_pane, h_session)
        .split(h_pane, SplitAxis::Horizontal, v_pane, 0.5)
        .unwrap()
        .split(v_pane, SplitAxis::Vertical, connection_pane, 0.5)
        .unwrap();
    first_tree.replace_session(v_pane, Some(v_session)).unwrap();
    first_tree
        .replace_session(connection_pane, Some(connection_session))
        .unwrap();
    let mut second_tree = PaneTree::with_session(other_pane, other_session)
        .split(other_pane, SplitAxis::Vertical, other_second_pane, 0.5)
        .unwrap();
    second_tree
        .replace_session(other_second_pane, Some(other_second_session))
        .unwrap();
    let workspace = WorkspaceState {
        tabs: vec![
            TabState {
                id: first_tab,
                title: "Nested".into(),
                pane_tree: first_tree,
                active_pane: h_pane,
            },
            TabState {
                id: second_tab,
                title: "Other".into(),
                pane_tree: second_tree,
                active_pane: other_pane,
            },
        ],
        active_tab: Some(first_tab),
    };
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace = workspace;
    for (pane, session) in [
        (h_pane, h_session),
        (v_pane, v_session),
        (other_pane, other_session),
        (other_second_pane, other_second_session),
    ] {
        view.pane_launches.insert(pane, PaneLaunchTarget::Local);
        view.session_states.insert(session, SessionState::Connected);
    }
    let connection = rshell_core::ConnectionProfile::new("Managed", "safe.example.test");
    let connection_id = connection.id;
    view.catalog.connections.insert(connection_id, connection);
    view.pane_launches.insert(
        connection_pane,
        PaneLaunchTarget::Connection {
            id: connection_id,
            host: "safe.example.test".into(),
        },
    );
    view.session_states
        .insert(connection_session, SessionState::Connected);
    Fixture {
        view,
        first_tab,
        second_tab,
        h_pane,
        h_session,
        v_session,
        connection_pane,
        connection_session,
        other_pane,
        other_second_pane,
    }
}

#[test]
fn projection_leaf_count_matches_nested_workspace() {
    let fixture = workspace_fixture();
    let projection =
        PaneProjection::from_app(&fixture.view, &fixture.view.workspace.tabs[0].pane_tree);
    assert_eq!(projection.leaf_count(), 3);
}

#[test]
fn missing_launch_target_is_explicitly_unavailable_and_projection_is_total() {
    let pane = PaneId::new();
    let session = SessionId::new();
    let tree = PaneTree::with_session(pane, session);
    let view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });

    let projection = PaneProjection::from_app(&view, &tree);
    let PaneProjection::Leaf(pane_view) = projection else {
        panic!("single leaf must remain a leaf")
    };
    assert_eq!(pane_view.page(), PanePageKind::Unavailable);
    assert_eq!(pane_view.status_label(), "Session unavailable");
    assert_eq!(pane_view.actions(), vec![PaneAction::Close]);
    assert_eq!(pane_view.connection_id(), None);
    assert_eq!(pane_view.diagnostics(), None);
    assert_eq!(pane_view.resolved_profile(&view), None);
}
