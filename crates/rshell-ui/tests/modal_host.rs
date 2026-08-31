#![cfg(not(target_os = "macos"))]

use std::sync::{Arc, Mutex};

use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppEvent, AppViewModel, AuthPrompt, InteractionId, InteractionRequest,
    PaneId, PaneLaunchTarget, PaneTree, SessionId, SessionState, SessionUiCommand, TabId, TabState,
    TerminalProfile, UiCommand, UiCommandPort, UiPortError,
};
use rshell_ui::{
    ConnectionSidebarOutput, MainWindow, MainWindowInit, MainWindowMsg, embedded_theme_css,
};

#[derive(Debug, Clone, Copy)]
enum Kind {
    ConnectionEditor,
    Settings,
    Import,
    Interaction,
}

impl Kind {
    const ALL: [Self; 4] = [
        Self::ConnectionEditor,
        Self::Settings,
        Self::Import,
        Self::Interaction,
    ];

    fn class(self) -> &'static str {
        match self {
            Self::ConnectionEditor => "editor-dialog",
            Self::Settings => "settings-window",
            Self::Import => "import-dialog",
            Self::Interaction => "interaction-dialog",
        }
    }

    fn groups(self) -> &'static [&'static str] {
        match self {
            Self::ConnectionEditor => &[
                "Identity",
                "Transport",
                "Authentication",
                "Terminal overrides",
            ],
            Self::Settings => &["Application", "Active terminal profile"],
            Self::Import => &["Source", "Preview", "Result"],
            Self::Interaction => &["Trust/auth message", "Required inputs", "Actions"],
        }
    }
}

