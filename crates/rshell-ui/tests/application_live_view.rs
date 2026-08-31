#![cfg(not(target_os = "macos"))]

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use async_trait::async_trait;
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppDependencies, AppSettings, AppViewModel, ApplicationService,
    CatalogMutation, ConnectionCatalog, ConnectionId, ConnectionProfile, ConnectionRepository,
    CredentialOperationError, CredentialPort, CredentialRef, DisplayRecoveryNotice,
    ImportCandidateId, ImportCommitResult, ImportError, ImportPort, ImportPreviewId,
    ImportPreviewView, ImportSourceKind, LatestViewStream, PaneId, PaneLaunchTarget, PaneTree,
    RenderFrame, RepositoryError, ResolvedTerminalProfile, SecretUpdate, SessionBinding,
    SessionFailure, SessionId, SessionPort, SessionState, SessionUiCommand, SessionUiEvent,
    SplitAxis, TabId, TabState, TerminalDisplayModes, TerminalOverrides, TerminalProfile,
    TerminalSize, UiCommand, UiCommandPort, UiPortError, WorkspaceState,
};
use rshell_ui::{
    ConnectionSidebarOutput, MainWindow, MainWindowInit, MainWindowMsg, PaneAction, PanePageKind,
    SessionPaneViewModel,
};
use secrecy::SecretString;
use tokio::time::timeout;

#[test]
fn breakpoint_crossing_preserves_controller_and_reducer_identity() {
    if let Err(error) = gtk::init() {
        eprintln!("live adaptive shell regression skipped: {error}");
        return;
    }
    let (view, active_tab, active_pane, session) = adaptive_fixture();
    let window = MainWindow::builder()
        .launch(MainWindowInit::new(Arc::new(AcceptingPort), view))
        .detach();
    window.widget().set_default_size(1_000, 700);
    window.widget().present();
    assert!(flush_gtk());
    window.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());

    let sidebar_search = sidebar_search(window.widget());
    sidebar_search.set_text("Retained");
    assert!(flush_gtk());
    let list = descendants(window.widget())
        .into_iter()
        .find_map(|widget| widget.downcast::<gtk::ListBox>().ok())
        .expect("connection list");
    let row = (0..list.observe_children().n_items() as i32)
        .filter_map(|index| list.row_at_index(index))
        .find(|row| {
            descendants(row).into_iter().any(|widget| {
                widget
                    .downcast::<gtk::Label>()
                    .is_ok_and(|label| label.text().as_str() == "Retained connection")
            })
        })
        .expect("filtered connection row");
    list.select_row(Some(&row));
    window.emit(MainWindowMsg::Sidebar(ConnectionSidebarOutput::OpenCreate(
        None,
    )));
    assert!(flush_gtk());
    let editor = css_child(window.widget(), "editor-dialog");
    let name = editor_field(&editor, "Name");
    let host = editor_field(&editor, "Host");
    name.set_text("Unsaved adaptive draft");
    host.set_text("draft.example.test");
    let canvas = active_terminal(window.widget());
    assert!(press_key(
        &canvas,
        gtk::gdk::Key::from_name("f").unwrap(),
        gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
    ));
    assert!(flush_gtk());
    let terminal_search = active_terminal_search(window.widget());
    terminal_search.set_text("needle");
    list.select_row(Some(&row));
    canvas.grab_focus();
    assert!(flush_gtk());
    assert!(
        wait_for_gtk(|| list.selected_row().is_some()),
        "selected connection row must settle before breakpoint assertions"
    );

    let identities = AdaptiveIdentities::capture(window.widget());
    assert!(css_child(window.widget(), "shell-compact").is_visible());
    assert_adaptive_state(
        window.widget(),
        &identities,
        active_tab,
        active_pane,
        session,
    );

    for (width, class) in [
        (900, "shell-standard"),
        (1_440, "shell-wide"),
        (800, "shell-compact"),
    ] {
        window.emit(MainWindowMsg::Allocated { width });
        assert!(flush_gtk(), "allocation {width} must quiesce");
        assert!(css_child(window.widget(), class).is_visible());
        assert_adaptive_state(
            window.widget(),
            &identities,
            active_tab,
            active_pane,
            session,
        );
    }

    window.widget().close();
    assert!(flush_gtk());
}

