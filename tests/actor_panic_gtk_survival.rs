use std::sync::Arc;

use async_trait::async_trait;
use gtk::prelude::*;
use relm4::{Component, ComponentController, gtk};
use rshell_core::{
    AppBootstrapState, AppEvent, AppViewModel, PaneId, PaneLaunchTarget, PaneTree, SessionState,
    SessionUiEvent, TabId, TabState, TerminalProfile, TerminalSize, UiCommand, UiCommandPort,
    UiPortError, WorkspaceState,
};
use rshell_session::{
    InteractionBroker, SessionEvent, SessionLaunch, SessionManager, SessionTransport,
    TransportCapabilities, TransportError, TransportEvent, TransportFactory, TransportRequest,
};
use rshell_ui::{MainWindow, MainWindowInit, MainWindowMsg};

struct PanicFactory;

impl TransportFactory for PanicFactory {
    fn create(
        &self,
        _request: &TransportRequest,
    ) -> Result<Box<dyn SessionTransport>, TransportError> {
        Ok(Box::new(PanicTransport))
    }
}

struct PanicTransport;

#[async_trait]
impl SessionTransport for PanicTransport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities::default()
    }

    async fn connect(
        &mut self,
        _request: &TransportRequest,
        _interactions: InteractionBroker,
    ) -> Result<(), TransportError> {
        Ok(())
    }

    async fn next_event(&mut self) -> Result<TransportEvent, TransportError> {
        panic!("intentional actor panic for GTK survival contract")
    }

    async fn write(&mut self, _bytes: &[u8]) -> Result<(), TransportError> {
        Ok(())
    }

    async fn resize(&mut self, _size: TerminalSize) -> Result<(), TransportError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

struct AcceptingPort;

impl UiCommandPort for AcceptingPort {
    fn try_send(&self, _command: UiCommand) -> Result<(), UiPortError> {
        Ok(())
    }
}

#[test]
fn actor_panic_keeps_realized_main_window_alive() {
    gtk::init().expect("GTK must initialize for the required survival surface");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    let manager = Arc::new(SessionManager::new(Arc::new(PanicFactory)));
    let terminal = TerminalProfile::default()
        .settings
        .resolve(&Default::default());
    let size = TerminalSize {
        cols: terminal.cols,
        rows: terminal.rows,
        pixel_width: 0,
        pixel_height: 0,
        dpi: 96,
    };
    let (session, crash) = runtime.block_on(async {
        let launch = SessionLaunch::with_default_engine(TransportRequest::new(size), &terminal)
            .expect("engine");
        let mut client = manager.launch(launch).expect("launch panic actor");
        let crash = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if let SessionEvent::Crashed(message) = client.events.recv().await.expect("event") {
                    break message;
                }
            }
        })
        .await
        .expect("actor crash event timeout");
        (client.id, crash)
    });

    let pane = PaneId::new();
    let tab = TabId::new_v4();
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Panic survival".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(Arc::new(AcceptingPort), view))
        .detach();
    main.widget().present();
    flush_gtk();
    assert!(main.widget().is_realized());

    main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
        session,
        event: SessionUiEvent::Crashed(crash),
    }));
    flush_gtk();
    assert!(main.widget().is_realized());
    assert!(main.widget().is_visible());

    main.widget().close();
    flush_gtk();
    runtime.block_on(manager.shutdown_all()).ok();
}

fn flush_gtk() {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
        context.iteration(false);
    }
}