#[test]
fn task7_modal_host_native_contract() {
    if let Err(error) = gtk::init() {
        eprintln!("modal host native contract skipped: {error}");
        return;
    }
    let mut failures = Vec::new();
    run_case(
        &mut failures,
        "editor overlay placement",
        assert_editor_overlay,
    );
    for kind in Kind::ALL {
        run_case(&mut failures, &format!("{kind:?} contract"), || {
            assert_surface_contract(kind)
        });
    }
    run_case(&mut failures, "fallback focus", assert_fallback_focus);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn assert_editor_overlay() {
    let (main, _commands, _session) = launch_main();
    let window = present(&main, 900, 600);
    main.emit(MainWindowMsg::Allocated { width: 800 });
    main.emit(MainWindowMsg::Sidebar(ConnectionSidebarOutput::OpenCreate(
        None,
    )));
    assert!(flush_gtk());
    let editor = css_child(main.widget(), "editor-dialog");
    assert!(
        editor
            .parent()
            .is_some_and(|parent| parent.is::<gtk::Overlay>()),
        "editor must be an overlay child, not an in-flow workspace sibling"
    );
    assert_eq!(editor.width_request(), 680);
    assert_ne!(editor.width_request(), 560);
    window.close();
    assert!(flush_gtk());
}

fn assert_fallback_focus() {
    let (main, _commands, _session) = launch_main();
    let window = present(&main, 1_360, 860);
    let trigger = button_by_tooltip(main.widget(), "Terminal settings");
    trigger.grab_focus();
    trigger.emit_clicked();
    assert!(flush_gtk());
    let parent = trigger
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
        .expect("command-bar button parent");
    parent.remove(&trigger);
    assert!(flush_gtk());
    let surface = css_child(main.widget(), "settings-window");
    assert!(press_key(
        &surface,
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    assert!(flush_gtk());
    let fallback = css_child(main.widget(), "terminal-canvas");
    let focused = fallback
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    assert!(
        focused
            .as_ref()
            .is_some_and(|focused| focus_matches(focused, &fallback)),
        "dead trigger must use terminal fallback; focused={focused:?}, mapped={}, sensitive={}, focusable={}",
        fallback.is_mapped(),
        fallback.is_sensitive(),
        fallback.is_focusable()
    );
    window.close();
    assert!(flush_gtk());
}

fn run_case(failures: &mut Vec<String>, name: &str, check: impl FnOnce() + std::panic::UnwindSafe) {
    if let Err(error) = std::panic::catch_unwind(check) {
        let detail = error
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                error
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| "unknown panic".into());
        failures.push(format!("{name}: {detail}"));
    }
}

fn assert_surface_contract(kind: Kind) {
    let (main, commands, session) = launch_main();
    let window = present(&main, 900, 600);
    main.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());
    let (trigger, interaction) = open(&main, kind, session);
    assert!(flush_gtk());
    let surface = css_child(main.widget(), kind.class());
    let scrim = css_child(main.widget(), "modal-scrim");
    let background = css_child(main.widget(), "modal-background");

    assert!(surface.is_mapped(), "{kind:?} surface must be mapped");
    assert!(scrim.is_mapped(), "{kind:?} scrim must be mapped");
    assert!(scrim.can_target(), "{kind:?} scrim must intercept pointers");
    assert!(embedded_theme_css().contains("background: #121212;"));
    assert_eq!(
        controller_count::<gtk::GestureClick>(&scrim),
        0,
        "{kind:?} scrim must not click-dismiss"
    );
    let open_key_controllers = controller_count::<gtk::EventControllerKey>(&surface);
    assert!(
        !background.is_sensitive(),
        "{kind:?} background must be inert"
    );
    for width in [800, 1_360, 1_920] {
        main.emit(MainWindowMsg::Allocated { width });
        assert!(flush_gtk());
        assert_eq!(
            surface.width_request(),
            680.min((width - 48).max(1)),
            "{kind:?} width at {width}"
        );
    }
    assert_dialog_structure(&surface, kind);

    let first = css_child(&surface, "modal-focus-first");
    let last = css_child(&surface, "modal-focus-last");
    let focused = surface
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    assert!(
        focused
            .as_ref()
            .is_some_and(|focused| is_same_or_descendant(focused, &first)),
        "{kind:?} initial focus: focused={focused:?}, first={first:?}"
    );

    last.grab_focus();
    assert!(flush_gtk());
    assert!(press_key(
        &surface,
        gtk::gdk::Key::Tab,
        gtk::gdk::ModifierType::empty()
    ));
    assert!(
        wait_for_gtk(|| has_focus_within(&first)),
        "{kind:?} Tab must wrap to first"
    );
    assert!(press_key(
        &surface,
        gtk::gdk::Key::Tab,
        gtk::gdk::ModifierType::SHIFT_MASK
    ));
    assert!(
        wait_for_gtk(|| has_focus_within(&last)),
        "{kind:?} Shift+Tab must wrap to last"
    );

    let before = commands.len();
    assert!(press_key(
        &surface,
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    assert!(flush_gtk());
    let (responses, unrelated) = commands.classify_since(before);
    if let Some(interaction) = interaction {
        assert_eq!(responses, 1, "interaction cancel reducer dispatch");
        assert_eq!(unrelated, 0, "interaction close emitted unrelated work");
        main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
            session,
            interaction,
        }));
        assert!(flush_gtk());
    } else {
        assert_eq!(
            responses, 0,
            "non-interaction modal close emitted a response"
        );
        assert_eq!(unrelated, 0, "modal close emitted unrelated work");
    }
    assert!(!surface.is_visible(), "{kind:?} reducer must close surface");
    assert!(!scrim.is_visible(), "{kind:?} close must hide scrim");
    assert_eq!(
        controller_count::<gtk::EventControllerKey>(&surface) + 1,
        open_key_controllers,
        "{kind:?} close must remove the host key controller"
    );
    assert!(
        background.is_sensitive(),
        "{kind:?} close must restore background"
    );
    assert!(
        has_focus_within(&trigger),
        "{kind:?} close must restore trigger focus; focused={:?}",
        trigger
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
    );
    window.close();
    assert!(flush_gtk());
}

fn assert_dialog_structure(surface: &gtk::Widget, kind: Kind) {
    let direct = direct_children(surface);
    assert!(
        direct
            .iter()
            .any(|child| child.has_css_class("dialog-header"))
    );
    assert!(
        direct
            .iter()
            .any(|child| child.has_css_class("dialog-footer"))
    );
    let scroll = direct
        .into_iter()
        .find_map(|child| child.downcast::<gtk::ScrolledWindow>().ok())
        .expect("modal body must be a direct scrolled child");
    assert!(scroll.vexpands(), "modal body must consume bounded height");
    let body = scroll.child().expect("scrolled modal body");
    assert!(
        body.has_css_class("dialog-body")
            || descendants(&body)
                .iter()
                .any(|child| child.has_css_class("dialog-body")),
        "scrolled content must contain .dialog-body"
    );
    let labels = visible_labels(surface);
    for group in kind.groups() {
        assert!(
            labels.iter().any(|label| label == group),
            "{kind:?} missing {group}"
        );
    }
}

