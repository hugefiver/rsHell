#![cfg(not(target_os = "macos"))]

use std::{
    cell::RefCell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
};
use rshell_core::{
    AppBootstrapState, AppViewModel, AuthenticationKind, ConnectionCatalog, ConnectionProfile,
    DisplayRecoveryNotice, ErrorPaneView, PaneId, PaneLaunchTarget, PaneTree, RenderFrame,
    SessionFailure, SessionId, SessionState, SessionUiCommand, SessionUiEvent, SplitAxis, TabId,
    TabState, TerminalDisplayModes, TerminalOverrides, TerminalProfile, TerminalSettingsV1,
    TerminalSize, TransportKind, UiCommand, UiPortError, WorkspaceState,
};
use rshell_ui::{
    ConnectionEditor, ConnectionEditorInit, ConnectionEditorMsg, ConnectionSidebar,
    ConnectionSidebarInit, ConnectionSidebarMsg, FontMetricEnvironment, FontMetricsService,
    MetricsChange, PaneHost, PaneHostInit, PaneHostMsg, PaneHostOutput, SessionTabBar,
    SessionTabBarInit, SessionTabBarMsg, StartupProbe, TerminalView, TerminalViewInit,
    TerminalViewMsg, TerminalViewOutput,
};

#[test]
fn twenty_tabs_are_keyboard_and_overflow_reachable() {
    if let Err(error) = gtk::init() {
        eprintln!("native GTK reducer regression skipped: {error}");
        return;
    }
    assert_twenty_tab_overflow_and_keyboard_reachability();
    assert_terminal_view_native_boundary();
    assert_post_render_terminal_geometry_settles_once_after_zero_pixel_frame();
    assert_pane_host_acknowledges_positive_geometry_after_reparent();
    assert_native_workspace_boundary();
    assert_sidebar_catalog_refresh_preserves_only_the_same_visible_identity();
    assert_catalog_rebuild_then_identity_switch_quiesces();

    let editor = ConnectionEditor::builder()
        .launch(ConnectionEditorInit {
            terminal_profiles: vec![TerminalProfile::default()],
        })
        .detach();
    assert!(editor.widget().has_css_class("editor-dialog"));
    assert!(editor.widget().has_css_class("content-dialog"));
    assert_eq!(editor.widget().width_request(), -1);
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

fn assert_twenty_tab_overflow_and_keyboard_reachability() {
    let tabs = (0..20)
        .map(|index| {
            let pane = PaneId::new();
            TabState {
                id: TabId::new_v4(),
                title: format!("Tab {:02}", index + 1),
                pane_tree: PaneTree::leaf(pane),
                active_pane: pane,
            }
        })
        .collect::<Vec<_>>();
    let tab_ids = tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();
    let tab_bar = SessionTabBar::builder()
        .launch(SessionTabBarInit {
            workspace: WorkspaceState {
                tabs,
                active_tab: Some(tab_ids[0]),
            },
        })
        .detach();
    let window = gtk::Window::new();
    window.set_default_size(520, 180);
    window.set_child(Some(tab_bar.widget()));
    window.present();
    assert!(flush_gtk());

    let scroll = descendants(tab_bar.widget())
        .into_iter()
        .find(|widget| widget.has_css_class("tab-strip-scroll"))
        .and_then(|widget| widget.downcast::<gtk::ScrolledWindow>().ok())
        .expect("tab strip horizontal scroller");
    assert_eq!(scroll.vscrollbar_policy(), gtk::PolicyType::Never);
    let scroll_required = scroll.hadjustment().upper() > scroll.hadjustment().page_size();
    let tab_widgets = descendants(tab_bar.widget())
        .into_iter()
        .filter(|widget| widget.has_css_class("terminal-tab"))
        .collect::<Vec<_>>();
    let visible_tabs = tab_widgets
        .iter()
        .filter(|widget| {
            widget.compute_bounds(&scroll).is_some_and(|bounds| {
                bounds.x() >= 0.0
                    && bounds.width() > 0.0
                    && bounds.x() + bounds.width() <= scroll.width() as f32
            })
        })
        .count();
    assert!(
        scroll_required || (!tab_widgets.is_empty() && visible_tabs == tab_widgets.len()),
        "rendered tabs must either require scrolling or all fit visibly: visible={visible_tabs}, rendered={}",
        tab_widgets.len()
    );
    let overflow_menu = descendants(tab_bar.widget())
        .into_iter()
        .filter(|widget| widget.has_css_class("tab-overflow"))
        .find_map(|widget| widget.downcast::<gtk::MenuButton>().ok())
        .expect("accessible tab-overflow menu button");
    let overflow_popover = overflow_menu.popover().expect("tab-overflow popover");
    let overflow_titles = descendants(&overflow_popover)
        .into_iter()
        .filter(|widget| widget.has_css_class("tab-overflow-row"))
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .filter_map(|button| button.label().map(|label| label.to_string()))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(overflow_titles.len(), 17);
    let last_overflow_row = descendants(&overflow_popover)
        .into_iter()
        .filter(|widget| widget.has_css_class("tab-overflow-row"))
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.label().as_deref() == Some("Tab 20"))
        .expect("last authoritative overflow row");
    last_overflow_row.emit_clicked();
    assert!(flush_gtk());
    assert!(descendants(tab_bar.widget()).into_iter().any(|widget| {
        widget.downcast::<gtk::Button>().is_ok_and(|button| {
            button.has_css_class("active-tab")
                && button.tooltip_text().as_deref() == Some("Activate Tab 20 tab")
        })
    }));

    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..20 {
        let active = descendants(tab_bar.widget())
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.has_css_class("active-tab"))
            .and_then(|button| button.tooltip_text())
            .expect("active tab tooltip");
        visited.insert(active.to_string());
        assert!(press_key(
            tab_bar.widget(),
            gtk::gdk::Key::Tab,
            gtk::gdk::ModifierType::CONTROL_MASK,
        ));
        assert!(flush_gtk());
    }
    assert_eq!(visited.len(), 20);
    assert!(press_key(
        tab_bar.widget(),
        gtk::gdk::Key::Tab,
        gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
    ));
    assert!(flush_gtk());
    assert!(
        wait_for_gtk(|| {
            let adjustment = scroll.hadjustment();
            adjustment.upper() <= adjustment.page_size() || adjustment.value() > 0.0
        }),
        "active tab must auto-reveal when scrolling is required"
    );
    window.close();
    assert!(flush_gtk());
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
    let refreshed_view = fixture.view.clone();
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
    assert_positive_pane_allocation(pane_host.widget());

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
    let recovery_rows = descendants(pane_host.widget())
        .into_iter()
        .filter(|widget| widget.has_css_class("display-recovery-notice"))
        .collect::<Vec<_>>();
    assert_eq!(recovery_rows.len(), 1);
    assert_eq!(
        visible_label_text(&recovery_rows[0])
            .into_iter()
            .filter(|label| label == "Display mode not restored")
            .count(),
        1
    );
    let reset_buttons = descendants(pane_host.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .filter(|button| button.tooltip_text().as_deref() == Some("Reset display"))
        .collect::<Vec<_>>();
    assert_eq!(reset_buttons.len(), 1);
    assert_eq!(
        reset_buttons[0].accessible_role(),
        gtk::AccessibleRole::Button
    );
    assert!(
        descendants(pane_host.widget())
            .into_iter()
            .filter(|widget| widget.has_css_class("pane-command-row"))
            .flat_map(|toolbar| descendants(&toolbar))
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .all(|button| button.tooltip_text().as_deref() != Some("Reset display"))
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

    pane_host.emit(PaneHostMsg::SessionEvent {
        session: fixture.terminal_session,
        event: SessionUiEvent::Failed(SessionFailure::Network),
    });
    assert!(flush_gtk());
    assert!(
        descendants(pane_host.widget())
            .into_iter()
            .all(|widget| !widget.is::<gtk::DrawingArea>()),
        "terminal completion must replace the terminal widget"
    );
    assert!(
        descendants(pane_host.widget())
            .into_iter()
            .all(|widget| !widget.has_css_class("display-recovery-notice"))
    );
    assert!(
        visible_label_text(pane_host.widget())
            .iter()
            .all(|label| !label.contains('\u{fffd}'))
    );
    pane_host.emit(PaneHostMsg::SessionEvent {
        session: fixture.terminal_session,
        event: SessionUiEvent::Frame(Arc::new(RenderFrame {
            generation: 2,
            size: TerminalSize {
                cols: 80,
                rows: 24,
                pixel_width: 0,
                pixel_height: 0,
                dpi: 96,
            },
            viewport_top: 0,
            rows: Arc::from([]),
            cursor: None,
            title: "stale terminal frame".into(),
            display_modes: Default::default(),
            alternate_screen: false,
            mouse_reporting: false,
        })),
    });
    assert!(flush_gtk());
    assert!(
        descendants(pane_host.widget())
            .into_iter()
            .all(|widget| !widget.is::<gtk::DrawingArea>()),
        "detached terminal controllers must not be recreated by later frames"
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
    pane_host.emit(PaneHostMsg::SetViewModel(Box::new(refreshed_view)));
    assert!(flush_gtk());
    assert_positive_pane_allocation(pane_host.widget());
    assert!(
        descendants(pane_host.widget())
            .iter()
            .any(|widget| widget.has_css_class("terminal-canvas")),
        "switching tabs must synchronize the newly active real TerminalView"
    );

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

fn assert_positive_pane_allocation(root: &impl IsA<gtk::Widget>) {
    assert!(
        wait_for_gtk(|| {
            let panes = descendants(root)
                .into_iter()
                .filter(|widget| widget.is_mapped() && widget.has_css_class("pane-surface"))
                .collect::<Vec<_>>();
            !panes.is_empty()
                && panes
                    .iter()
                    .all(|pane| pane.width() > 0 && pane.height() > 0)
        }),
        "mapped pane surfaces must settle with positive allocations"
    );
}

fn wait_for_gtk(mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        flush_gtk();
        if condition() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct NativeWorkspaceFixture {
    view: AppViewModel,
    first_tab: TabId,
    other_tab: TabId,
    terminal_session: SessionId,
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
    view.display_recovery.insert(
        terminal_session,
        DisplayRecoveryNotice {
            interrupted_generation: 1,
            observed_generation: 2,
            modes: TerminalDisplayModes {
                alternate_screen: true,
                enhanced_keyboard: true,
                ..TerminalDisplayModes::default()
            },
        },
    );
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
        terminal_session,
        pending_session,
    }
}

fn assert_terminal_view_native_boundary() {
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let metric_probe = gtk::Label::new(None);
    let context = metric_probe.pango_context();
    let environment =
        FontMetricEnvironment::from_context(&context, f64::from(metric_probe.scale_factor()))
            .expect("native metric environment");
    let metrics = match FontMetricsService::default()
        .measure(&context, &profile, environment)
        .expect("native terminal metrics")
    {
        MetricsChange::Changed(metrics) | MetricsChange::Unchanged(metrics) => metrics,
    };
    let terminal = TerminalView::builder()
        .launch(TerminalViewInit {
            pane: PaneId::new(),
            session: SessionId::new(),
            profile,
            metrics,
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
        display_modes: Default::default(),
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

#[derive(Debug)]
enum GeometryHarnessMsg {
    Terminal(TerminalViewOutput),
    ApplyFrame(Arc<RenderFrame>),
    RefreshGeometry,
    AcknowledgeGeometry(TerminalSize),
}

struct GeometryHarnessInit {
    terminal: TerminalViewInit,
    sizes: Rc<RefCell<Vec<TerminalSize>>>,
}

struct GeometryHarness {
    terminal: Controller<TerminalView>,
    sizes: Rc<RefCell<Vec<TerminalSize>>>,
}

struct GeometryHarnessWidgets;

#[derive(Debug)]
enum PaneGeometryHarnessMsg {
    Pane(PaneHostOutput),
    RefreshUnacknowledgedGeometry,
    SetViewModel(Box<AppViewModel>),
}

struct PaneGeometryHarnessInit {
    view: AppViewModel,
    probe: StartupProbe,
    sizes: Rc<RefCell<Vec<TerminalSize>>>,
    rendered_sessions: Rc<RefCell<Vec<Option<SessionId>>>>,
}

struct PaneGeometryHarness {
    host: Controller<PaneHost>,
    sizes: Rc<RefCell<Vec<TerminalSize>>>,
    rendered_sessions: Rc<RefCell<Vec<Option<SessionId>>>>,
}

struct PaneGeometryHarnessWidgets;

impl SimpleComponent for PaneGeometryHarness {
    type Init = PaneGeometryHarnessInit;
    type Input = PaneGeometryHarnessMsg;
    type Output = ();
    type Root = gtk::Box;
    type Widgets = PaneGeometryHarnessWidgets;

    fn init_root() -> Self::Root {
        gtk::Box::new(gtk::Orientation::Vertical, 0)
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let host = PaneHost::builder()
            .launch(PaneHostInit {
                view_model: init.view,
                startup_probe: Some(init.probe),
            })
            .forward(sender.input_sender(), PaneGeometryHarnessMsg::Pane);
        root.append(host.widget());
        ComponentParts {
            model: Self {
                host,
                sizes: init.sizes,
                rendered_sessions: init.rendered_sessions,
            },
            widgets: PaneGeometryHarnessWidgets,
        }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            PaneGeometryHarnessMsg::Pane(PaneHostOutput::Command(command)) => {
                if let UiCommand::Session {
                    command: SessionUiCommand::Resize(size),
                    ..
                } = *command
                {
                    self.sizes.borrow_mut().push(size);
                }
            }
            PaneGeometryHarnessMsg::Pane(PaneHostOutput::RenderedSession(session)) => {
                self.rendered_sessions.borrow_mut().push(session);
            }
            PaneGeometryHarnessMsg::Pane(_) => {}
            PaneGeometryHarnessMsg::RefreshUnacknowledgedGeometry => {
                self.host.emit(PaneHostMsg::RefreshUnacknowledgedGeometry);
            }
            PaneGeometryHarnessMsg::SetViewModel(view) => {
                self.host.emit(PaneHostMsg::SetViewModel(view));
            }
        }
    }
}

impl SimpleComponent for GeometryHarness {
    type Init = GeometryHarnessInit;
    type Input = GeometryHarnessMsg;
    type Output = ();
    type Root = gtk::Box;
    type Widgets = GeometryHarnessWidgets;

    fn init_root() -> Self::Root {
        gtk::Box::new(gtk::Orientation::Vertical, 0)
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let terminal = TerminalView::builder()
            .launch(init.terminal)
            .forward(sender.input_sender(), GeometryHarnessMsg::Terminal);
        root.append(terminal.widget());
        ComponentParts {
            model: Self {
                terminal,
                sizes: init.sizes,
            },
            widgets: GeometryHarnessWidgets,
        }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            GeometryHarnessMsg::Terminal(TerminalViewOutput::Command(command)) => {
                if let UiCommand::Session {
                    command: SessionUiCommand::Resize(size),
                    ..
                } = *command
                {
                    self.sizes.borrow_mut().push(size);
                }
            }
            GeometryHarnessMsg::Terminal(_) => {}
            GeometryHarnessMsg::ApplyFrame(frame) => {
                self.terminal.emit(TerminalViewMsg::ApplyFrame(frame));
            }
            GeometryHarnessMsg::RefreshGeometry => {
                self.terminal.emit(TerminalViewMsg::RefreshGeometry);
            }
            GeometryHarnessMsg::AcknowledgeGeometry(size) => {
                self.terminal
                    .emit(TerminalViewMsg::GeometryAcknowledged(size));
            }
        }
    }
}

fn assert_post_render_terminal_geometry_settles_once_after_zero_pixel_frame() {
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let metric_probe = gtk::Label::new(None);
    let context = metric_probe.pango_context();
    let environment =
        FontMetricEnvironment::from_context(&context, f64::from(metric_probe.scale_factor()))
            .expect("native metric environment");
    let metrics = match FontMetricsService::default()
        .measure(&context, &profile, environment)
        .expect("native terminal metrics")
    {
        MetricsChange::Changed(metrics) | MetricsChange::Unchanged(metrics) => metrics,
    };
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let session = SessionId::new();
    let terminal = GeometryHarness::builder()
        .launch(GeometryHarnessInit {
            terminal: TerminalViewInit {
                pane: PaneId::new(),
                session,
                profile,
                metrics,
            },
            sizes: Rc::clone(&sizes),
        })
        .detach();
    let canvas = descendants(terminal.widget())
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::DrawingArea>().ok())
        .expect("terminal geometry canvas");
    assert_eq!(canvas.width(), 0);
    assert_eq!(canvas.height(), 0);
    assert!(
        canvas.has_css_class("terminal-geometry-pending"),
        "initial geometry must remain pending before model confirmation"
    );
    terminal.emit(GeometryHarnessMsg::ApplyFrame(zero_pixel_frame(1)));
    assert!(flush_gtk(), "unmapped zero-pixel frame must quiesce");
    assert!(sizes.borrow().is_empty());

    let window = gtk::Window::new();
    window.set_default_size(640, 360);
    window.set_child(Some(terminal.widget()));
    window.present();
    assert!(wait_for_gtk(|| {
        canvas.is_mapped()
            && canvas.width() > 0
            && canvas.height() > 0
            && !sizes.borrow().is_empty()
    }));
    terminal.emit(GeometryHarnessMsg::ApplyFrame(zero_pixel_frame(2)));
    assert!(
        flush_gtk(),
        "delayed frame sync must not leave a busy geometry retry"
    );
    let emitted = sizes.borrow().clone();
    assert_eq!(
        emitted.len(),
        1,
        "one measured resize must reach output before acknowledgement"
    );
    assert!(
        emitted[0].pixel_width > 0 && emitted[0].pixel_height > 0,
        "emitted geometry must have positive physical dimensions"
    );
    assert!(
        canvas.has_css_class("terminal-geometry-pending"),
        "output acceptance before the host round trip must keep geometry pending"
    );

    terminal.emit(GeometryHarnessMsg::AcknowledgeGeometry(emitted[0]));
    assert!(wait_for_gtk(|| {
        !canvas.has_css_class("terminal-geometry-pending")
    }));
    let acknowledged_count = sizes.borrow().len();

    terminal.emit(GeometryHarnessMsg::RefreshGeometry);
    terminal.emit(GeometryHarnessMsg::RefreshGeometry);
    terminal.emit(GeometryHarnessMsg::ApplyFrame(zero_pixel_frame(3)));
    assert!(flush_gtk());
    assert_eq!(
        sizes.borrow().len(),
        acknowledged_count,
        "duplicate refreshes are deduped only after round-trip acknowledgement"
    );
    window.close();
    assert!(flush_gtk(), "geometry retry window close must quiesce");
}

fn zero_pixel_frame(generation: u64) -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation,
        size: TerminalSize {
            cols: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([]),
        cursor: None,
        title: "delayed geometry fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

fn assert_pane_host_acknowledges_positive_geometry_after_reparent() {
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Geometry".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);

    let probe = StartupProbe::new();
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let rendered_sessions = Rc::new(RefCell::new(Vec::new()));
    let host = PaneGeometryHarness::builder()
        .launch(PaneGeometryHarnessInit {
            view: view.clone(),
            probe: probe.clone(),
            sizes: Rc::clone(&sizes),
            rendered_sessions: Rc::clone(&rendered_sessions),
        })
        .detach();
    assert_eq!(host.widget().width(), 0);
    assert!(
        descendants(host.widget())
            .iter()
            .any(|widget| widget.has_css_class("pane-geometry-pending")),
        "PaneHost must await real terminal geometry before mapping"
    );

    let reparent = gtk::Box::new(gtk::Orientation::Vertical, 0);
    reparent.append(host.widget());
    let window = gtk::Window::new();
    window.set_default_size(640, 360);
    window.set_child(Some(&reparent));
    window.present();
    let initial_geometry_ready = wait_for_gtk(|| {
        sizes.borrow().len() == 1
            && probe.report(false).measured_terminal_geometry_ready
            && !descendants(host.widget())
                .iter()
                .any(|widget| widget.has_css_class("pane-geometry-pending"))
    });
    assert!(
        initial_geometry_ready,
        "round-trip geometry did not settle once: sizes={:?}, probe={:?}, pane_pending={}",
        sizes.borrow(),
        probe.report(false),
        descendants(host.widget())
            .iter()
            .any(|widget| widget.has_css_class("pane-geometry-pending"))
    );
    host.emit(PaneGeometryHarnessMsg::SetViewModel(Box::new(view.clone())));
    assert!(wait_for_gtk(|| {
        rendered_sessions.borrow().last().copied() == Some(Some(session))
    }));
    let emitted = sizes.borrow().clone();
    assert!(
        emitted[0].cols > 0
            && emitted[0].rows > 0
            && emitted[0].pixel_width > 0
            && emitted[0].pixel_height > 0
            && emitted[0].dpi > 0,
        "PaneHost must forward only positive terminal geometry"
    );

    host.emit(PaneGeometryHarnessMsg::RefreshUnacknowledgedGeometry);
    assert!(wait_for_gtk(|| {
        sizes.borrow().len() == 1
            && !descendants(host.widget())
                .iter()
                .any(|widget| widget.has_css_class("pane-geometry-pending"))
    }));

    let replacement = SessionId::new();
    view.workspace.tabs[0]
        .pane_tree
        .replace_session(pane, Some(replacement))
        .expect("replace bound session");
    view.session_states.remove(&session);
    view.session_states
        .insert(replacement, SessionState::Connected);
    host.emit(PaneGeometryHarnessMsg::SetViewModel(Box::new(view)));
    assert!(wait_for_gtk(|| {
        rendered_sessions.borrow().last().copied() == Some(Some(replacement))
            && sizes.borrow().len() == 2
            && !descendants(host.widget())
                .iter()
                .any(|widget| widget.has_css_class("pane-geometry-pending"))
    }));
    window.close();
    assert!(wait_for_gtk(|| !window.is_visible()));
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

fn press_key(
    root: &impl IsA<gtk::Widget>,
    key: gtk::gdk::Key,
    state: gtk::gdk::ModifierType,
) -> bool {
    let controllers = root.observe_controllers();
    let controller = (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .find_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .expect("key controller");
    controller.emit_by_name::<bool>("key-pressed", &[&key, &0u32, &state])
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
