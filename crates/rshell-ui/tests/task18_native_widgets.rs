#![cfg(not(target_os = "macos"))]

use std::{
    cell::RefCell,
    collections::VecDeque,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppEvent, AppSettings, AppViewModel, AuthPrompt, CatalogMutation,
    ColorScheme, ConnectionId, ConnectionProfile, HostKeyPrompt, ImportCandidateId,
    ImportCandidateView, ImportPreviewId, ImportPreviewView, ImportSourceKind, ImportWarningView,
    InteractionId, InteractionRequest, KeyBinding, KeyCode, KeyModifiers, PaneId, PaneLaunchTarget,
    PaneTree, SessionId, SessionState, SessionUiEvent, SplitAxis, TabId, TabState, TerminalProfile,
    UiCommand, UiCommandPort, UiPortError, WorkspaceState,
};
use rshell_platform::{FileSelectionCallback, FileSelectionRequest, FileSelectionService};
use rshell_ui::{
    ImportDialog, ImportDialogInit, ImportDialogMsg, InteractionAction, InteractionDialog,
    InteractionDialogInit, InteractionDialogMsg, MainWindow, MainWindowInit, MainWindowMsg,
    SettingsWindow, SettingsWindowInit, SettingsWindowMsg, ShellLayout, ShellLayoutMode,
};

fn assert_task18_dialogs_present_real_accessible_widgets_and_wipe_native_secrets() {
    assert_main_shell_surface();
    assert_saved_connection_row_activation_connects_the_active_pane();
    assert_settings_surface();
    assert_import_surface();
    assert_interaction_surface();
    assert_stale_file_selection_callbacks();
    assert_expanded_connection_overrides();
    assert_exact_interaction_handshake();
    assert_terminal_controller_ingress();
}

#[test]
fn task18_native_contracts_run_on_one_gtk_thread() {
    if let Err(error) = gtk::init() {
        eprintln!("Task 18 native contracts skipped: {error}");
        return;
    }
    let settings = gtk::Settings::default().expect("GTK settings");
    settings.set_property("gtk-cursor-blink", false);
    assert!(!settings.property::<bool>("gtk-cursor-blink"));
    assert_task18_dialogs_present_real_accessible_widgets_and_wipe_native_secrets();
    assert_adaptive_recursive_pane_fixtures();
    assert_adaptive_command_bar();
    let commands = Arc::new(RecordingPort::default());
    let connection = ConnectionProfile::new("Drawer connection", "drawer.example.test");
    let connection_id = connection.id;
    let pane = PaneId::new();
    let tab = TabId::new_v4();
    let mut view = view_with_profiles(vec![TerminalProfile::default()]);
    view.catalog.connections.insert(connection_id, connection);
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Drawer".into(),
        pane_tree: PaneTree::leaf(pane),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    let main = MainWindow::builder()
        .launch(
            MainWindowInit::new(commands.clone(), view)
                .with_file_selection(Rc::new(CancelSelection)),
        )
        .detach();
    let window = present_main(&main, 800, 600);
    main.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());

    let rail = css_child(main.widget(), "compact-nav-rail");
    assert_eq!(rail.width_request(), 48);
    for tooltip in ["Navigation", "New connection", "New group"] {
        let action = descendants(&rail)
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.tooltip_text().as_deref() == Some(tooltip))
            .unwrap_or_else(|| panic!("missing compact rail action {tooltip}"));
        assert!(action.is_focusable());
    }
    let navigation = descendants(&rail)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.tooltip_text().as_deref() == Some("Navigation"))
        .unwrap();
    let sidebar = css_child(main.widget(), "sidebar");
    assert!(!sidebar.is_visible());
    navigation.emit_clicked();
    assert!(flush_gtk());
    assert!(sidebar.is_visible());
    let search = css_child(&sidebar, "connection-search");
    assert!(
        focus_is_within(&search),
        "opening the drawer focuses search"
    );

    assert!(press_key(
        &sidebar,
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty(),
    ));
    assert!(flush_gtk());
    assert!(!sidebar.is_visible());
    assert!(navigation.has_focus(), "Escape restores Navigation focus");

    navigation.emit_clicked();
    assert!(flush_gtk());
    let list = css_child(&sidebar, "connection-list")
        .downcast::<gtk::ListBox>()
        .expect("drawer connection list");
    let row = list.row_at_index(0).expect("drawer connection row");
    row.emit_by_name::<()>("activate", &[]);
    assert!(flush_gtk());
    assert_eq!(commands.connects(), [(pane, connection_id)]);
    assert!(!sidebar.is_visible(), "delivery precedes drawer close");

    main.emit(MainWindowMsg::Allocated { width: 1_360 });
    assert!(flush_gtk());
    assert!(sidebar.is_visible());
    assert_eq!(
        descendants(main.widget())
            .into_iter()
            .filter(|widget| widget.has_css_class("sidebar"))
            .count(),
        1,
        "breakpoint changes must reattach one existing sidebar widget"
    );
    window.close();
    assert!(flush_gtk());
}

