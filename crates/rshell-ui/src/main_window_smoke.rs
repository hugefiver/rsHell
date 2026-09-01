use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::{ConnectionId, ImportSourceKind, InteractionId, SessionId};

use crate::{
    ConnectionEditorDraftState, ConnectionEditorState, ImportDialogState, InteractionDialogState,
    MainWindow, MainWindowMsg, SmokeImportEvidence, SmokeImportExpectation, SmokeTerminalEvidence,
    main_window_smoke_capture::capture_widget_png,
    main_window_smoke_evidence::{PendingSearch, PendingSelection},
    main_window_smoke_observation::frame_contains,
    main_window_smoke_resize::{PendingResize, PreparedResize},
    main_window_smoke_terminal_effects::{
        PendingColor, PendingPaste, PreparedColor, PreparedPaste,
    },
    main_window_smoke_visual::VisualCheckpointPhase,
    main_window_smoke_workflow_evidence::{ImportBaseline, PendingImportPreview, PendingReconnect},
    smoke_driver_state::{SmokeDecision, SmokeDriver},
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct SmokeUiState {
    pub window_realized: bool,
    pub editor_open: bool,
    pub editor_revision: u64,
    pub editor_draft: Option<ConnectionEditorDraftState>,
    pub sidebar_selection: Option<ConnectionId>,
    pub interaction_revision: u64,
    pub interaction: Option<InteractionId>,
    pub interaction_pending: bool,
    pub interaction_prompt_count: usize,
    pub interaction_answered_prompts: Vec<usize>,
    pub last_interaction_response: Option<InteractionId>,
    pub import_preview_ready: bool,
    pub import_revision: u64,
    pub active_tab: Option<rshell_core::TabId>,
    pub pane_host_session: Option<SessionId>,
    pub pane_host_geometry_session: Option<SessionId>,
    pub shutdown_complete: bool,
    pub terminal_commands: u64,
    pub clipboard_writes: u64,
    pub clipboard_bytes: Option<usize>,
    pub catalog_changes: u64,
    pub interaction_responses: u64,
    pub import_completions: u64,
    pub import_cancellations: u64,
    pub close_all_last_tabs: Option<usize>,
    pub terminal: SmokeTerminalEvidence,
    pub imports: SmokeImportEvidence,
    pub pending_resize: Option<PendingResize>,
    pub resize_input: Option<PreparedResize>,
    pub pending_search: Option<PendingSearch>,
    pub pending_selection: Option<PendingSelection>,
    pub selection_expect_wide: bool,
    pub selection_input_bits: Option<[u64; 4]>,
    pub copy_expected: Option<String>,
    pub copy_session: Option<SessionId>,
    pub prepared_paste: Option<PreparedPaste>,
    pub pending_paste: Option<PendingPaste>,
    pub prepared_color: Option<PreparedColor>,
    pub pending_color: Option<PendingColor>,
    pub tui_session: Option<SessionId>,
    pub pending_reconnect: Option<PendingReconnect>,
    pub pending_recovery: Option<crate::main_window_smoke_recovery::PendingRecovery>,
    pub import_expectation: Option<SmokeImportExpectation>,
    pub import_source: Option<ImportSourceKind>,
    pub import_baseline: Option<ImportBaseline>,
    pub pending_import_preview: Option<PendingImportPreview>,
    pub evidence_sequence: u64,
    pub visual_checkpoint: VisualCheckpointPhase,
    pub active_visual_checkpoint: Option<crate::SmokeVisualCheckpoint>,
    pub visual_focus_trigger: Option<gtk::Widget>,
    pub modal_escape_verified: bool,
    pub modal_focus_restore_verified: bool,
    pub visual: Option<crate::SmokeVisualEvidence>,
    pub visuals: BTreeMap<String, crate::SmokeVisualCheckpointEvidence>,
    pub visual_capture_attempted: bool,
    pub visual_completion_tick_pending: bool,
    pub visual_paintable: Option<gtk::WidgetPaintable>,
    pub visual_accent_paintable: Option<gtk::WidgetPaintable>,
    pub visual_stage_count: Option<usize>,
    pub window_resize: Option<crate::SmokeWindowResizeEvidence>,
}

impl MainWindow {
    pub(crate) fn next_smoke_evidence_sequence(&mut self) -> u64 {
        self.smoke_state.evidence_sequence = self.smoke_state.evidence_sequence.saturating_add(1);
        self.smoke_state.evidence_sequence
    }
    pub(crate) fn observe_smoke_editor(&mut self, state: ConnectionEditorState) {
        self.smoke_state.editor_open = state.open;
        self.smoke_state.editor_revision = state.revision;
        self.smoke_state.editor_draft = state.draft;
        if state.has_error {
            self.fail_smoke("editor_rejected");
        }
    }

    pub(crate) fn observe_smoke_interaction(&mut self, state: InteractionDialogState) {
        self.smoke_state.interaction_revision = state.revision;
        self.smoke_state.interaction = state.interaction;
        self.smoke_state.interaction_pending = state.pending;
        self.smoke_state.interaction_prompt_count = state.prompt_count;
        self.smoke_state.interaction_answered_prompts = state.answered_prompts;
        if state.has_error {
            self.fail_smoke("interaction_rejected");
        }
    }

    pub(crate) fn observe_smoke_import(&mut self, state: ImportDialogState) {
        self.smoke_state.import_preview_ready = state.preview_ready;
        self.smoke_state.import_revision = state.revision;
        if state.has_error {
            self.fail_smoke("import_rejected");
        }
    }

    pub(crate) fn fail_smoke(&mut self, code: &'static str) {
        let binding = self
            .smoke
            .as_ref()
            .and_then(SmokeDriver::current_binding_request);
        let observation = self.smoke_observation(binding.as_ref());
        let was_active = self.smoke.as_ref().is_some_and(SmokeDriver::is_active);
        if let Some(driver) = &mut self.smoke {
            driver.fail(&observation, code);
        }
        if was_active {
            self.capture_smoke_png();
            relm4::main_application().quit();
        }
    }

    pub(crate) fn drive_smoke(&mut self, sender: &ComponentSender<Self>) {
        self.refresh_smoke_window_allocation();
        let binding = self
            .smoke
            .as_ref()
            .and_then(SmokeDriver::current_binding_request);
        let observation = self.smoke_observation(binding.as_ref());
        let view_model = &self.view_model;
        let active_tab = self
            .smoke_state
            .active_tab
            .or(self.view_model.workspace.active_tab);
        let decision = self.smoke.as_mut().and_then(|driver| {
            driver.tick(&observation, |needle| {
                frame_contains(view_model, active_tab, needle)
            })
        });
        match decision {
            Some(SmokeDecision::Route(action)) => match self.route_smoke_action(action) {
                Ok(true) => {}
                Ok(false) => {
                    if let Some(driver) = &mut self.smoke {
                        driver.defer_current_route();
                    }
                }
                Err(code) => self.fail_smoke(code),
            },
            Some(SmokeDecision::Quit) => {
                self.capture_smoke_png();
                relm4::main_application().quit();
            }
            None => {}
        }
        self.schedule_smoke_tick(sender);
    }

    fn schedule_smoke_tick(&mut self, sender: &ComponentSender<Self>) {
        if self.smoke_tick_pending || !self.smoke.as_ref().is_some_and(SmokeDriver::is_active) {
            return;
        }
        self.smoke_tick_pending = true;
        if queue_visual_completion_tick(
            &mut self.smoke_state.visual_completion_tick_pending,
            |message| sender.input(message),
        ) {
            return;
        }
        let sender = sender.input_sender().clone();
        if self.smoke_state.window_resize.is_some_and(|evidence| {
            evidence.realized_width == 0
                || evidence.realized_height == 0
                || evidence.layout != evidence.expected_layout
        }) {
            crate::main_window_smoke_frame::schedule_after_frame(&self.shell.overlay, sender);
            return;
        }
        if self.smoke_state.visual_checkpoint == VisualCheckpointPhase::Opening
            && self.smoke_state.visual_paintable.is_some()
        {
            crate::main_window_smoke_frame::schedule_after_frame(&self.shell.overlay, sender);
            return;
        }
        gtk::glib::timeout_add_local_full(
            std::time::Duration::from_millis(25),
            gtk::glib::Priority::DEFAULT,
            move || {
                send_smoke_tick(&sender);
                gtk::glib::ControlFlow::Break
            },
        );
    }

    pub(crate) fn capture_smoke_png(&mut self) {
        if self.smoke_state.visual_capture_attempted
            || self
                .smoke_state
                .visual
                .is_some_and(|visual| visual.png.is_some())
        {
            return;
        }
        let Some(path) = self.smoke_png_path.clone() else {
            return;
        };
        self.smoke_state.visual_capture_attempted = true;
        let facts = self
            .smoke_state
            .visual
            .map_or(crate::SmokeVisualFacts::default(), |visual| visual.facts);
        let result = self
            .smoke_paintable
            .as_ref()
            .ok_or("snapshot_paintable_unavailable")
            .and_then(|paintable| capture_widget_png(paintable, &path, facts));
        if let Some(driver) = &self.smoke {
            match result {
                Ok(evidence) => {
                    self.smoke_state.visual = Some(evidence);
                    driver.record_png_path(path);
                }
                Err(error) => driver.record_png_error(error),
            }
        }
    }
}

fn send_smoke_tick(sender: &relm4::Sender<MainWindowMsg>) {
    let _ = sender.send(MainWindowMsg::SmokeTick);
}

pub(crate) fn queue_visual_completion_tick(
    pending: &mut bool,
    enqueue: impl FnOnce(MainWindowMsg),
) -> bool {
    if !std::mem::take(pending) {
        return false;
    }
    enqueue(MainWindowMsg::SmokeTick);
    true
}