#[derive(Clone, Copy)]
struct AdaptiveIdentities {
    sidebar: usize,
    active_tab: usize,
    terminal: usize,
    editor: usize,
    selected_row: usize,
    focused: usize,
}

impl AdaptiveIdentities {
    fn capture(root: &impl IsA<gtk::Widget>) -> Self {
        Self {
            sidebar: css_child(root, "sidebar").as_ptr() as usize,
            active_tab: css_child(root, "active-tab").as_ptr() as usize,
            terminal: active_terminal(root).as_ptr() as usize,
            editor: css_child(root, "editor-dialog").as_ptr() as usize,
            selected_row: css_child(root, "connection-list")
                .downcast::<gtk::ListBox>()
                .ok()
                .and_then(|list| list.selected_row())
                .expect("selected connection row")
                .as_ptr() as usize,
            focused: root
                .as_ref()
                .root()
                .and_then(|root| gtk::prelude::RootExt::focus(&root))
                .expect("focused shell widget")
                .as_ptr() as usize,
        }
    }
}

fn assert_adaptive_state(
    root: &gtk::ApplicationWindow,
    identities: &AdaptiveIdentities,
    active_tab: TabId,
    active_pane: PaneId,
    session: SessionId,
) {
    let actual = AdaptiveIdentities::capture(root);
    assert_eq!(actual.sidebar, identities.sidebar);
    assert_eq!(
        actual.active_tab, identities.active_tab,
        "active tab {active_tab}"
    );
    assert_eq!(actual.terminal, identities.terminal, "session {session:?}");
    assert_eq!(
        active_terminal(root).as_ptr() as usize,
        identities.terminal,
        "active pane {active_pane:?}"
    );
    assert_eq!(actual.editor, identities.editor);
    assert_eq!(actual.selected_row, identities.selected_row);
    assert_eq!(sidebar_search(root).text(), "Retained");
    let terminal_search = active_terminal_search(root);
    assert!(terminal_search.is_visible());
    assert_eq!(terminal_search.text(), "needle");
    let editor = css_child(root, "editor-dialog");
    assert!(editor.is_visible());
    assert_eq!(
        editor_field(&editor, "Name").text(),
        "Unsaved adaptive draft"
    );
    assert_eq!(editor_field(&editor, "Host").text(), "draft.example.test");
    assert_eq!(
        gtk::prelude::GtkWindowExt::focus(root)
            .as_ref()
            .map(|widget| widget.as_ptr() as usize),
        Some(identities.focused)
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

fn adaptive_fixture() -> (AppViewModel, TabId, PaneId, SessionId) {
    let active_tab = TabId::new_v4();
    let active_pane = PaneId::new();
    let session = SessionId::new();
    let sibling_pane = PaneId::new();
    let sibling_session = SessionId::new();
    let inactive_tab = TabId::new_v4();
    let inactive_pane = PaneId::new();
    let inactive_session = SessionId::new();
    let retained = ConnectionProfile::new("Retained connection", "retained.example.test");
    let mut catalog = ConnectionCatalog::default();
    catalog.connections.insert(retained.id, retained);
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog,
        settings: AppSettings::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    let mut active_tree = PaneTree::with_session(sibling_pane, sibling_session)
        .split(sibling_pane, SplitAxis::Horizontal, active_pane, 0.5)
        .unwrap();
    active_tree
        .replace_session(active_pane, Some(session))
        .unwrap();
    view.workspace = WorkspaceState {
        tabs: vec![
            TabState {
                id: inactive_tab,
                title: "Inactive tab".into(),
                pane_tree: PaneTree::with_session(inactive_pane, inactive_session),
                active_pane: inactive_pane,
            },
            TabState {
                id: active_tab,
                title: "Retained tab".into(),
                pane_tree: active_tree,
                active_pane,
            },
        ],
        active_tab: Some(active_tab),
    };
    for (pane, bound) in [
        (active_pane, session),
        (sibling_pane, sibling_session),
        (inactive_pane, inactive_session),
    ] {
        view.pane_launches.insert(pane, PaneLaunchTarget::Local);
        view.session_states.insert(bound, SessionState::Connected);
        view.latest_frames.insert(bound, frame(37));
    }
    (view, active_tab, active_pane, session)
}

struct AcceptingPort;

impl UiCommandPort for AcceptingPort {
    fn try_send(&self, _command: UiCommand) -> Result<(), UiPortError> {
        Ok(())
    }
}

fn sidebar_search(root: &impl IsA<gtk::Widget>) -> gtk::SearchEntry {
    let sidebar = css_child(root, "sidebar");
    descendants(&sidebar)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::SearchEntry>().ok())
        .find(|entry| !entry.has_css_class("terminal-search"))
        .expect("connection search")
}

fn editor_field(root: &impl IsA<gtk::Widget>, label: &str) -> gtk::Entry {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .find(|candidate| candidate.text().as_str() == label)
        .and_then(|label| label.next_sibling())
        .and_then(|widget| widget.downcast::<gtk::Entry>().ok())
        .unwrap_or_else(|| panic!("missing editor field {label}"))
}

fn css_child(root: &impl IsA<gtk::Widget>, class: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class))
        .unwrap_or_else(|| panic!("missing .{class}"))
}