fn assert_adaptive_command_bar() {
    let commands = Arc::new(RecordingPort::default());
    let main = launch_main_with_profiles(
        commands.clone(),
        Rc::new(CancelSelection),
        vec![TerminalProfile::default()],
    );
    let window = present_main(&main, 900, 700);
    main.emit(MainWindowMsg::Allocated { width: 900 });
    assert!(flush_gtk());
    let command_bar = css_child(main.widget(), "command-bar");
    let actions = descendants(&command_bar)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .collect::<Vec<_>>();
    assert_eq!(actions.len(), 3);
    assert_eq!(
        actions
            .iter()
            .map(|button| button.tooltip_text().unwrap().to_string())
            .collect::<Vec<_>>(),
        [
            "New local terminal tab",
            "Import connections",
            "Terminal settings"
        ]
    );
    assert_eq!(
        actions
            .iter()
            .map(|button| visible_button_text(button).unwrap())
            .collect::<Vec<_>>(),
        ["New session", "Import", "Settings"]
    );
    assert!(actions.iter().all(|button| {
        button.accessible_role() == gtk::AccessibleRole::Button
            && descendants(button)
                .iter()
                .any(|child| child.has_css_class("product-icon"))
    }));
    assert!(
        descendants(&command_bar)
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
            .all(|label| label.text().as_str() != "rsHell"),
        "the native title is the sole product identity"
    );
    assert!(
        descendants(main.widget())
            .iter()
            .all(|widget| !widget.has_css_class("status-bar"))
    );
    assert!(
        command_bar
            .last_child()
            .is_some_and(|child| child.has_css_class("command-status"))
    );
    actions[0].emit_clicked();
    assert!(flush_gtk());
    assert_eq!(commands.new_tab_count(), 1);

    window.set_default_size(800, 700);
    main.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());
    assert!(
        wait_for_gtk(|| {
            css_child(main.widget(), "shell-compact").is_visible()
                && actions
                    .iter()
                    .all(|action| visible_button_text(action).is_none())
        }),
        "compact command labels must settle hidden"
    );
    let compact_text = actions
        .iter()
        .filter_map(visible_button_text)
        .collect::<Vec<_>>();
    assert!(
        compact_text.is_empty(),
        "compact command labels remained visible: {compact_text:?}"
    );
    window.close();
    assert!(flush_gtk());
}

fn assert_adaptive_recursive_pane_fixtures() {
    for fixture in AdaptivePaneFixture::ALL {
        for (width, height) in [(800, 600), (1_360, 860), (1_920, 1_080)] {
            let (view, leaves) = fixture.view();
            let main = MainWindow::builder()
                .launch(MainWindowInit::new(
                    Arc::new(RecordingPort::default()),
                    view,
                ))
                .detach();
            let window = present_main(&main, width, height);
            main.emit(MainWindowMsg::Allocated { width });
            assert!(flush_gtk(), "{fixture:?} at {width}x{height}");
            assert!(
                wait_for_gtk(|| {
                    let actual_width = window
                        .surface()
                        .map_or_else(|| window.width(), |surface| surface.width());
                    let class = shell_mode_class(ShellLayout::for_width(actual_width).mode);
                    descendants(main.widget())
                        .into_iter()
                        .any(|widget| widget.has_css_class(class) && widget.is_visible())
                }),
                "{fixture:?} did not settle for the realized surface requested at {width}x{height}"
            );
            let terminals = descendants(main.widget())
                .into_iter()
                .filter(|widget| widget.has_css_class("terminal-canvas"))
                .collect::<Vec<_>>();
            assert_eq!(terminals.len(), leaves, "{fixture:?} at {width}x{height}");
            assert!(
                terminals
                    .iter()
                    .all(|terminal| terminal.width() > 0 && terminal.height() > 0),
                "{fixture:?} has a zero terminal allocation at {width}x{height}"
            );
            for tooltip in [
                "Split horizontally",
                "Split vertically",
                "Reconnect session",
                "Close",
            ] {
                let controls = descendants(main.widget())
                    .into_iter()
                    .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
                    .filter(|button| button.has_css_class("pane-action-btn"))
                    .filter(|button| button.tooltip_text().as_deref() == Some(tooltip))
                    .collect::<Vec<_>>();
                assert_eq!(controls.len(), leaves, "{fixture:?}: {tooltip}");
                assert!(controls.iter().all(|button| {
                    button.accessible_role() == gtk::AccessibleRole::Button
                        && button.tooltip_text().is_some_and(|name| !name.is_empty())
                }));
            }
            window.close();
            assert!(flush_gtk());
        }
    }
}

fn shell_mode_class(mode: ShellLayoutMode) -> &'static str {
    match mode {
        ShellLayoutMode::Compact => "shell-compact",
        ShellLayoutMode::Standard => "shell-standard",
        ShellLayoutMode::Wide => "shell-wide",
    }
}

#[derive(Debug, Clone, Copy)]
enum AdaptivePaneFixture {
    Single,
    HSplit,
    VSplit,
    TopBottom3,
    Grid,
}

impl AdaptivePaneFixture {
    const ALL: [Self; 5] = [
        Self::Single,
        Self::HSplit,
        Self::VSplit,
        Self::TopBottom3,
        Self::Grid,
    ];

