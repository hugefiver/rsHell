use std::sync::Arc;

use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppViewModel, AuthenticationKind, ConnectionCatalog, ConnectionProfile,
    ErrorPaneView, PaneId, PaneLaunchTarget, PaneTree, RenderFrame, SessionFailure, SessionId,
    SessionState, SessionUiEvent, SplitAxis, TabId, TabState, TerminalOverrides, TerminalProfile,
    TerminalSettingsV1, TerminalSize, TransportKind, UiPortError, WorkspaceState,
};
use rshell_ui::{
    ConnectionEditor, ConnectionEditorInit, ConnectionEditorMsg, ConnectionSidebar,
    ConnectionSidebarInit, ConnectionSidebarMsg, FontMetrics, PaneHost, PaneHostInit, PaneHostMsg,
    SessionTabBar, SessionTabBarInit, SessionTabBarMsg, TerminalView, TerminalViewInit,
    TerminalViewMsg,
};

#[test]
fn native_components_construct_and_clear_stale_reducer_state() {
    if let Err(error) = gtk::init() {
        eprintln!("native GTK reducer regression skipped: {error}");
        return;
    }
    assert_terminal_view_native_boundary();
    assert_native_workspace_boundary();
    assert_sidebar_catalog_refresh_preserves_only_the_same_visible_identity();
    assert_catalog_rebuild_then_identity_switch_quiesces();

    let editor = ConnectionEditor::builder()
        .launch(ConnectionEditorInit {
            terminal_profiles: vec![TerminalProfile::default()],
        })
        .detach();
    assert!(editor.widget().has_css_class("editor-dialog"));
    assert_eq!(editor.widget().width_request(), 560);
    assert!(!editor.widget().is_visible());
    let mut source = ConnectionProfile::new("Existing", "existing.example.test");
    source.transport = TransportKind::NativeSsh;
    source.authentication = AuthenticationKind::Password;
    source.credential_ref = Some("rshell://existing".into());
    editor.emit(ConnectionEditorMsg::OpenEdit(Box::new(source)));
    let open_quiesced = flush_gtk();
    let password = descendants(editor.widget())
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::PasswordEntry>().ok())
        .expect("editor must contain its native secret field");
    password.set_text("TASK15-NATIVE-SENTINEL");
    let secret_quiesced = flush_gtk();
    editor.emit(ConnectionEditorMsg::AuthenticationChanged(
        AuthenticationKind::KeyboardInteractive,
    ));
    let auth_quiesced = flush_gtk();

    let mut failures = Vec::new();
    if !open_quiesced || !secret_quiesced {
        failures.push("opening or editing the native form did not quiesce");
    }
    if !auth_quiesced {
        failures.push("transport/auth rendering did not quiesce");
    }
    if !password.text().is_empty() {
        failures.push("changing to non-managed auth retained stale GTK secret text");
    }

    let empty_sidebar = ConnectionSidebar::builder()
        .launch(ConnectionSidebarInit {
            catalog: ConnectionCatalog::default(),
        })
        .detach();
    assert!(
        visible_label_text(empty_sidebar.widget())
            .iter()
            .any(|text| text == "No connections yet")
    );
    for tooltip in [
        "Select a connection or group to edit",
        "Select a connection to duplicate",
        "Select a connection or group to delete",
    ] {
        let action = button_by_tooltip(empty_sidebar.widget(), tooltip);
        assert!(!action.is_sensitive());
        assert!(
            descendants(&action)
                .iter()
                .any(|child| child.has_css_class("product-icon"))
        );
    }
    empty_sidebar.emit(ConnectionSidebarMsg::Search("no-match".into()));
    assert!(flush_gtk());
    assert!(
        visible_label_text(empty_sidebar.widget())
            .iter()
            .any(|text| text == "No connections match the search")
    );

    let mut old_catalog = ConnectionCatalog::default();
    let old = ConnectionProfile::new("Old row", "old.example.test");
    old_catalog.connections.insert(old.id, old);
    let sidebar = ConnectionSidebar::builder()
        .launch(ConnectionSidebarInit {
            catalog: old_catalog,
        })
        .detach();
    assert!(sidebar.widget().has_css_class("sidebar"));
    assert_eq!(sidebar.widget().width_request(), 232);
    sidebar.emit(ConnectionSidebarMsg::Select(0));
    assert!(flush_gtk());
    assert!(
        descendants(sidebar.widget())
            .iter()
            .any(|widget| widget.has_css_class("navigation-selected"))
    );
    sidebar.emit(ConnectionSidebarMsg::RequestDelete);
    sidebar.emit(ConnectionSidebarMsg::CommandRejected(UiPortError::Busy));
    let sidebar_setup_quiesced = flush_gtk();
    assert!(
        descendants(sidebar.widget()).into_iter().any(|widget| {
            widget
                .downcast::<gtk::Revealer>()
                .is_ok_and(|revealer| revealer.reveals_child())
        }),
        "test precondition requires visible destructive confirmation"
    );
    assert!(
        visible_label_text(sidebar.widget())
            .iter()
            .any(|text| text.contains("application is busy")),
        "test precondition requires a visible stale error"
    );
    let mut replacement = ConnectionCatalog::default();
    let replacement_profile = ConnectionProfile::new("Replacement", "new.example.test");
    replacement
        .connections
        .insert(replacement_profile.id, replacement_profile);
    sidebar.emit(ConnectionSidebarMsg::SetCatalog(replacement));
    let catalog_quiesced = flush_gtk();
    sidebar.emit(ConnectionSidebarMsg::ConfirmDelete);
    let confirm_quiesced = flush_gtk();
    if !sidebar_setup_quiesced || !catalog_quiesced || !confirm_quiesced {
        failures.push("sidebar processing did not quiesce");
    }

    for tooltip in ["Edit selection", "Duplicate connection", "Delete selection"] {
        let enabled = descendants(sidebar.widget()).into_iter().any(|widget| {
            widget
                .clone()
                .downcast::<gtk::Button>()
                .is_ok_and(|button| {
                    button.tooltip_text().as_deref() == Some(tooltip) && button.is_sensitive()
                })
        });
        if enabled {
            failures.push("catalog replacement left a stale sidebar action enabled");
        }
    }
    if descendants(sidebar.widget()).into_iter().any(|widget| {
        widget
            .downcast::<gtk::Revealer>()
            .is_ok_and(|revealer| revealer.reveals_child())
    }) {
        failures.push("catalog replacement retained destructive confirmation");
    }
    if visible_label_text(sidebar.widget())
        .iter()
        .any(|text| text.contains("application is busy"))
    {
        failures.push("catalog replacement retained stale sidebar error");
    }
    assert!(
        failures.is_empty(),
        "native reducer regressions: {failures:#?}"
    );

    editor.emit(ConnectionEditorMsg::Cancel);
    assert!(flush_gtk());
    editor.emit(ConnectionEditorMsg::Save);
    assert!(flush_gtk(), "closed Save must be a native no-op");
}