fn active_terminal(root: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let active = css_child(root, "active-pane");
    css_child(&active, "terminal-canvas")
}

fn active_terminal_search(root: &impl IsA<gtk::Widget>) -> gtk::SearchEntry {
    let active = css_child(root, "active-pane");
    css_child(&active, "terminal-search")
        .downcast::<gtk::SearchEntry>()
        .expect("active terminal search")
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

#[tokio::test(flavor = "current_thread")]
async fn application_stream_drives_authoritative_connection_retry_and_failure_ui() {
    if let Err(error) = gtk::init() {
        eprintln!("live MainWindow stream regression skipped: {error}");
        return;
    }
    let (bootstrap, connection) = bootstrap();
    let ports = TestPorts::new();
    let app = ApplicationService::start(ports.dependencies(), bootstrap)
        .await
        .unwrap();
    let initial = app.view_model();
    let pane = initial.workspace.tabs[0].active_pane;
    let initial_session = initial.workspace.tabs[0]
        .pane_tree
        .session_id(pane)
        .unwrap()
        .unwrap();
    let init = MainWindowInit::from_application(&app);
    let mut latest = init
        .latest_view_stream()
        .expect("application init must retain authoritative updates");
    let window = MainWindow::builder().launch(init).detach();

    app.ui_port()
        .try_send(UiCommand::Connect { pane, connection })
        .unwrap();
    let connected = wait_view(&mut latest, |view| {
        matches!(
            view.pane_launches.get(&pane),
            Some(PaneLaunchTarget::Connection { id, .. }) if *id == connection
        ) && view.workspace.tabs[0]
            .pane_tree
            .session_id(pane)
            .ok()
            .flatten()
            != Some(initial_session)
    })
    .await;
    let connected_session = connected.workspace.tabs[0]
        .pane_tree
        .session_id(pane)
        .unwrap()
        .unwrap();
    assert_ne!(connected_session, initial_session);
    assert!(!ports.is_live(initial_session));
    ports.send_event(
        connected_session,
        SessionUiEvent::State(SessionState::Connected),
    );
    let connected = wait_view(&mut latest, |view| {
        view.session_states.get(&connected_session) == Some(&SessionState::Connected)
    })
    .await;
    let pane_view = SessionPaneViewModel::from_app(&connected, pane).unwrap();
    let resolved = pane_view.resolved_profile(&connected).unwrap();
    assert_eq!(resolved.cols, 132);
    assert_eq!(resolved.font_size, 19.0);

    let recovery = DisplayRecoveryNotice {
        interrupted_generation: 76,
        observed_generation: 77,
        modes: TerminalDisplayModes {
            alternate_screen: true,
            enhanced_keyboard: true,
            ..TerminalDisplayModes::default()
        },
    };
    ports.send_event(
        connected_session,
        SessionUiEvent::RecoveryChanged(Some(recovery)),
    );
    let recovering = wait_view(&mut latest, |view| {
        view.display_recovery.get(&connected_session) == Some(&recovery)
    })
    .await;
    let pane_view = SessionPaneViewModel::from_app(&recovering, pane).unwrap();
    assert_eq!(pane_view.page(), PanePageKind::Terminal);
    assert_eq!(pane_view.recovery_notice(), Some(recovery));
    assert_eq!(
        pane_view
            .actions()
            .iter()
            .filter(|action| **action == PaneAction::ResetDisplay)
            .count(),
        1
    );
    assert!(flush_gtk());
    let recovery_rows = descendants(window.widget())
        .into_iter()
        .filter(|widget| widget.has_css_class("display-recovery-notice"))
        .collect::<Vec<_>>();
    assert_eq!(recovery_rows.len(), 1);
    assert_eq!(
        label_text(&recovery_rows[0])
            .into_iter()
            .filter(|label| label == "Display mode not restored")
            .count(),
        1
    );
    let reset_buttons = descendants(window.widget())
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
        descendants(window.widget())
            .into_iter()
            .filter(|widget| widget.has_css_class("pane-command-row"))
            .flat_map(|toolbar| descendants(&toolbar))
            .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
            .all(|button| button.tooltip_text().as_deref() != Some("Reset display"))
    );
    reset_buttons[0].emit_clicked();
    assert!(flush_gtk());
    wait_for_reset_display(&ports).await;

    ports.send_event(
        connected_session,
        SessionUiEvent::Failed(SessionFailure::Network),
    );
    wait_view(&mut latest, |view| {
        view.session_states.get(&connected_session) == Some(&SessionState::Failed)
    })
    .await;
    app.ui_port().try_send(UiCommand::RetryPane(pane)).unwrap();
    let retried = wait_view(&mut latest, |view| {
        view.workspace.tabs[0]
            .pane_tree
            .session_id(pane)
            .ok()
            .flatten()
            .is_some_and(|session| session != connected_session)
    })
    .await;
    let retry_session = retried.workspace.tabs[0]
        .pane_tree
        .session_id(pane)
        .unwrap()
        .unwrap();
    assert_ne!(retry_session, connected_session);
    assert!(!ports.is_live(connected_session));

    ports.send_frame(retry_session, frame(77));
    wait_view(&mut latest, |view| {
        view.latest_frames
            .get(&retry_session)
            .map(|frame| frame.generation)
            == Some(77)
    })
    .await;
    ports.send_event(
        retry_session,
        SessionUiEvent::Failed(SessionFailure::Authentication),
    );
    wait_view(&mut latest, |view| {
        view.error_panes.contains_key(&retry_session)
    })
    .await;
    ports.fail_launch(true);
    app.ui_port().try_send(UiCommand::RetryPane(pane)).unwrap();
    let failed = wait_view(&mut latest, |view| {
        view.session_states.get(&retry_session) == Some(&SessionState::Failed)
            && view
                .error_panes
                .get(&retry_session)
                .is_some_and(|error| error.diagnostic == "session launch failed")
    })
    .await;

    assert!(!ports.is_live(retry_session));
    assert!(!failed.latest_frames.contains_key(&retry_session));
    assert!(!failed.display_recovery.contains_key(&connected_session));
    assert!(matches!(
        failed.pane_launches.get(&pane),
        Some(PaneLaunchTarget::Connection { id, host })
            if *id == connection && host == "safe.example.test"
    ));
    let error = failed.error_panes.get(&retry_session).unwrap();
    assert_eq!(error.host.as_deref(), Some("safe.example.test"));
    assert!(error.timestamp_unix_seconds > 0);
    let pane_view = SessionPaneViewModel::from_app(&failed, pane).unwrap();
    assert_eq!(pane_view.page(), PanePageKind::Error);
    assert_eq!(
        pane_view.actions(),
        [
            PaneAction::Retry,
            PaneAction::EditConnection,
            PaneAction::CopyDiagnostics,
            PaneAction::Close,
        ]
    );
    let diagnostics = pane_view.diagnostics().unwrap();
    assert!(diagnostics.contains("host: safe.example.test"));
    assert!(!diagnostics.to_ascii_lowercase().contains("password"));

    assert!(flush_gtk());
    let labels = label_text(window.widget());
    assert!(labels.iter().any(|label| label == "Failed"));
    assert!(labels.iter().all(|label| !label.contains('\u{fffd}')));
    assert!(descendants(window.widget()).into_iter().any(|widget| {
        widget
            .downcast::<gtk::Button>()
            .is_ok_and(|button| button.tooltip_text().as_deref() == Some("Edit Connection"))
    }));

    window.emit(rshell_ui::MainWindowMsg::ReplaceViewModel(connected));
    window.emit(rshell_ui::MainWindowMsg::AppEvent(
        rshell_core::AppEvent::Session {
            session: connected_session,
            event: SessionUiEvent::Frame(frame(1_000)),
        },
    ));
    assert!(flush_gtk());
    assert!(
        label_text(window.widget())
            .iter()
            .any(|label| label == "Failed")
    );
    assert!(
        !descendants(window.widget())
            .iter()
            .any(|widget| widget.is::<gtk::DrawingArea>())
    );

    ports.send_frame(retry_session, frame(99));
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!app.view_model().latest_frames.contains_key(&retry_session));
    drop(window);
    app.shutdown().await.unwrap();
}