    fn view(self) -> (AppViewModel, usize) {
        let leaf_count = match self {
            Self::Single => 1,
            Self::HSplit | Self::VSplit => 2,
            Self::TopBottom3 => 3,
            Self::Grid => 4,
        };
        let panes = (0..leaf_count).map(|_| PaneId::new()).collect::<Vec<_>>();
        let sessions = (0..leaf_count)
            .map(|_| SessionId::new())
            .collect::<Vec<_>>();
        let mut tree = PaneTree::with_session(panes[0], sessions[0]);
        match self {
            Self::Single => {}
            Self::HSplit => tree = split(tree, panes[0], SplitAxis::Horizontal, panes[1]),
            Self::VSplit => tree = split(tree, panes[0], SplitAxis::Vertical, panes[1]),
            Self::TopBottom3 => {
                tree = split(tree, panes[0], SplitAxis::Vertical, panes[1]);
                tree = split(tree, panes[1], SplitAxis::Horizontal, panes[2]);
            }
            Self::Grid => {
                tree = split(tree, panes[0], SplitAxis::Vertical, panes[1]);
                tree = split(tree, panes[0], SplitAxis::Horizontal, panes[2]);
                tree = split(tree, panes[1], SplitAxis::Horizontal, panes[3]);
            }
        }
        for (&pane, &session) in panes.iter().zip(&sessions).skip(1) {
            tree.replace_session(pane, Some(session)).unwrap();
        }
        let tab = TabId::new_v4();
        let mut view = view_with_profiles(vec![TerminalProfile::default()]);
        view.workspace = WorkspaceState {
            tabs: vec![TabState {
                id: tab,
                title: format!("{self:?}"),
                pane_tree: tree,
                active_pane: panes[0],
            }],
            active_tab: Some(tab),
        };
        for (&pane, &session) in panes.iter().zip(&sessions) {
            view.pane_launches.insert(pane, PaneLaunchTarget::Local);
            view.session_states.insert(session, SessionState::Connected);
        }
        (view, leaf_count)
    }
}

fn split(tree: PaneTree, pane: PaneId, axis: SplitAxis, new_pane: PaneId) -> PaneTree {
    tree.split(pane, axis, new_pane, 0.5).unwrap()
}

fn assert_terminal_controller_ingress() {
    let commands = Arc::new(RecordingPort::default());
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let shift = KeyModifiers {
        shift: true,
        ..KeyModifiers::default()
    };
    let reserved = KeyModifiers {
        shift: true,
        control: true,
        ..KeyModifiers::default()
    };
    let mut view = view_with_profiles(vec![TerminalProfile::default()]);
    view.settings.key_bindings = [
        (KeyCode::Character('x'), KeyModifiers::default()),
        (KeyCode::Character('X'), shift),
        (KeyCode::F(13), KeyModifiers::default()),
        (KeyCode::F(24), KeyModifiers::default()),
        (KeyCode::Character('c'), reserved),
        (KeyCode::Character('v'), reserved),
        (KeyCode::Character('f'), reserved),
    ]
    .into_iter()
    .map(|(code, modifiers)| KeyBinding {
        code,
        modifiers,
        action: "new_tab".into(),
    })
    .collect();
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Controller ingress".into(),
        pane_tree: PaneTree::with_session(pane, session),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    let main = MainWindow::builder()
        .launch(
            MainWindowInit::new(commands.clone(), view)
                .with_file_selection(Rc::new(CancelSelection)),
        )
        .detach();
    let window = present_main(&main, 900, 700);
    assert!(flush_gtk());
    let canvas = css_child(main.widget(), "terminal-canvas");

    assert!(press_key(
        &canvas,
        gtk::gdk::Key::from_name("x").unwrap(),
        gtk::gdk::ModifierType::empty()
    ));
    assert!(flush_gtk());
    assert_eq!(commands.new_tab_count(), 1);
    assert!(press_key(
        &canvas,
        gtk::gdk::Key::from_name("X").unwrap(),
        gtk::gdk::ModifierType::SHIFT_MASK
    ));
    assert!(flush_gtk());
    assert_eq!(commands.new_tab_count(), 2);

    assert!(!press_key(
        &canvas,
        gtk::gdk::Key::from_name("z").unwrap(),
        gtk::gdk::ModifierType::empty()
    ));
    assert!(!press_key(
        &canvas,
        gtk::gdk::Key::from_name("Z").unwrap(),
        gtk::gdk::ModifierType::SHIFT_MASK
    ));
    assert!(flush_gtk());
    assert_eq!(
        commands.new_tab_count(),
        2,
        "unbound characters must remain IM-driven"
    );

    assert!(press_key(
        &canvas,
        gtk::gdk::Key::F13,
        gtk::gdk::ModifierType::empty()
    ));
    assert!(press_key(
        &canvas,
        gtk::gdk::Key::F24,
        gtk::gdk::ModifierType::empty()
    ));
    assert!(flush_gtk());
    assert_eq!(commands.new_tab_count(), 4);

    let control_shift = gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK;
    for character in ["c", "v", "f"] {
        assert!(press_key(
            &canvas,
            gtk::gdk::Key::from_name(character).unwrap(),
            control_shift
        ));
    }
    assert!(flush_gtk());
    assert_eq!(
        commands.new_tab_count(),
        4,
        "reserved Ctrl+Shift+C/V/F must win"
    );
    assert!(css_child(main.widget(), "terminal-search").is_visible());

    window.close();
    assert!(flush_gtk());
}

