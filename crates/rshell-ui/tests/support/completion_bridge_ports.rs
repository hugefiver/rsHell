use super::frame;
use async_trait::async_trait;
use rshell_core::*;
use secrecy::SecretString;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Ports {
    pub state: Arc<Mutex<State>>,
    gate: async_channel::Receiver<bool>,
    pub release: async_channel::Sender<bool>,
    initial_pending: bool,
}
#[derive(Default)]
pub struct State {
    pub attempts: usize,
    pub failures: usize,
    pub launched: Vec<(PaneId, SessionId)>,
    pub live: BTreeSet<SessionId>,
    pub inputs: Vec<SessionId>,
    sessions: BTreeMap<SessionId, SessionChannels>,
}
type SessionChannels = (
    async_channel::Sender<SessionUiEvent>,
    tokio::sync::watch::Sender<Option<Arc<RenderFrame>>>,
);
impl Ports {
    pub fn new() -> Self {
        let (release, gate) = async_channel::bounded(8);
        Self {
            state: Arc::new(Mutex::new(State::default())),
            gate,
            release,
            initial_pending: false,
        }
    }
    pub fn pending_initial() -> Self {
        Self {
            initial_pending: true,
            ..Self::new()
        }
    }
    pub fn ready_initial(&self) {
        let state = self.state.lock().unwrap();
        let session = state.launched[0].1;
        let (events, frames) = &state.sessions[&session];
        events
            .try_send(SessionUiEvent::State(SessionState::Connected))
            .unwrap();
        frames.send_replace(Some(frame(1)));
    }
    pub fn later_initial_frame(&self) {
        let state = self.state.lock().unwrap();
        let session = state.launched[0].1;
        state.sessions[&session].1.send_replace(Some(frame(2)));
    }
    pub fn dependencies(&self) -> AppDependencies {
        AppDependencies {
            repository: Arc::new(self.clone()),
            credentials: Arc::new(self.clone()),
            imports: Arc::new(self.clone()),
            sessions: Arc::new(self.clone()),
        }
    }
}
#[async_trait]
impl SessionPort for Ports {
    async fn launch_local(
        &self,
        pane: PaneId,
        _: ResolvedTerminalProfile,
    ) -> Result<SessionBinding, SessionFailure> {
        let attempt = {
            let mut state = self.state.lock().unwrap();
            state.attempts += 1;
            state.attempts
        };
        if attempt > 1
            && !self
                .gate
                .recv()
                .await
                .map_err(|_| SessionFailure::Crashed)?
        {
            self.state.lock().unwrap().failures += 1;
            return Err(SessionFailure::Pty);
        }
        let id = SessionId::new();
        let (events, event_rx) = async_channel::bounded(8);
        let pending = attempt == 1 && self.initial_pending;
        if !pending {
            events
                .try_send(SessionUiEvent::State(SessionState::Connected))
                .unwrap();
        }
        let (frames, frame_rx) = tokio::sync::watch::channel((!pending).then(|| frame(1)));
        let mut state = self.state.lock().unwrap();
        state.launched.push((pane, id));
        state.live.insert(id);
        state.sessions.insert(id, (events, frames));
        Ok(SessionBinding {
            id,
            events: event_rx,
            frames: frame_rx,
        })
    }
    async fn launch_ssh(
        &self,
        _: PaneId,
        _: ConnectionProfile,
        _: ResolvedTerminalProfile,
        _: TerminalSize,
        _: Option<SecretString>,
    ) -> Result<SessionBinding, SessionFailure> {
        panic!("no SSH in completion bridge fixture")
    }
    async fn command(
        &self,
        session: SessionId,
        command: SessionUiCommand,
    ) -> Result<(), SessionFailure> {
        if matches!(command, SessionUiCommand::Input(_)) {
            self.state.lock().unwrap().inputs.push(session);
        }
        Ok(())
    }
    async fn shutdown(&self, session: SessionId) -> Result<(), SessionFailure> {
        let mut state = self.state.lock().unwrap();
        state.live.remove(&session);
        state.sessions.remove(&session);
        Ok(())
    }
    async fn shutdown_all(&self) -> Result<(), SessionFailure> {
        let mut state = self.state.lock().unwrap();
        state.live.clear();
        state.sessions.clear();
        Ok(())
    }
}