fn open(
    main: &relm4::Controller<MainWindow>,
    kind: Kind,
    session: SessionId,
) -> (gtk::Widget, Option<InteractionId>) {
    match kind {
        Kind::ConnectionEditor => {
            let trigger = button_by_tooltip(main.widget(), "Create a connection");
            trigger.grab_focus();
            trigger.emit_clicked();
            (trigger.upcast(), None)
        }
        Kind::Settings => {
            let trigger = button_by_tooltip(main.widget(), "Terminal settings");
            trigger.grab_focus();
            trigger.emit_clicked();
            (trigger.upcast(), None)
        }
        Kind::Import => {
            let trigger = button_by_tooltip(main.widget(), "Import connections");
            trigger.grab_focus();
            trigger.emit_clicked();
            (trigger.upcast(), None)
        }
        Kind::Interaction => {
            let trigger = css_child(main.widget(), "terminal-canvas");
            trigger.grab_focus();
            let interaction = InteractionId::new();
            main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
                session,
                request: InteractionRequest::Password(AuthPrompt {
                    id: interaction,
                    label: "Password".into(),
                    echo: false,
                }),
            }));
            (trigger, Some(interaction))
        }
    }
}

fn launch_main() -> (relm4::Controller<MainWindow>, Arc<RecordingPort>, SessionId) {
    let commands = Arc::new(RecordingPort::default());
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Modal contract".into(),
        pane_tree: PaneTree::with_session(pane, session),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(commands.clone(), view))
        .detach();
    (main, commands, session)
}

#[derive(Default)]
struct RecordingPort {
    commands: Mutex<Vec<UiCommand>>,
}

impl RecordingPort {
    fn len(&self) -> usize {
        self.commands.lock().unwrap().len()
    }

    fn classify_since(&self, index: usize) -> (usize, usize) {
        let commands = self.commands.lock().unwrap();
        let commands = &commands[index..];
        let responses = commands
            .iter()
            .filter(|command| matches!(command, UiCommand::Respond { .. }))
            .count();
        let unrelated = commands
            .iter()
            .filter(|command| {
                !matches!(command, UiCommand::Respond { .. }) && !is_geometry_refresh(command)
            })
            .count();
        (responses, unrelated)
    }
}

fn is_geometry_refresh(command: &UiCommand) -> bool {
    matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Resize(_),
            ..
        }
    )
}

impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        self.commands.lock().unwrap().push(command);
        Ok(())
    }
}

fn present(
    main: &relm4::Controller<MainWindow>,
    width: i32,
    height: i32,
) -> gtk::ApplicationWindow {
    main.widget().set_default_size(width, height);
    main.widget().present();
    main.widget().clone()
}

fn button_by_tooltip(root: &impl IsA<gtk::Widget>, tooltip: &str) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.tooltip_text().as_deref() == Some(tooltip))
        .unwrap_or_else(|| panic!("missing button with tooltip {tooltip}"))
}

fn css_child(root: &impl IsA<gtk::Widget>, class: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class))
        .unwrap_or_else(|| panic!("missing .{class}"))
}

fn visible_labels(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .filter(|label| label.is_visible())
        .map(|label| label.text().into())
        .collect()
}

fn has_focus_within(widget: &gtk::Widget) -> bool {
    widget
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root))
        .as_ref()
        .is_some_and(|focused| focus_matches(focused, widget))
}

fn focus_matches(focused: &gtk::Widget, target: &gtk::Widget) -> bool {
    is_same_or_descendant(focused, target)
        || (focused.is::<gtk::Paned>() && is_same_or_descendant(target, focused))
}

fn is_same_or_descendant(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget == *ancestor {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn direct_children(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut output = Vec::new();
    let mut child = root.as_ref().first_child();
    while let Some(current) = child {
        output.push(current.clone());
        child = current.next_sibling();
    }
    output
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

fn press_key(
    root: &impl IsA<gtk::Widget>,
    key: gtk::gdk::Key,
    state: gtk::gdk::ModifierType,
) -> bool {
    let controllers = root.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .any(|controller| controller.emit_by_name::<bool>("key-pressed", &[&key, &0u32, &state]))
}

fn controller_count<T: IsA<gtk::EventController> + Clone + 'static>(
    widget: &impl IsA<gtk::Widget>,
) -> u32 {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter(|controller| controller.is::<T>())
        .count() as u32
}

fn wait_for_gtk(mut condition: impl FnMut() -> bool) -> bool {
    let context = gtk::glib::MainContext::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        while context.iteration(false) {}
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    condition()
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