fn assert_saved_connection_row_activation_connects_the_active_pane() {
    let commands = Arc::new(RecordingPort::default());
    let connection = ConnectionProfile::new("Saved connection", "saved.example.test");
    let connection_id = connection.id;
    let pane = PaneId::new();
    let tab = TabId::new_v4();
    let mut view = view_with_profiles(vec![TerminalProfile::default()]);
    view.catalog.connections.insert(connection_id, connection);
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Active".into(),
        pane_tree: PaneTree::with_session(pane, SessionId::new()),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    let main = MainWindow::builder()
        .launch(
            MainWindowInit::new(commands.clone(), view)
                .with_file_selection(Rc::new(CancelSelection)),
        )
        .detach();
    let window = present_main(&main, 900, 700);
    assert!(flush_gtk());

    let list = descendants(main.widget())
        .into_iter()
        .find(|widget| widget.has_css_class("connection-list"))
        .and_then(|widget| widget.downcast::<gtk::ListBox>().ok())
        .expect("real sidebar GtkListBox");
    let row = list.row_at_index(0).expect("saved connection row");
    list.select_row(Some(&row));
    row.emit_by_name::<()>("activate", &[]);
    assert!(flush_gtk());
    assert_eq!(commands.connects(), [(pane, connection_id)]);

    let edit = descendants(main.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.tooltip_text().as_deref() == Some("Edit selection"))
        .expect("enabled sidebar Edit action");
    edit.emit_clicked();
    assert!(flush_gtk());
    assert!(css_child(main.widget(), "editor-dialog").is_visible());
    assert_eq!(
        commands.connects(),
        [(pane, connection_id)],
        "toolbar Edit must not dispatch another Connect"
    );

    window.close();
    assert!(flush_gtk());
}

fn assert_main_shell_surface() {
    let main = launch_main_with_profiles(
        Arc::new(RecordingPort::default()),
        Rc::new(CancelSelection),
        vec![TerminalProfile::default()],
    );
    let window = present_main(&main, 900, 700);
    assert!(flush_gtk());
    for header in descendants(main.widget())
        .into_iter()
        .filter(|widget| widget.is::<gtk::HeaderBar>())
    {
        assert!(
            descendants(&header)
                .into_iter()
                .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
                .all(|button| !matches!(
                    button.tooltip_text().as_deref(),
                    Some("Import connections" | "Terminal settings")
                )),
            "native decoration must not own product command actions"
        );
    }
    assert!(main.widget().has_css_class("fluent-shell"));
    let command_bar = css_child(main.widget(), "command-bar");
    for tooltip in ["Import connections", "Terminal settings"] {
        let action = descendants(&command_bar)
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .find(|button| button.tooltip_text().as_deref() == Some(tooltip))
            .expect("command-bar action");
        assert!(action.is_focusable());
        assert!(
            descendants(&action)
                .iter()
                .any(|child| child.has_css_class("product-icon"))
        );
        action.emit_clicked();
        assert!(flush_gtk());
        let (dialog_class, close_label) = if tooltip == "Import connections" {
            ("import-dialog", "Cancel")
        } else {
            ("settings-window", "Close")
        };
        let dialog = css_child(main.widget(), dialog_class);
        assert!(dialog.is_visible());
        button(&dialog, close_label).emit_clicked();
        assert!(flush_gtk());
        assert!(!dialog.is_visible());
    }
    window.close();
    assert!(flush_gtk());
}

fn assert_stale_file_selection_callbacks() {
    let selections = Rc::new(DelayedSelection::default());
    let commands = Arc::new(RecordingPort::default());
    let view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(commands.clone(), view).with_file_selection(selections.clone()))
        .detach();
    let window = present_main(&main, 900, 700);

    main.emit(MainWindowMsg::OpenImport);
    assert!(flush_gtk());
    let import = css_child(main.widget(), "import-dialog");
    button(&import, "Legacy rsHell JSON").emit_clicked();
    assert!(flush_gtk());
    button(&import, "Cancel").emit_clicked();
    assert!(flush_gtk());
    selections.complete_next(Path::new("closed-legacy.json"));
    assert!(flush_gtk());
    assert_eq!(commands.preview_count(), 0, "closed callback is stale");

    main.emit(MainWindowMsg::OpenImport);
    assert!(flush_gtk());
    button(&import, "Legacy rsHell JSON").emit_clicked();
    assert!(flush_gtk());
    button(&import, "Cancel").emit_clicked();
    main.emit(MainWindowMsg::OpenImport);
    assert!(flush_gtk());
    button(&import, "OpenSSH config").emit_clicked();
    assert!(flush_gtk());

    selections.complete_next(Path::new("old-legacy.json"));
    assert!(flush_gtk());
    assert_eq!(commands.preview_count(), 0, "older generation is stale");
    selections.complete_next(Path::new("current-ssh-config"));
    assert!(flush_gtk());
    assert!(commands.has_preview(
        ImportSourceKind::OpenSshConfig,
        Path::new("current-ssh-config")
    ));
    assert_eq!(commands.preview_count(), 1);

    window.close();
    assert!(flush_gtk());
}