async fn wait_view(
    stream: &mut LatestViewStream,
    predicate: impl Fn(&AppViewModel) -> bool,
) -> AppViewModel {
    timeout(Duration::from_secs(2), async {
        loop {
            let view = stream
                .changed()
                .await
                .expect("application view stream closed");
            if predicate(&view) {
                return view;
            }
        }
    })
    .await
    .expect("authoritative view update timed out")
}

async fn wait_for_reset_display(ports: &TestPorts) {
    timeout(Duration::from_secs(2), async {
        while ports.reset_display_commands() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Reset display command was not dispatched through the application port");
    assert_eq!(ports.reset_display_commands(), 1);
}

fn bootstrap() -> (AppBootstrapState, ConnectionId) {
    let mut connection = ConnectionProfile::new("Managed", "safe.example.test");
    connection.terminal_overrides = TerminalOverrides {
        initial_cols: Some(132),
        font_size: Some(19.0),
        ..TerminalOverrides::default()
    };
    let id = connection.id;
    let mut catalog = ConnectionCatalog::default();
    catalog.connections.insert(id, connection);
    (
        AppBootstrapState {
            catalog,
            settings: AppSettings::default(),
            terminal_profiles: vec![TerminalProfile::default()],
        },
        id,
    )
}

fn frame(generation: u64) -> Arc<RenderFrame> {
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
        title: "stream fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

#[derive(Clone, Default)]
struct TestPorts {
    state: Arc<Mutex<TestState>>,
}

#[derive(Default)]
struct TestState {
    sessions: BTreeMap<SessionId, TestSession>,
    live: BTreeSet<SessionId>,
    fail_launch: bool,
    reset_display_commands: usize,
}

struct TestSession {
    events: async_channel::Sender<SessionUiEvent>,
    frames: tokio::sync::watch::Sender<Option<Arc<RenderFrame>>>,
}

impl TestPorts {
    fn new() -> Self {
        Self::default()
    }

    fn dependencies(&self) -> AppDependencies {
        AppDependencies {
            repository: Arc::new(self.clone()),
            credentials: Arc::new(self.clone()),
            imports: Arc::new(self.clone()),
            sessions: Arc::new(self.clone()),
        }
    }

    fn launch(&self) -> Result<SessionBinding, SessionFailure> {
        let mut state = self.state.lock().unwrap();
        if state.fail_launch {
            return Err(SessionFailure::Pty);
        }
        let id = SessionId::new();
        let (events, event_rx) = async_channel::bounded(16);
        let (frames, frame_rx) = tokio::sync::watch::channel(None);
        state.sessions.insert(id, TestSession { events, frames });
        state.live.insert(id);
        Ok(SessionBinding {
            id,
            events: event_rx,
            frames: frame_rx,
        })
    }

    fn fail_launch(&self, fail: bool) {
        self.state.lock().unwrap().fail_launch = fail;
    }

    fn is_live(&self, session: SessionId) -> bool {
        self.state.lock().unwrap().live.contains(&session)
    }

    fn reset_display_commands(&self) -> usize {
        self.state.lock().unwrap().reset_display_commands
    }

    fn send_event(&self, session: SessionId, event: SessionUiEvent) {
        self.state
            .lock()
            .unwrap()
            .sessions
            .get(&session)
            .unwrap()
            .events
            .try_send(event)
            .unwrap();
    }

    fn send_frame(&self, session: SessionId, frame: Arc<RenderFrame>) {
        self.state
            .lock()
            .unwrap()
            .sessions
            .get(&session)
            .unwrap()
            .frames
            .send_replace(Some(frame));
    }
}

#[async_trait]
impl SessionPort for TestPorts {
    async fn launch_local(
        &self,
        _pane: PaneId,
        _terminal: ResolvedTerminalProfile,
    ) -> Result<SessionBinding, SessionFailure> {
        self.launch()
    }

    async fn launch_ssh(
        &self,
        _pane: PaneId,
        _profile: ConnectionProfile,
        _terminal: ResolvedTerminalProfile,
        _initial_size: TerminalSize,
        _secret: Option<SecretString>,
    ) -> Result<SessionBinding, SessionFailure> {
        self.launch()
    }

    async fn command(
        &self,
        _session: SessionId,
        command: SessionUiCommand,
    ) -> Result<(), SessionFailure> {
        if matches!(command, SessionUiCommand::ResetDisplay) {
            self.state.lock().unwrap().reset_display_commands += 1;
        }
        Ok(())
    }

    async fn shutdown(&self, session: SessionId) -> Result<(), SessionFailure> {
        self.state.lock().unwrap().live.remove(&session);
        Ok(())
    }

    async fn shutdown_all(&self) -> Result<(), SessionFailure> {
        self.state.lock().unwrap().live.clear();
        Ok(())
    }
}

#[async_trait]
impl ConnectionRepository for TestPorts {
    async fn load_catalog(&self) -> Result<ConnectionCatalog, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn apply(&self, _: CatalogMutation) -> Result<ConnectionCatalog, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn load_terminal_profiles(&self) -> Result<Vec<TerminalProfile>, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn save_terminal_profile(&self, _: TerminalProfile) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn load_settings(&self) -> Result<AppSettings, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn save_settings(&self, _: AppSettings) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
}

#[async_trait]
impl CredentialPort for TestPorts {
    async fn apply_catalog(
        &self,
        _: CatalogMutation,
        _: SecretUpdate,
    ) -> Result<ConnectionCatalog, CredentialOperationError> {
        unreachable!("catalog mutation is outside Task17 stream coverage")
    }
    async fn get(
        &self,
        _: &CredentialRef,
    ) -> Result<Option<SecretString>, CredentialOperationError> {
        Ok(None)
    }
}

#[async_trait]
impl ImportPort for TestPorts {
    async fn preview(
        &self,
        _: ImportSourceKind,
        _: &Path,
    ) -> Result<ImportPreviewView, ImportError> {
        Err(ImportError::Validation)
    }
    async fn commit(
        &self,
        _: ImportPreviewId,
        _: &BTreeSet<ImportCandidateId>,
    ) -> Result<ImportCommitResult, ImportError> {
        Err(ImportError::Validation)
    }
    async fn cancel(&self, _: ImportPreviewId) -> Result<(), ImportError> {
        Err(ImportError::Validation)
    }
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

fn label_text(root: &impl IsA<gtk::Widget>) -> Vec<String> {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Label>().ok())
        .map(|label| label.text().into())
        .collect()
}
