#![allow(dead_code)]

pub mod ssh_server;

use std::{
    collections::VecDeque,
    future::pending,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use rshell_core::{
    CellAttributes, Color, InteractionRequest, RenderCell, RenderCursor, RenderFrame, RenderRow,
    SearchMatch, SearchQuery, SelectionRange, SessionFailure, TerminalInput, TerminalMouseEvent,
    TerminalSize, Viewport,
};
use rshell_session::{
    EngineDelta, EngineError, InteractionBroker, SessionLaunch, SessionTransport, TerminalEngine,
    TransportCapabilities, TransportError, TransportEvent, TransportFactory, TransportRequest,
};
use tokio::sync::Semaphore;

type RecordedWrites = Arc<Mutex<Vec<(usize, Vec<u8>)>>>;

pub fn size() -> TerminalSize {
    TerminalSize {
        cols: 80,
        rows: 24,
        pixel_width: 640,
        pixel_height: 384,
        dpi: 96,
    }
}

#[derive(Clone)]
pub struct WriteBlocker {
    started: Arc<Semaphore>,
    release: Arc<Semaphore>,
}

impl WriteBlocker {
    pub fn new() -> Self {
        Self {
            started: Arc::new(Semaphore::new(0)),
            release: Arc::new(Semaphore::new(0)),
        }
    }

    pub async fn wait_started(&self) {
        tokio::time::timeout(Duration::from_secs(1), self.started.acquire())
            .await
            .expect("write did not start")
            .expect("started semaphore closed")
            .forget();
    }

    pub fn release(&self) {
        self.release.add_permits(1);
    }
}

pub enum NextBehavior {
    Events(VecDeque<TransportEvent>),
    Burst {
        next: usize,
        end: usize,
        outputs_per_tick: usize,
        interval: Duration,
        started_at: Option<tokio::time::Instant>,
    },
    Panic,
    Pending,
}

pub struct TransportScript {
    pub next: NextBehavior,
    pub interactions: VecDeque<InteractionRequest>,
    pub write_blocker: Option<WriteBlocker>,
    pub shutdown_failure: Option<SessionFailure>,
}

impl TransportScript {
    pub fn pending() -> Self {
        Self {
            next: NextBehavior::Pending,
            interactions: VecDeque::new(),
            write_blocker: None,
            shutdown_failure: None,
        }
    }

    pub fn events(events: impl IntoIterator<Item = TransportEvent>) -> Self {
        Self {
            next: NextBehavior::Events(events.into_iter().collect()),
            interactions: VecDeque::new(),
            write_blocker: None,
            shutdown_failure: None,
        }
    }

    pub fn burst(end: usize) -> Self {
        Self {
            next: NextBehavior::Burst {
                next: 1,
                end,
                outputs_per_tick: 40,
                interval: Duration::from_millis(1),
                started_at: None,
            },
            interactions: VecDeque::new(),
            write_blocker: None,
            shutdown_failure: None,
        }
    }

    pub fn panic() -> Self {
        Self {
            next: NextBehavior::Panic,
            interactions: VecDeque::new(),
            write_blocker: None,
            shutdown_failure: None,
        }
    }

    pub fn interacting(request: InteractionRequest) -> Self {
        Self::interacting_many([request])
    }

    pub fn interacting_many(requests: impl IntoIterator<Item = InteractionRequest>) -> Self {
        Self {
            next: NextBehavior::Pending,
            interactions: requests.into_iter().collect(),
            write_blocker: None,
            shutdown_failure: None,
        }
    }

    pub fn with_write_blocker(mut self, blocker: WriteBlocker) -> Self {
        self.write_blocker = Some(blocker);
        self
    }

    pub fn with_shutdown_failure(mut self, failure: SessionFailure) -> Self {
        self.shutdown_failure = Some(failure);
        self
    }
}

#[derive(Clone)]
pub struct FactoryProbe {
    log: Arc<Mutex<Vec<String>>>,
    writes: RecordedWrites,
}

impl FactoryProbe {
    pub fn log(&self) -> Vec<String> {
        lock(&self.log).clone()
    }

    pub fn writes(&self) -> Vec<(usize, Vec<u8>)> {
        lock(&self.writes).clone()
    }

    pub fn shared_log(&self) -> Arc<Mutex<Vec<String>>> {
        Arc::clone(&self.log)
    }
}

pub struct FakeFactory {
    scripts: Mutex<VecDeque<TransportScript>>,
    probe: FactoryProbe,
    next_id: Mutex<usize>,
}

impl FakeFactory {
    pub fn new(scripts: impl IntoIterator<Item = TransportScript>) -> (Arc<Self>, FactoryProbe) {
        let probe = FactoryProbe {
            log: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(Vec::new())),
        };
        (
            Arc::new(Self {
                scripts: Mutex::new(scripts.into_iter().collect()),
                probe: probe.clone(),
                next_id: Mutex::new(0),
            }),
            probe,
        )
    }
}

