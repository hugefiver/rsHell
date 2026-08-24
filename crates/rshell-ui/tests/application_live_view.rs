use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppDependencies, AppSettings, AppViewModel, ApplicationService,
    CatalogMutation, ConnectionCatalog, ConnectionId, ConnectionProfile, ConnectionRepository,
    CredentialOperationError, CredentialPort, CredentialRef, ImportCandidateId, ImportCommitResult,
    ImportError, ImportPort, ImportPreviewId, ImportPreviewView, ImportSourceKind,
    LatestViewStream, PaneId, PaneLaunchTarget, RenderFrame, RepositoryError,
    ResolvedTerminalProfile, SecretUpdate, SessionBinding, SessionFailure, SessionId, SessionPort,
    SessionState, SessionUiCommand, SessionUiEvent, TerminalOverrides, TerminalProfile,
    TerminalSize, UiCommand,
};
use rshell_ui::{MainWindow, MainWindowInit, PaneAction, PanePageKind, SessionPaneViewModel};
use secrecy::SecretString;
use tokio::time::timeout;

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
    let pane_view = SessionPaneViewModel::from_app(&connected, pane).unwrap();
    let resolved = pane_view.resolved_profile(&connected).unwrap();
    assert_eq!(resolved.cols, 132);
    assert_eq!(resolved.font_size, 19.0);

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
        _command: SessionUiCommand,
    ) -> Result<(), SessionFailure> {
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