fn assert_expanded_connection_overrides() {
    let commands = Arc::new(RecordingPort::default());
    let mut profile = TerminalProfile::default();
    profile.settings.terminal_type = "base-terminal".into();
    profile.settings.initial_cols = 177;
    profile.settings.color_scheme = ColorScheme::Nord;
    profile.settings.left_alt_as_meta = true;
    let main = launch_main_with_profiles(commands.clone(), Rc::new(CancelSelection), vec![profile]);
    let window = present_main(&main, 900, 800);
    main.emit(MainWindowMsg::Sidebar(
        rshell_ui::ConnectionSidebarOutput::OpenCreate(None),
    ));
    assert!(flush_gtk());
    let editor = css_child(main.widget(), "editor-dialog");
    assert!(editor.has_css_class("content-dialog"));
    assert!(
        descendants(&editor)
            .iter()
            .any(|widget| widget.has_css_class("dialog-footer"))
    );

    let inherits = descendants(&editor)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::CheckButton>().ok())
        .filter(|button| button.label().as_deref() == Some("Inherit") && button.is_visible())
        .collect::<Vec<_>>();
    assert_eq!(inherits.len(), 16);
    assert!(inherits.iter().all(gtk::CheckButton::is_active));

    base_editor_value::<gtk::Entry>(&editor, "Name").set_text("Override host");
    base_editor_value::<gtk::Entry>(&editor, "Host").set_text("override.example.test");
    let terminal = override_editor_value::<gtk::Entry>(&editor, "Terminal type");
    let columns = override_editor_value::<gtk::SpinButton>(&editor, "Columns");
    let scheme = override_editor_value::<gtk::DropDown>(&editor, "Color scheme");
    let alt = override_editor_value::<gtk::CheckButton>(&editor, "Left Alt as Meta");
    let bindings = override_editor_value::<gtk::Entry>(&editor, "Key bindings");

    for label in [
        "Terminal type",
        "Columns",
        "Color scheme",
        "Left Alt as Meta",
        "Key bindings",
    ] {
        override_inherit(&editor, label).set_active(false);
        assert!(flush_gtk());
    }
    assert_eq!(terminal.text().as_str(), "base-terminal");
    assert_eq!(columns.value_as_int(), 177);
    assert_eq!(scheme.selected(), 6);
    assert!(alt.is_active());
    for widget in [
        terminal.clone().upcast::<gtk::Widget>(),
        columns.clone().upcast(),
        scheme.clone().upcast(),
        alt.clone().upcast(),
        bindings.clone().upcast(),
    ] {
        assert!(widget.is_sensitive());
    }

    terminal.set_text("explicit-terminal");
    columns.set_value(211.0);
    scheme.set_selected(8);
    alt.set_active(false);
    bindings.set_text("Ctrl+K=clear_scrollback");
    assert!(flush_gtk());
    button(&editor, "Save connection").emit_clicked();
    assert!(flush_gtk());
    assert!(commands.has_exact_override_profile());
    button(&editor, "Cancel").emit_clicked();
    assert!(flush_gtk());
    assert!(!editor.is_visible());

    window.close();
    assert!(flush_gtk());
}

fn assert_exact_interaction_handshake() {
    let commands = Arc::new(RecordingPort::default());
    let session = SessionId::new();
    let main = launch_main_bound(commands.clone(), Rc::new(CancelSelection), session);
    let window = present_main(&main, 900, 700);
    let first = InteractionId::new();

    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
        session,
        request: password(first, "First password"),
    }));
    assert!(flush_gtk());
    let secret = mapped_password(main.widget());
    secret.set_text("one-shot-secret");
    button(main.widget(), "Submit").emit_clicked();
    assert!(flush_gtk());
    assert!(secret.text().is_empty());

    main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
        session,
        event: SessionUiEvent::State(SessionState::Connected),
    }));
    assert!(flush_gtk());
    assert!(
        mapped_labels(main.widget()).contains(&"First password".into()),
        "stale State must not acknowledge the response"
    );
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
        session,
        interaction: first,
    }));
    assert!(flush_gtk());
    assert!(!mapped_labels(main.widget()).contains(&"First password".into()));

    let old = InteractionId::new();
    let replacement = InteractionId::new();
    let newest = InteractionId::new();
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
        session,
        request: password(old, "Old password"),
    }));
    assert!(flush_gtk());
    commands.reject_next();
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
        session,
        request: password(replacement, "Replacement password"),
    }));
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
        session,
        request: password(newest, "Newest password"),
    }));
    assert!(flush_gtk());
    let labels = mapped_labels(main.widget());
    assert!(labels.contains(&"Old password".into()));
    assert!(!labels.contains(&"Replacement password".into()));
    assert!(!labels.contains(&"Newest password".into()));

    button(main.widget(), "Cancel").emit_clicked();
    assert!(flush_gtk());
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
        session,
        interaction: old,
    }));
    assert!(flush_gtk());
    assert!(
        mapped_labels(main.widget()).contains(&"Replacement password".into()),
        "exact old acknowledgement promotes the queued replacement"
    );
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
        session,
        interaction: replacement,
    }));
    assert!(flush_gtk());
    assert!(
        mapped_labels(main.widget()).contains(&"Newest password".into()),
        "the third request is retained and promoted rather than abandoned"
    );
    button(main.widget(), "Cancel").emit_clicked();
    assert!(flush_gtk());
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
        session,
        interaction: newest,
    }));
    assert!(flush_gtk());
    assert!(!mapped_labels(main.widget()).contains(&"Newest password".into()));

    window.close();
    assert!(flush_gtk());
}