impl TransportFactory for FakeFactory {
    fn create(
        &self,
        _request: &TransportRequest,
    ) -> Result<Box<dyn SessionTransport>, TransportError> {
        let script = lock(&self.scripts)
            .pop_front()
            .ok_or_else(|| TransportError::new(SessionFailure::Validation))?;
        let id = {
            let mut next_id = lock(&self.next_id);
            *next_id += 1;
            *next_id
        };
        lock(&self.probe.log).push(format!("create:{id}"));
        Ok(Box::new(FakeTransport {
            id,
            next: script.next,
            interactions: script.interactions,
            write_blocker: script.write_blocker,
            block_next_write: true,
            shutdown_failure: script.shutdown_failure,
            probe: self.probe.clone(),
        }))
    }
}

struct FakeTransport {
    id: usize,
    next: NextBehavior,
    interactions: VecDeque<InteractionRequest>,
    write_blocker: Option<WriteBlocker>,
    block_next_write: bool,
    shutdown_failure: Option<SessionFailure>,
    probe: FactoryProbe,
}

#[async_trait]
impl SessionTransport for FakeTransport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities::default()
    }

    async fn connect(
        &mut self,
        _request: &TransportRequest,
        interactions: InteractionBroker,
    ) -> Result<(), TransportError> {
        lock(&self.probe.log).push(format!("connect:{}", self.id));
        while let Some(request) = self.interactions.pop_front() {
            let _response = interactions.request(request).await?;
            lock(&self.probe.log).push(format!("interaction-response:{}", self.id));
        }
        Ok(())
    }

    async fn next_event(&mut self) -> Result<TransportEvent, TransportError> {
        match &mut self.next {
            NextBehavior::Events(events) => match events.pop_front() {
                Some(event) => Ok(event),
                None => pending().await,
            },
            NextBehavior::Burst {
                next,
                end,
                outputs_per_tick,
                interval,
                started_at,
            } if *next <= *end => {
                if (*next - 1) % *outputs_per_tick == 0 {
                    let started_at = *started_at.get_or_insert_with(tokio::time::Instant::now);
                    let tick = ((*next - 1) / *outputs_per_tick + 1) as u32;
                    tokio::time::sleep_until(started_at + *interval * tick).await;
                }
                let line = format!("line{}\r\n", *next);
                *next += 1;
                Ok(TransportEvent::Output(line.into_bytes()))
            }
            NextBehavior::Burst { .. } | NextBehavior::Pending => pending().await,
            NextBehavior::Panic => panic!("fake transport panic"),
        }
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.block_next_write
            && let Some(blocker) = &self.write_blocker
        {
            self.block_next_write = false;
            blocker.started.add_permits(1);
            blocker
                .release
                .acquire()
                .await
                .map_err(|_| TransportError::new(SessionFailure::Crashed))?
                .forget();
        }
        lock(&self.probe.writes).push((self.id, bytes.to_vec()));
        Ok(())
    }

    async fn resize(&mut self, _size: TerminalSize) -> Result<(), TransportError> {
        lock(&self.probe.log).push(format!("transport:resize:{}", self.id));
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), TransportError> {
        lock(&self.probe.log).push(format!("shutdown:{}", self.id));
        self.shutdown_failure
            .map_or(Ok(()), |failure| Err(TransportError::new(failure)))
    }
}