fn assert_sidebar_catalog_refresh_preserves_only_the_same_visible_identity() {
    let original = ConnectionProfile::new("Retained row", "retained.example.test");
    let original_id = original.id;
    let mut catalog = ConnectionCatalog::default();
    catalog.connections.insert(original_id, original.clone());
    let sidebar = ConnectionSidebar::builder()
        .launch(ConnectionSidebarInit { catalog })
        .detach();

    sidebar.emit(ConnectionSidebarMsg::Search("Retained".into()));
    sidebar.emit(ConnectionSidebarMsg::Select(0));
    sidebar.emit(ConnectionSidebarMsg::RequestDelete);
    sidebar.emit(ConnectionSidebarMsg::CommandRejected(UiPortError::Busy));
    assert!(flush_gtk());

    let mut updated = original;
    updated.name = "Retained row updated".into();
    let mut same_identity = ConnectionCatalog::default();
    same_identity.connections.insert(original_id, updated);
    sidebar.emit(ConnectionSidebarMsg::SetCatalog(same_identity));
    assert!(flush_gtk());
    assert_eq!(selected_navigation_rows(sidebar.widget()), 1);
    for tooltip in ["Edit selection", "Duplicate connection", "Delete selection"] {
        assert!(button_by_tooltip(sidebar.widget(), tooltip).is_sensitive());
    }
    assert_sidebar_transients_cleared(sidebar.widget());

    sidebar.emit(ConnectionSidebarMsg::RequestDelete);
    sidebar.emit(ConnectionSidebarMsg::CommandRejected(UiPortError::Busy));
    assert!(flush_gtk());
    let replacement = ConnectionProfile::new("Different row", "different.example.test");
    let mut different_identity = ConnectionCatalog::default();
    different_identity
        .connections
        .insert(replacement.id, replacement);
    sidebar.emit(ConnectionSidebarMsg::SetCatalog(different_identity));
    assert!(flush_gtk());
    assert_eq!(selected_navigation_rows(sidebar.widget()), 0);
    for tooltip in [
        "Select a connection or group to edit",
        "Select a connection to duplicate",
        "Select a connection or group to delete",
    ] {
        assert!(!button_by_tooltip(sidebar.widget(), tooltip).is_sensitive());
    }
    assert_sidebar_transients_cleared(sidebar.widget());
}