fn launch_main_bound(
    commands: Arc<RecordingPort>,
    file_selection: Rc<dyn FileSelectionService>,
    session: SessionId,
) -> relm4::Controller<MainWindow> {
    let mut view = view_with_profiles(vec![TerminalProfile::default()]);
    let pane = PaneId::new();
    let tab = TabId::new_v4();
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Bound".into(),
        pane_tree: PaneTree::with_session(pane, session),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    MainWindow::builder()
        .launch(MainWindowInit::new(commands, view).with_file_selection(file_selection))
        .detach()
}

fn launch_main_with_profiles(
    commands: Arc<RecordingPort>,
    file_selection: Rc<dyn FileSelectionService>,
    profiles: Vec<TerminalProfile>,
) -> relm4::Controller<MainWindow> {
    let view = view_with_profiles(profiles);
    MainWindow::builder()
        .launch(MainWindowInit::new(commands, view).with_file_selection(file_selection))
        .detach()
}

fn view_with_profiles(profiles: Vec<TerminalProfile>) -> AppViewModel {
    AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: profiles,
    })
}

fn password(id: InteractionId, label: &str) -> InteractionRequest {
    InteractionRequest::Password(AuthPrompt {
        id,
        label: label.into(),
        echo: false,
    })
}

fn assert_settings_surface() {
    let settings = SettingsWindow::builder()
        .launch(SettingsWindowInit {
            settings: AppSettings::default(),
            profiles: vec![TerminalProfile::default()],
        })
        .detach();
    let window = present(settings.widget(), 700, 780);
    settings.emit(SettingsWindowMsg::Open);
    assert!(flush_gtk());
    assert!(settings.widget().has_css_class("settings-window"));
    assert!(settings.widget().has_css_class("content-dialog"));
    assert!(
        descendants(settings.widget())
            .iter()
            .any(|widget| widget.has_css_class("dialog-footer"))
    );
    press_key(
        settings.widget(),
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty(),
    );
    assert!(flush_gtk());
    assert!(
        !settings.widget().is_visible(),
        "Escape must close settings"
    );
    settings.emit(SettingsWindowMsg::Open);
    assert!(flush_gtk());
    assert!(
        descendants(settings.widget())
            .iter()
            .filter(|widget| widget.is::<gtk::SpinButton>())
            .count()
            >= 4
    );
    assert!(
        descendants(settings.widget())
            .iter()
            .filter(|widget| widget.is::<gtk::CheckButton>())
            .count()
            >= 7
    );
    let name = descendants(settings.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Entry>().ok())
        .find(|entry| entry.text().as_str() == "Default")
        .expect("profile name entry");
    name.set_text("");
    assert!(flush_gtk());
    settings.emit(SettingsWindowMsg::SaveProfile);
    assert!(flush_gtk());
    assert!(
        visible_labels(settings.widget())
            .iter()
            .any(|text| { text.contains("invalid terminal setting") })
    );
    window.close();
    assert!(flush_gtk());
}

fn assert_import_surface() {
    let import = ImportDialog::builder()
        .launch(ImportDialogInit {
            file_selection: Rc::new(CancelSelection),
        })
        .detach();
    let window = present(import.widget(), 700, 620);
    import.emit(ImportDialogMsg::Open);
    import.emit(ImportDialogMsg::Preview(import_preview()));
    assert!(flush_gtk());
    assert!(import.widget().has_css_class("import-dialog"));
    assert!(import.widget().has_css_class("content-dialog"));
    assert!(
        descendants(import.widget())
            .iter()
            .any(|widget| widget.has_css_class("dialog-footer"))
    );
    let checks = descendants(import.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::CheckButton>().ok())
        .collect::<Vec<_>>();
    assert_eq!(checks.len(), 2);
    assert_eq!(
        checks.iter().filter(|check| check.is_sensitive()).count(),
        1
    );
    assert!(
        visible_labels(import.widget())
            .iter()
            .any(|text| { text == "Wildcard templates cannot be imported" })
    );
    assert!(descendants(import.widget()).into_iter().any(|widget| {
        widget.has_css_class("product-icon")
            && widget.tooltip_text().as_deref()
                == Some("Secret is present; value is never displayed")
    }));
    import.emit(ImportDialogMsg::Commit);
    assert!(flush_gtk());
    let commit = button(import.widget(), "Import selected");
    assert!(
        !commit.is_sensitive(),
        "pending commit must disable resubmit"
    );
    window.close();
    assert!(flush_gtk());
}