#[derive(Clone)]
pub struct EngineProbe {
    state: Arc<Mutex<EngineState>>,
}

impl EngineProbe {
    pub fn bytes(&self) -> Vec<u8> {
        lock(&self.state).bytes.clone()
    }

    pub fn render_count(&self) -> usize {
        lock(&self.state).renders
    }
}

struct EngineState {
    bytes: Vec<u8>,
    renders: usize,
    generation: u64,
    size: TerminalSize,
    log: Arc<Mutex<Vec<String>>>,
}

pub struct FakeEngine {
    state: Arc<Mutex<EngineState>>,
}

impl FakeEngine {
    pub fn new(log: Arc<Mutex<Vec<String>>>) -> (Self, EngineProbe) {
        let state = Arc::new(Mutex::new(EngineState {
            bytes: Vec::new(),
            renders: 0,
            generation: 0,
            size: size(),
            log,
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            EngineProbe { state },
        )
    }
}

impl TerminalEngine for FakeEngine {
    fn advance(&mut self, bytes: &[u8]) -> Result<EngineDelta, EngineError> {
        lock(&self.state).bytes.extend_from_slice(bytes);
        Ok(EngineDelta {
            outbound: Vec::new(),
            dirty: !bytes.is_empty(),
        })
    }

    fn resize(&mut self, size: TerminalSize) -> Result<(), EngineError> {
        let mut state = lock(&self.state);
        lock(&state.log).push("engine:resize".to_owned());
        state.size = size;
        Ok(())
    }

    fn render(
        &mut self,
        viewport: Viewport,
        selection: Option<SelectionRange>,
    ) -> Result<Arc<RenderFrame>, EngineError> {
        let mut state = lock(&self.state);
        state.renders += 1;
        state.generation += 1;
        let text = String::from_utf8_lossy(&state.bytes)
            .lines()
            .next_back()
            .unwrap_or_default()
            .to_owned();
        let cell = RenderCell {
            text,
            width: 1,
            foreground: Color::Default,
            background: Color::Default,
            attributes: CellAttributes::default(),
            selected: selection.is_some(),
        };
        Ok(Arc::new(RenderFrame {
            generation: state.generation,
            size: state.size,
            viewport_top: viewport.top_stable_row,
            rows: Arc::from([RenderRow {
                stable_row: viewport.top_stable_row,
                wrapped: false,
                cells: Arc::from([cell]),
            }]),
            cursor: None::<RenderCursor>,
            title: "fake".to_owned(),
            alternate_screen: false,
            mouse_reporting: false,
        }))
    }

    fn encode_input(&mut self, input: TerminalInput) -> Result<Vec<u8>, EngineError> {
        match input {
            TerminalInput::CommittedText(text) => Ok(text.into_bytes()),
            TerminalInput::Key { .. } => Err(EngineError::UnsupportedInput("fake key")),
        }
    }

    fn encode_mouse(&mut self, input: TerminalMouseEvent) -> Result<Vec<u8>, EngineError> {
        Ok(format!(
            "mouse:{:?}:{}:{}:{}",
            input.kind, input.cell.column, input.cell.stable_row, input.viewport_row
        )
        .to_ascii_lowercase()
        .into_bytes())
    }

    fn scroll(&mut self, _delta_rows: i32) -> Result<(), EngineError> {
        Ok(())
    }

    fn search(&self, _query: &SearchQuery) -> Result<Vec<SearchMatch>, EngineError> {
        Ok(Vec::new())
    }

    fn selected_text(&self, _range: SelectionRange) -> Result<String, EngineError> {
        Ok(String::new())
    }
}

pub fn launch(probe: &FactoryProbe) -> (SessionLaunch, EngineProbe) {
    let (engine, engine_probe) = FakeEngine::new(probe.shared_log());
    (
        SessionLaunch::new(TransportRequest::new(size()), Box::new(engine)),
        engine_probe,
    )
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}