fn assert_catalog_rebuild_then_identity_switch_quiesces() {
    let first = ConnectionProfile::new("First row", "first.example.test");
    let first_id = first.id;
    let second = ConnectionProfile::new("Second row", "second.example.test");
    let second_id = second.id;
    let mut initial = ConnectionCatalog::default();
    initial.connections.insert(first_id, first.clone());
    let sidebar = ConnectionSidebar::builder()
        .launch(ConnectionSidebarInit { catalog: initial })
        .detach();
    sidebar.emit(ConnectionSidebarMsg::SelectConnection(first_id));
    assert!(flush_gtk());

    let mut expanded = ConnectionCatalog::default();
    expanded.connections.insert(first_id, first);
    expanded.connections.insert(second_id, second);
    sidebar.emit(ConnectionSidebarMsg::SetCatalog(expanded));
    sidebar.emit(ConnectionSidebarMsg::SelectConnection(second_id));
    assert!(
        flush_gtk(),
        "catalog rebuild plus an identity switch must not create a row-selection feedback loop"
    );
    let selected = descendants(sidebar.widget())
        .into_iter()
        .filter(|widget| widget.has_css_class("navigation-selected"))
        .flat_map(|widget| visible_label_text(&widget))
        .collect::<Vec<_>>();
    assert!(selected.iter().any(|text| text == "Second row"));
}