fn assert_interaction_surface() {
    let interaction = InteractionDialog::builder()
        .launch(InteractionDialogInit)
        .detach();
    let window = present(interaction.widget(), 600, 420);
    let changed_session = SessionId::new();
    let changed_interaction = InteractionId::new();
    interaction.emit(InteractionDialogMsg::Open {
        session: changed_session,
        request: InteractionRequest::HostKey(HostKeyPrompt {
            id: changed_interaction,
            host: "server.example.test".into(),
            port: 22,
            algorithm: "ssh-ed25519".into(),
            sha256: "SHA256:changed".into(),
            changed: true,
        }),
    });
    assert!(flush_gtk());
    assert!(
        focus_is_within(interaction.widget().upcast_ref()),
        "new interaction content must receive focus after its controls are rendered"
    );
    assert!(interaction.widget().has_css_class("content-dialog"));
    assert!(
        descendants(interaction.widget())
            .iter()
            .any(|widget| widget.has_css_class("dialog-footer"))
    );
    let labels = button_labels(interaction.widget());
    assert_eq!(labels, ["Copy diagnostics", "Close"]);
    assert!(!labels.iter().any(|label| label.contains("Accept")));

    let password_interaction = InteractionId::new();
    interaction.emit(InteractionDialogMsg::Open {
        session: SessionId::new(),
        request: InteractionRequest::Password(AuthPrompt {
            id: password_interaction,
            label: "Password".into(),
            echo: false,
        }),
    });
    interaction.emit(InteractionDialogMsg::ResponseAccepted(changed_interaction));
    assert!(flush_gtk());
    assert!(
        focus_is_within(interaction.widget().upcast_ref()),
        "queued authentication controls must retain interaction focus"
    );
    let password = descendants(interaction.widget())
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::PasswordEntry>().ok())
        .expect("non-echo prompt must use PasswordEntry");
    password.set_text("native-secret-sentinel");
    assert!(flush_gtk());
    interaction.emit(InteractionDialogMsg::Action(InteractionAction::Submit));
    assert!(flush_gtk());
    assert!(
        password.text().is_empty(),
        "submit must wipe native secret text"
    );
    assert!(!button(interaction.widget(), "Submit").is_sensitive());
    interaction.emit(InteractionDialogMsg::ResponseAccepted(password_interaction));
    assert!(flush_gtk());
    assert!(!interaction.widget().is_visible());
    window.close();
    assert!(flush_gtk());
}

fn import_preview() -> ImportPreviewView {
    let warning = ImportWarningView {
        code: "wildcard_template".into(),
        message: "Wildcard templates cannot be imported".into(),
    };
    ImportPreviewView {
        id: ImportPreviewId::new(),
        source: ImportSourceKind::OpenSshConfig,
        groups: Vec::new(),
        candidates: vec![
            candidate("production", true, true, Vec::new()),
            candidate("*.corp", false, false, vec![warning.clone()]),
        ],
        warnings: vec![warning],
    }
}

fn candidate(
    name: &str,
    selectable: bool,
    has_secret: bool,
    warnings: Vec<ImportWarningView>,
) -> ImportCandidateView {
    ImportCandidateView {
        id: ImportCandidateId::new(),
        name: name.into(),
        host: "host.example.test".into(),
        port: 22,
        username: "operator".into(),
        source_label: name.into(),
        has_secret,
        selectable,
        authentication: rshell_core::AuthenticationKind::Agent,
        credential_reference_present: false,
        terminal_override_present: false,
        importable: selectable,
        wildcard: !selectable,
        warnings,
    }
}

struct CancelSelection;

impl FileSelectionService for CancelSelection {
    fn select_file(&self, _request: FileSelectionRequest, complete: FileSelectionCallback) {
        complete(Ok(None));
    }
}

#[derive(Default)]
struct DelayedSelection {
    callbacks: RefCell<VecDeque<FileSelectionCallback>>,
}

impl DelayedSelection {
    fn complete_next(&self, path: &Path) {
        self.callbacks
            .borrow_mut()
            .pop_front()
            .expect("pending file selection")(Ok(Some(path.to_path_buf())));
    }
}

impl FileSelectionService for DelayedSelection {
    fn select_file(&self, _request: FileSelectionRequest, complete: FileSelectionCallback) {
        self.callbacks.borrow_mut().push_back(complete);
    }
}

#[derive(Default)]
struct RecordingPort {
    commands: Mutex<Vec<UiCommand>>,
    reject_next: Mutex<bool>,
}

impl RecordingPort {
    fn reject_next(&self) {
        *self.reject_next.lock().unwrap() = true;
    }

