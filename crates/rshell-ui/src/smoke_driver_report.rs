use std::{cell::RefCell, collections::BTreeMap, path::PathBuf, rc::Rc, time::Duration};

use rshell_core::{ConnectionId, PaneId, SessionId, SessionState};

pub use crate::smoke_driver_evidence::{
    SmokeAccessibilityEvidence, SmokeCellRangeEvidence, SmokeClipboardEvidence, SmokeColorEvidence,
    SmokeDpiEvidence, SmokeFrameEvidence, SmokeImportEvidence, SmokeImportPreviewEvidence,
    SmokeInterruptEvidence, SmokePasteEvidence, SmokeReconnectEvidence, SmokeResizeEvidence,
    SmokeSearchEvidence, SmokeSelectionEvidence, SmokeTerminalEvidence,
    SmokeVisualCheckpointEvidence, SmokeWindowResizeEvidence,
};

use crate::{SmokeActionKind, SmokeDriverInit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeStepState {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeFieldStatus {
    Accepted,
    Rejected,
    NotObserved,
}

#[derive(Debug, Clone, Default)]
pub struct SmokeCounters {
    pub tabs: usize,
    pub panes: usize,
    pub sessions: usize,
    pub frames: usize,
    pub active_session: Option<SessionId>,
    pub active_session_state: Option<SessionState>,
    pub terminal_commands: u64,
    pub clipboard_writes: u64,
    pub clipboard_bytes: Option<usize>,
    pub catalog_changes: u64,
    pub interaction_responses: u64,
    pub import_completions: u64,
    pub import_cancellations: u64,
    pub latest_frame: Option<SmokeFrameEvidence>,
    pub editor_revisions: u64,
    pub interaction_revisions: u64,
    pub import_revisions: u64,
    pub terminal: SmokeTerminalEvidence,
    pub imports: SmokeImportEvidence,
    pub visual: BTreeMap<String, SmokeVisualCheckpointEvidence>,
    pub window_resize: Option<SmokeWindowResizeEvidence>,
}

#[derive(Debug, Clone)]
pub struct SmokeStepReport {
    pub index: usize,
    pub action: SmokeActionKind,
    pub surface: Option<String>,
    pub connection: Option<String>,
    pub binding: Option<SmokeBindingEvidence>,
    pub state: SmokeStepState,
    pub elapsed: Duration,
    pub evidence: SmokeCounters,
    pub field_status: Option<SmokeFieldStatus>,
}

#[derive(Debug, Clone, Default)]
pub struct SmokeBindingEvidence {
    pub verified: bool,
    pub component_verified: bool,
    pub actual_label: Option<String>,
    pub connection_id: Option<ConnectionId>,
    pub profile_name: Option<String>,
    pub endpoint: Option<String>,
    pub pane_id: Option<PaneId>,
    pub session_id: Option<SessionId>,
    pub local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeScenarioState {
    Pending,
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SmokeFailure {
    pub step: Option<usize>,
    pub code: &'static str,
}

#[derive(Debug, Clone)]
pub struct SmokeReport {
    pub version: u16,
    pub run_nonce: String,
    pub state: SmokeScenarioState,
    pub elapsed: Duration,
    pub steps: Vec<SmokeStepReport>,
    pub counters: SmokeCounters,
    pub failure: Option<SmokeFailure>,
    pub requested_png_path: Option<PathBuf>,
    pub png_path: Option<PathBuf>,
    pub requested_png_paths: Vec<PathBuf>,
    pub png_paths: Vec<PathBuf>,
    pub png_error: Option<&'static str>,
}

#[derive(Clone)]
pub struct SmokeReportHandle(Rc<RefCell<SmokeReport>>);

impl SmokeReportHandle {
    pub(crate) fn new(init: &SmokeDriverInit) -> Self {
        let steps = init
            .scenario
            .actions
            .iter()
            .enumerate()
            .map(|(index, step)| SmokeStepReport {
                index,
                action: step.action.kind(),
                surface: step.surface.clone(),
                connection: step.connection.clone(),
                binding: None,
                state: SmokeStepState::Pending,
                elapsed: Duration::ZERO,
                evidence: SmokeCounters::default(),
                field_status: matches!(step.action, crate::SmokeAction::SetConnectionField(_))
                    .then_some(SmokeFieldStatus::NotObserved),
            })
            .collect();
        Self(Rc::new(RefCell::new(SmokeReport {
            version: init.scenario.version,
            run_nonce: init.scenario.run_nonce.clone(),
            state: SmokeScenarioState::Pending,
            elapsed: Duration::ZERO,
            steps,
            counters: SmokeCounters::default(),
            failure: None,
            requested_png_path: init.png_path.clone(),
            png_path: None,
            requested_png_paths: Vec::new(),
            png_paths: Vec::new(),
            png_error: None,
        })))
    }

    pub fn report(&self) -> SmokeReport {
        self.0.borrow().clone()
    }
    pub fn is_complete(&self) -> bool {
        matches!(
            self.0.borrow().state,
            SmokeScenarioState::Passed | SmokeScenarioState::Failed
        )
    }
    pub(crate) fn mutate(&self, update: impl FnOnce(&mut SmokeReport)) {
        update(&mut self.0.borrow_mut());
    }
}

impl std::fmt::Debug for SmokeReportHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SmokeReportHandle")
            .field("complete", &self.is_complete())
            .finish()
    }
}