fn assert_native_workspace_boundary() {
    let fixture = native_workspace_fixture();
    let tab_bar = SessionTabBar::builder()
        .launch(SessionTabBarInit {
            workspace: fixture.view.workspace.clone(),
        })
        .detach();
    let pane_host = PaneHost::builder()
        .launch(PaneHostInit {
            view_model: fixture.view,
            startup_probe: None,
        })
        .detach();
    assert!(
        descendants(tab_bar.widget())
            .iter()
            .any(|widget| widget.has_css_class("terminal-tab"))
    );
    assert!(
        descendants(pane_host.widget())
            .iter()
            .any(|widget| widget.has_css_class("pane-command-row"))
    );
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(tab_bar.widget());
    content.append(pane_host.widget());
    let window = gtk::Window::new();
    window.set_default_size(1_000, 700);
    window.set_child(Some(&content));
    window.present();
    assert!(flush_gtk(), "workspace native window must quiesce");

    let panes = descendants(pane_host.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Paned>().ok())
        .map(|paned| paned.orientation())
        .collect::<Vec<_>>();
    assert_eq!(
        panes,
        [gtk::Orientation::Horizontal, gtk::Orientation::Vertical]
    );
    assert!(
        descendants(pane_host.widget())
            .iter()
            .any(|widget| widget.is::<gtk::DrawingArea>()),
        "connected leaf must own a real TerminalView canvas"
    );
    let labels = visible_label_text(pane_host.widget());
    assert!(labels.iter().any(|label| label == "Connecting"));
    assert!(labels.iter().any(|label| label == "Failed"));

    for tooltip in [
        "New local terminal tab",
        "Split horizontally",
        "Split vertically",
        "Retry",
        "Copy Diagnostics",
        "Close",
    ] {
        let roots: [&gtk::Widget; 2] = [
            tab_bar.widget().upcast_ref(),
            pane_host.widget().upcast_ref(),
        ];
        let button = roots.iter().find_map(|root| {
            descendants(*root).into_iter().find_map(|widget| {
                widget
                    .downcast::<gtk::Button>()
                    .ok()
                    .filter(|button| button.tooltip_text().as_deref() == Some(tooltip))
            })
        });
        let button = button.unwrap_or_else(|| panic!("missing embedded action {tooltip}"));
        assert_eq!(button.accessible_role(), gtk::AccessibleRole::Button);
        assert!(
            descendants(&button)
                .into_iter()
                .any(|widget| widget.has_css_class("product-icon")),
            "{tooltip} must use an embedded product icon"
        );
    }
    assert!(
        visible_label_text(tab_bar.widget())
            .iter()
            .all(|text| !text.contains("symbolic"))
    );
    tab_bar.emit(SessionTabBarMsg::CommandRejected(UiPortError::Busy));
    assert!(flush_gtk());
    assert!(
        visible_label_text(tab_bar.widget())
            .iter()
            .any(|text| text.contains("application is busy"))
    );
    let error_actions = descendants(pane_host.widget())
        .into_iter()
        .find(|widget| widget.has_css_class("pane-error-actions"))
        .expect("failed pane actions");
    let labels = descendants(&error_actions)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .filter_map(|button| {
            descendants(&button)
                .into_iter()
                .find_map(|widget| widget.downcast::<gtk::Label>().ok())
                .map(|label| label.text().to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        ["Retry", "Edit Connection", "Copy Diagnostics", "Close"]
    );

    let other_tab = descendants(tab_bar.widget())
        .into_iter()
        .find_map(|widget| {
            widget
                .downcast::<gtk::Button>()
                .ok()
                .filter(|button| button.tooltip_text().as_deref() == Some("Activate Other tab"))
        })
        .expect("second tab activation button");
    other_tab.emit_clicked();
    assert!(flush_gtk());
    assert!(
        descendants(tab_bar.widget()).into_iter().any(|widget| {
            widget.downcast::<gtk::Button>().is_ok_and(|button| {
                button.tooltip_text().as_deref() == Some("Activate Other tab")
                    && button.has_css_class("active-tab")
            })
        }),
        "tab click must update the local active-tab surface"
    );
    pane_host.emit(PaneHostMsg::ActivateTab(fixture.other_tab));
    assert!(flush_gtk());

    pane_host.emit(PaneHostMsg::ActivateTab(fixture.first_tab));
    pane_host.emit(PaneHostMsg::SessionEvent {
        session: fixture.pending_session,
        event: SessionUiEvent::Failed(SessionFailure::Timeout),
    });
    assert!(flush_gtk());
    assert!(
        visible_label_text(pane_host.widget())
            .iter()
            .filter(|label| label.as_str() == "Failed")
            .count()
            >= 2,
        "pending leaf must transition to an actionable error page"
    );
    window.close();
    assert!(flush_gtk(), "workspace native window close must quiesce");
}

struct NativeWorkspaceFixture {
    view: AppViewModel,
    first_tab: TabId,
    other_tab: TabId,
    pending_session: SessionId,
}

fn native_workspace_fixture() -> NativeWorkspaceFixture {
    let terminal_pane = PaneId::new();
    let terminal_session = SessionId::new();
    let pending_pane = PaneId::new();
    let pending_session = SessionId::new();
    let error_pane = PaneId::new();
    let error_session = SessionId::new();
    let other_pane = PaneId::new();
    let other_session = SessionId::new();
    let first_tab = TabId::new_v4();
    let other_tab = TabId::new_v4();
    let mut tree = PaneTree::with_session(terminal_pane, terminal_session)
        .split(terminal_pane, SplitAxis::Horizontal, pending_pane, 0.5)
        .unwrap()
        .split(pending_pane, SplitAxis::Vertical, error_pane, 0.5)
        .unwrap();
    tree.replace_session(pending_pane, Some(pending_session))
        .unwrap();
    tree.replace_session(error_pane, Some(error_session))
        .unwrap();
    let workspace = WorkspaceState {
        tabs: vec![
            TabState {
                id: first_tab,
                title: "Nested".into(),
                pane_tree: tree,
                active_pane: terminal_pane,
            },
            TabState {
                id: other_tab,
                title: "Other".into(),
                pane_tree: PaneTree::with_session(other_pane, other_session),
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
    for pane in [terminal_pane, pending_pane, other_pane] {
        view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    }
    let connection = ConnectionProfile::new("Managed", "safe.example.test");
    let connection_id = connection.id;
    view.catalog.connections.insert(connection_id, connection);
    view.pane_launches.insert(
        error_pane,
        PaneLaunchTarget::Connection {
            id: connection_id,
            host: "safe.example.test".into(),
        },
    );
    for (session, state) in [
        (terminal_session, SessionState::Connected),
        (pending_session, SessionState::Connecting),
        (error_session, SessionState::Failed),
        (other_session, SessionState::Connected),
    ] {
        view.session_states.insert(session, state);
    }
    view.error_panes.insert(
        error_session,
        ErrorPaneView {
            failure: SessionFailure::Authentication,
            diagnostic: "session failed",
            host: Some("safe.example.test".into()),
            timestamp_unix_seconds: 1_785_632_400,
        },
    );
    NativeWorkspaceFixture {
        view,
        first_tab,
        other_tab,
        pending_session,
    }
}

fn assert_terminal_view_native_boundary() {
    let terminal = TerminalView::builder()
        .launch(TerminalViewInit {
            session: SessionId::new(),
            profile: TerminalSettingsV1::default().resolve(&TerminalOverrides::default()),
            metrics: FontMetrics::new(9.0, 18.0).unwrap(),
        })
        .detach();
    let window = gtk::Window::new();
    window.set_child(Some(terminal.widget()));
    window.present();
    assert!(flush_gtk(), "terminal native window must quiesce");
    assert!(terminal.widget().has_css_class("terminal-view"));
    let widgets = descendants(terminal.widget());
    let canvas = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<gtk::DrawingArea>().ok())
        .expect("terminal component must contain its native drawing canvas");
    assert!(canvas.is_focusable());
    assert!(canvas.has_css_class("terminal-canvas"));
    let search = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<gtk::SearchEntry>().ok())
        .expect("terminal component must contain its native search entry");
    assert!(!search.is_visible());

    terminal.emit(TerminalViewMsg::ApplyFrame(Arc::new(RenderFrame {
        generation: 1,
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
        title: "native terminal fixture".into(),
        alternate_screen: false,
        mouse_reporting: false,
    })));
    terminal.emit(TerminalViewMsg::Key {
        key: gtk::gdk::Key::from_name("f").unwrap(),
        state: gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
    });
    assert!(flush_gtk(), "terminal component messages must quiesce");
    assert!(search.is_visible());
    window.close();
    assert!(flush_gtk(), "terminal native window close must quiesce");
}

fn flush_gtk() -> bool {
    let context = gtk::glib::MainContext::default();
    for _ in 0..512 {
        if !context.iteration(false) {
            return true;
        }
    }
    false
}

fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    fn push_children(widget: &gtk::Widget, output: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(current) = child {
            output.push(current.clone());
            push_children(&current, output);
            child = current.next_sibling();
        }
    }

    let mut output = Vec::new();
    push_children(root.as_ref(), &mut output);
    output
}

fn button_by_tooltip(root: &impl IsA<gtk::Widget>, tooltip: &str) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.tooltip_text().as_deref() == Some(tooltip))
        .unwrap_or_else(|| panic!("missing button with tooltip {tooltip}"))
}

fn selected_navigation_rows(root: &impl IsA<gtk::Widget>) -> usize {
    descendants(root)
        .into_iter()
        .filter(|widget| widget.has_css_class("navigation-selected"))
        .count()
}

fn assert_sidebar_transients_cleared(root: &impl IsA<gtk::Widget>) {
    assert!(
        descendants(root).into_iter().all(|widget| {
            !widget
                .downcast::<gtk::Revealer>()
                .is_ok_and(|revealer| revealer.reveals_child())
        }),
        "catalog replacement must close destructive confirmation"
    );
    assert!(
        visible_label_text(root)
            .iter()
            .all(|text| !text.contains("application is busy")),
        "catalog replacement must clear transient command errors"
    );
}

fn visible_label_text(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .filter(|label| label.is_visible())
        .map(|label| label.text().into())
        .collect()
}