    fn preview_count(&self) -> usize {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|command| matches!(command, UiCommand::PreviewImport { .. }))
            .count()
    }

    fn has_preview(&self, source: ImportSourceKind, path: &Path) -> bool {
        self.commands.lock().unwrap().iter().any(|command| {
            matches!(
                command,
                UiCommand::PreviewImport {
                    source: event_source,
                    path: event_path,
                } if *event_source == source && event_path == path
            )
        })
    }

    fn has_exact_override_profile(&self) -> bool {
        self.commands.lock().unwrap().iter().any(|command| {
            let UiCommand::ApplyCatalog { mutation, .. } = command else {
                return false;
            };
            let profile = match mutation {
                CatalogMutation::Create(profile) | CatalogMutation::Update(profile) => profile,
                _ => return false,
            };
            let values = &profile.terminal_overrides;
            values.terminal_type.as_deref() == Some("explicit-terminal")
                && values.initial_cols == Some(211)
                && values.color_scheme == Some(ColorScheme::TokyoNight)
                && values.left_alt_as_meta == Some(false)
                && values
                    .key_bindings
                    .as_ref()
                    .is_some_and(|bindings| bindings.len() == 1)
                && values.explicit_field_count() == 5
        })
    }

    fn connects(&self) -> Vec<(PaneId, ConnectionId)> {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter_map(|command| match command {
                UiCommand::Connect { pane, connection } => Some((*pane, *connection)),
                _ => None,
            })
            .collect()
    }

    fn new_tab_count(&self) -> usize {
        self.commands
            .lock()
            .unwrap()
            .iter()
            .filter(|command| matches!(command, UiCommand::NewLocalTab))
            .count()
    }
}

impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        if std::mem::take(&mut *self.reject_next.lock().unwrap()) {
            return Err(UiPortError::Busy);
        }
        self.commands.lock().unwrap().push(command);
        Ok(())
    }
}

fn present(widget: &impl IsA<gtk::Widget>, width: i32, height: i32) -> gtk::Window {
    let window = gtk::Window::new();
    window.set_default_size(width, height);
    window.set_child(Some(widget));
    window.present();
    window
}

fn present_main(
    main: &relm4::Controller<MainWindow>,
    width: i32,
    height: i32,
) -> gtk::ApplicationWindow {
    main.widget().set_default_size(width, height);
    main.widget().present();
    main.widget().clone()
}

fn button(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button_text(button).as_deref() == Some(label))
        .unwrap_or_else(|| panic!("missing button {label}"))
}

fn button_labels(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .filter_map(|button| button_text(&button))
        .collect()
}

fn button_text(button: &gtk::Button) -> Option<String> {
    button.label().map(Into::into).or_else(|| {
        descendants(button)
            .into_iter()
            .find_map(|widget| widget.downcast::<gtk::Label>().ok())
            .map(|label| label.text().into())
    })
}

fn visible_button_text(button: &gtk::Button) -> Option<String> {
    descendants(button)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .find(|label| label.is_visible())
        .map(|label| label.text().into())
}

fn visible_labels(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .filter(|label| label.is_visible())
        .map(|label| label.text().into())
        .collect()
}

fn mapped_labels(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .filter(|label| label.is_mapped())
        .map(|label| label.text().into())
        .collect()
}

fn mapped_password(root: &impl IsA<gtk::Widget>) -> gtk::PasswordEntry {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::PasswordEntry>().ok())
        .find(|entry| entry.is_mapped())
        .expect("mapped password entry")
}

fn override_inherit(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::CheckButton {
    let label = mapped_label(root, label);
    label
        .next_sibling()
        .and_then(|widget| widget.downcast::<gtk::CheckButton>().ok())
        .expect("override inherit toggle")
}

fn override_editor_value<T: IsA<gtk::Widget> + Clone + 'static>(
    root: &impl IsA<gtk::Widget>,
    label: &str,
) -> T {
    override_inherit(root, label)
        .next_sibling()
        .and_then(|widget| widget.downcast::<T>().ok())
        .expect("override value control")
}

fn base_editor_value<T: IsA<gtk::Widget> + Clone + 'static>(
    root: &impl IsA<gtk::Widget>,
    label: &str,
) -> T {
    mapped_label(root, label)
        .next_sibling()
        .and_then(|widget| widget.downcast::<T>().ok())
        .expect("editor value control")
}

fn mapped_label(root: &impl IsA<gtk::Widget>, text: &str) -> gtk::Label {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .find(|label| label.is_visible() && label.text().as_str() == text)
        .unwrap_or_else(|| panic!("missing mapped label {text}"))
}

fn css_child(root: &impl IsA<gtk::Widget>, class: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class))
        .unwrap_or_else(|| panic!("missing .{class}"))
}

fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    fn collect(widget: &gtk::Widget, output: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(current) = child {
            output.push(current.clone());
            collect(&current, output);
            child = current.next_sibling();
        }
    }
    let mut output = Vec::new();
    collect(root.as_ref(), &mut output);
    output
}

fn focus_is_within(root: &gtk::Widget) -> bool {
    root.root()
        .and_then(|window| gtk::prelude::RootExt::focus(&window))
        .is_some_and(|focused| {
            focused == *root || descendants(root).into_iter().any(|child| child == focused)
        })
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

fn flush_gtk() -> bool {
    let context = gtk::glib::MainContext::default();
    for _ in 0..1_024 {
        if !context.iteration(false) {
            return true;
        }
    }
    false
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
