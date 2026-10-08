use super::ui_focus::{FocusWindow, drain};
use relm4::ComponentController;
use rshell_core::{
    AppEvent, NewLocalTabCompletion, NewLocalTabIdentity, NewLocalTabReceipt,
    NewLocalTabReceiptClosed, NewLocalTabSubmitError, PaneId, SessionBinding, SessionId,
    SessionPort, SessionState, SessionUiCommand, SessionUiEvent, TerminalProfile, UiCommand,
    UiCommandPort, UiPortError,
};
use rshell_session::{
    KnownHostsVerifier, LocalLaunch, LocalPtyFactory, SessionManager, ports::SessionPortAdapter,
};
use rshell_ui::MainWindowMsg;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct OwnedPty {
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub port: Arc<PtyPort>,
    pub bindings: BTreeMap<SessionId, SessionBinding>,
}

pub struct PtyPort {
    adapter: Arc<SessionPortAdapter>,
    runtime: Arc<tokio::runtime::Runtime>,
    new_commands: Mutex<usize>,
    pub inputs: Mutex<Vec<SessionId>>,
    failures: Arc<Mutex<Vec<rshell_core::SessionFailure>>>,
    receipts: Mutex<Vec<Option<tokio::sync::oneshot::Sender<NewLocalTabCompletion>>>>,
}

impl UiCommandPort for PtyPort {
    fn try_new_local_tab_with_completion(
        &self,
    ) -> Result<NewLocalTabReceipt, NewLocalTabSubmitError> {
        *self.new_commands.lock().unwrap() += 1;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.receipts.lock().unwrap().push(Some(sender));
        Ok(Box::pin(async move {
            receiver.await.map_err(|_| NewLocalTabReceiptClosed)
        }))
    }
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        match command {
            UiCommand::NewLocalTab => *self.new_commands.lock().unwrap() += 1,
            UiCommand::Session { session, command } => {
                if matches!(command, SessionUiCommand::Input(_)) {
                    self.inputs.lock().unwrap().push(session);
                }
                let adapter = self.adapter.clone();
                let failures = self.failures.clone();
                if let Err(failure) = self.runtime.block_on(adapter.command(session, command)) {
                    failures.lock().unwrap().push(failure);
                    return Err(UiPortError::Closed);
                }
            }
            _ => panic!("unexpected command in isolated owned PTY focus fixture"),
        }
        Ok(())
    }
}

impl PtyPort {
    pub fn created(&self, identity: NewLocalTabIdentity) {
        if let Some(sender) = self
            .receipts
            .lock()
            .unwrap()
            .last_mut()
            .and_then(Option::take)
        {
            let _ = sender.send(NewLocalTabCompletion::Created(identity));
        }
    }
    pub fn new_count(&self) -> usize {
        *self.new_commands.lock().unwrap()
    }
    pub fn manager(&self) -> &Arc<SessionManager> {
        self.adapter.manager()
    }
}

impl OwnedPty {
    pub fn new() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
        );
        let manager = Arc::new(SessionManager::new(LocalPtyFactory::new(
            LocalLaunch::Command {
                program: "C:\\Windows\\System32\\cmd.exe".into(),
                args: vec!["/d".into(), "/q".into()],
                cwd: None,
                env: BTreeMap::new(),
            },
        )));
        // Placeholder is never read/written: this fixture never launches SSH.
        let adapter = Arc::new(SessionPortAdapter::new(
            manager,
            KnownHostsVerifier::new(
                "C:\\Users\\HUGEFI~1\\AppData\\Local\\Temp\\opencode\\rshell-focus-unused-known-hosts-6540860",
            ),
        ));
        let port = Arc::new(PtyPort {
            adapter,
            runtime: runtime.clone(),
            new_commands: Mutex::new(0),
            inputs: Mutex::new(Vec::new()),
            failures: Arc::new(Mutex::new(Vec::new())),
            receipts: Mutex::new(Vec::new()),
        });
        Self {
            runtime,
            port,
            bindings: BTreeMap::new(),
        }
    }

    pub fn launch(&mut self) -> SessionId {
        let profile = TerminalProfile::default()
            .settings
            .resolve(&Default::default());
        let binding = self
            .runtime
            .block_on(self.port.adapter.launch_local(PaneId::new(), profile))
            .unwrap();
        let id = binding.id;
        self.bindings.insert(id, binding);
        id
    }

    pub fn pump(&mut self, window: &mut FocusWindow) {
        for (&session, binding) in &mut self.bindings {
            while let Ok(event) = binding.events.try_recv() {
                match &event {
                    SessionUiEvent::State(state) => {
                        window.view.session_states.insert(session, *state);
                    }
                    SessionUiEvent::Failed(_)
                    | SessionUiEvent::Exited(_)
                    | SessionUiEvent::Crashed(_) => {
                        panic!("owned local session stopped unexpectedly")
                    }
                    _ => {}
                }
                window
                    .controller
                    .emit(MainWindowMsg::AppEvent(AppEvent::Session {
                        session,
                        event,
                    }));
            }
            if let Some(frame) = binding.frames.borrow_and_update().clone() {
                window.view.latest_frames.insert(session, frame.clone());
                window
                    .controller
                    .emit(MainWindowMsg::AppEvent(AppEvent::Session {
                        session,
                        event: SessionUiEvent::Frame(frame),
                    }));
            }
        }
        drain();
        assert!(
            self.port.failures.lock().unwrap().is_empty(),
            "real adapter command failed"
        );
    }

    pub fn ready(&self, session: SessionId, window: &FocusWindow) -> bool {
        window.view.session_states.get(&session) == Some(&SessionState::Connected)
            && window.view.latest_frames.contains_key(&session)
    }

    pub fn contains(&self, session: SessionId, token: &str) -> bool {
        self.bindings[&session]
            .frames
            .borrow()
            .as_ref()
            .is_some_and(|frame| {
                frame.rows.iter().any(|row| {
                    row.cells
                        .iter()
                        .map(|cell| cell.text.as_str())
                        .collect::<String>()
                        .contains(token)
                })
            })
    }

    pub fn shutdown(&self) {
        let result = self.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(6), self.port.adapter.shutdown_all()).await
        });
        let actors = self.port.manager().active_session_count();
        let children = self.port.manager().active_child_process_count();
        eprintln!("FOCUS_PTY_CLEANUP result={result:?} actors={actors} children={children}");
        if !std::thread::panicking() {
            assert!(matches!(result, Ok(Ok(()))));
            assert_eq!(actors, 0);
            assert_eq!(children, 0);
        }
    }
}

impl Drop for OwnedPty {
    fn drop(&mut self) {
        self.shutdown();
    }
}
