use rshell_core::{RenderFrame, SessionId, SessionUiCommand, SessionUiEvent};

use crate::{MainWindow, SmokeInterruptEvidence};

pub(crate) struct PendingRecovery {
    session: SessionId,
    generation: u64,
    reset: bool,
}

impl MainWindow {
    pub(crate) fn observe_smoke_recovery_command(
        &mut self,
        session: SessionId,
        command: &SessionUiCommand,
        generation: u64,
    ) {
        match command {
            SessionUiCommand::Interrupt => {
                let sequence = self.next_smoke_evidence_sequence();
                let command_count = self
                    .smoke_state
                    .terminal
                    .interrupt
                    .as_ref()
                    .filter(|evidence| evidence.reset_generation.is_none())
                    .map_or(1, |evidence| evidence.command_count.saturating_add(1));
                self.smoke_state.terminal.interrupt = Some(SmokeInterruptEvidence {
                    sequence,
                    command_count,
                    wire_byte: 0x03,
                    exact_etx: true,
                    enhanced_encoder_bypassed: true,
                    surviving_tui: false,
                    notice_visible: false,
                    reset_generation: None,
                    modes_clean: false,
                    replacement_character_count: 0,
                    old_tui_overlap: false,
                });
                self.smoke_state.pending_recovery = Some(PendingRecovery {
                    session,
                    generation,
                    reset: false,
                });
            }
            SessionUiCommand::ResetDisplay => {
                let dirty = self.smoke_state.terminal.interrupt.is_some_and(|evidence| {
                    evidence.surviving_tui
                        && evidence.notice_visible
                        && !evidence.modes_clean
                        && evidence.reset_generation.is_none()
                });
                if dirty {
                    self.smoke_state.pending_recovery = Some(PendingRecovery {
                        session,
                        generation,
                        reset: true,
                    });
                }
            }
            _ => {}
        }
    }

    pub(crate) fn observe_smoke_recovery_event(
        &mut self,
        session: SessionId,
        event: &SessionUiEvent,
    ) {
        let Some(pending) = self
            .smoke_state
            .pending_recovery
            .as_ref()
            .filter(|pending| pending.session == session)
        else {
            return;
        };
        if let SessionUiEvent::RecoveryChanged(notice) = event
            && let Some(evidence) = &mut self.smoke_state.terminal.interrupt
        {
            evidence.notice_visible = notice.is_some();
            if !pending.reset {
                evidence.surviving_tui = notice.is_some();
            }
        }
        let interrupt_complete = !pending.reset
            && self
                .smoke_state
                .terminal
                .interrupt
                .is_some_and(|evidence| evidence.notice_visible && evidence.old_tui_overlap);
        if interrupt_complete {
            self.smoke_state.pending_recovery = None;
        }
    }

    pub(crate) fn observe_smoke_recovery_frame(&mut self, session: SessionId, frame: &RenderFrame) {
        let Some(pending) = self
            .smoke_state
            .pending_recovery
            .as_ref()
            .filter(|pending| pending.session == session && frame.generation > pending.generation)
        else {
            return;
        };
        let reset = pending.reset;
        let modes_clean = !frame.display_modes.has_residue();
        let replacement_character_count = frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| cell.text.matches('\u{fffd}').count())
            .sum();
        let old_tui_overlap = frame.display_modes.has_residue()
            || frame.alternate_screen
            || frame.title == crate::main_window_smoke_evidence::P0_TUI_ACTIVE_TITLE;
        let sequence = self.next_smoke_evidence_sequence();
        if let Some(evidence) = &mut self.smoke_state.terminal.interrupt {
            evidence.sequence = sequence;
            evidence.modes_clean = modes_clean;
            evidence.replacement_character_count = replacement_character_count;
            evidence.old_tui_overlap = old_tui_overlap;
            if reset && modes_clean {
                evidence.notice_visible = false;
                evidence.reset_generation = Some(frame.generation);
            } else if !reset && !modes_clean {
                evidence.surviving_tui = true;
            }
        }
        let complete = reset && modes_clean
            || !reset
                && !modes_clean
                && self
                    .smoke_state
                    .terminal
                    .interrupt
                    .is_some_and(|evidence| evidence.notice_visible);
        if complete {
            self.smoke_state.pending_recovery = None;
        }
    }
}
