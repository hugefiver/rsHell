use rshell_core::{ConnectionId, InteractionId};

use crate::smoke_driver_completion_evidence::{
    commit_complete, host_key_outcome_complete, preview_complete,
};
use crate::smoke_driver_recovery::{interrupt_complete, reset_complete};
use crate::smoke_driver_sequences::{
    clipboard_sequence, color_sequence, paste_sequence, reconnect_sequence, resize_sequence,
    search_sequence, selection_sequence,
};
use crate::{SmokeAction, SmokeCounters, smoke_driver_observation::SmokeObservation};

pub(crate) struct CompletionContext<'a> {
    pub(crate) before: &'a SmokeCounters,
    pub(crate) now: &'a SmokeObservation,
    pub(crate) selected_connection: Option<ConnectionId>,
    pub(crate) selection_target: Option<ConnectionId>,
    pub(crate) shutdown_sent: bool,
    pub(crate) auth_interaction: Option<InteractionId>,
    pub(crate) auth_submits: bool,
    pub(crate) binding_required: bool,
}

impl<'a> CompletionContext<'a> {
    #[cfg(test)]
    pub(crate) fn new(before: &'a SmokeCounters, now: &'a SmokeObservation) -> Self {
        Self {
            before,
            now,
            selected_connection: None,
            selection_target: None,
            shutdown_sent: false,
            auth_interaction: None,
            auth_submits: false,
            binding_required: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn require_binding(mut self) -> Self {
        self.binding_required = true;
        self
    }
}

pub(crate) fn action_is_complete(
    action: &SmokeAction,
    context: &CompletionContext<'_>,
    contains: impl Fn(&str) -> bool,
) -> bool {
    let before = context.before;
    let now = context.now;
    let selected_connection = context.selected_connection;
    let selection_target = context.selection_target;
    let shutdown_sent = context.shutdown_sent;
    let auth_interaction = context.auth_interaction;
    let auth_submits = context.auth_submits;
    let complete = match action {
        SmokeAction::WaitWindowRealized => now.window_realized,
        SmokeAction::NewTab => now.counters.tabs > before.tabs,
        SmokeAction::OpenConnectionEditor => {
            now.editor_open && now.counters.editor_revisions > before.editor_revisions
        }
        SmokeAction::SetConnectionField(_) => {
            now.editor_open && now.counters.editor_revisions > before.editor_revisions
        }
        SmokeAction::SubmitConnection => now.counters.catalog_changes > before.catalog_changes,
        SmokeAction::SelectConnection(_) => now.sidebar_selection == selection_target,
        SmokeAction::Connect => {
            selected_connection.is_some_and(|id| now.connection_panes.contains(&id))
                && now.counters.active_session_state.is_some_and(|state| {
                    !matches!(
                        state,
                        rshell_core::SessionState::Created | rshell_core::SessionState::Connecting
                    )
                })
        }
        SmokeAction::RespondHostKey { accept } => auth_interaction.is_some_and(|interaction| {
            now.counters.interaction_responses > before.interaction_responses
                && now.last_interaction_response == Some(interaction)
                && host_key_outcome_complete(now.counters.active_session_state, *accept)
        }),
        SmokeAction::RespondAuth { prompt, .. } => match auth_interaction {
            Some(interaction) if auth_submits => {
                now.counters.interaction_responses > before.interaction_responses
                    && now.last_interaction_response == Some(interaction)
                    && now.counters.active_session_state
                        == Some(rshell_core::SessionState::Connected)
            }
            Some(interaction) => {
                now.active_interaction == Some(interaction) && now.answered_prompts.contains(prompt)
            }
            None => false,
        },
        SmokeAction::SendTerminalText {
            expected_color_marker: Some(marker),
            ..
        } => now.counters.terminal.color.is_some_and(|evidence| {
            evidence.sequence > color_sequence(before)
                && evidence.marker_bytes == marker.len()
                && evidence.marker_cells > 0
                && evidence.non_default_foreground
                && evidence.red_foreground
        }),
        SmokeAction::SendTerminalText { .. } => {
            now.counters.terminal_commands > before.terminal_commands
        }
        SmokeAction::PasteTextFromEnv { .. } => {
            now.counters.terminal.paste.is_some_and(|evidence| {
                evidence.sequence > paste_sequence(before)
                    && evidence.command_exact
                    && evidence.frame_effect
                    && evidence.expected_bytes > 0
                    && evidence.actual_bytes == evidence.expected_bytes
            })
        }
        SmokeAction::ResizeTerminal {
            width,
            height,
            scale,
        } => now.counters.terminal.resize.is_some_and(|evidence| {
            evidence.sequence > resize_sequence(before)
                && evidence.input_width == *width
                && evidence.input_height == *height
                && evidence.input_scale_bits == scale.to_bits()
                && evidence.exact
                && evidence.observed.is_some()
        }),
        SmokeAction::SearchTerminal {
            text,
            case_sensitive,
            regex,
        } => now.counters.terminal.search.is_some_and(|evidence| {
            evidence.sequence > search_sequence(before)
                && evidence.query_bytes == text.len()
                && evidence.case_sensitive == *case_sensitive
                && evidence.regex == *regex
                && evidence.completed
                && evidence.match_count > 0
                && evidence.current.is_some()
        }),
        SmokeAction::SelectRange {
            start_x,
            start_y,
            end_x,
            end_y,
            rectangular,
            ..
        } => now.counters.terminal.selection.is_some_and(|evidence| {
            evidence.sequence > selection_sequence(before)
                && evidence.start_x_bits == start_x.to_bits()
                && evidence.start_y_bits == start_y.to_bits()
                && evidence.end_x_bits == end_x.to_bits()
                && evidence.end_y_bits == end_y.to_bits()
                && evidence.rectangular == *rectangular
                && evidence.frame_confirmed
        }),
        SmokeAction::Reconnect => now.counters.terminal.reconnect.is_some_and(|evidence| {
            evidence.sequence > reconnect_sequence(before)
                && evidence.new_session.is_some()
                && evidence.old_session_absent
        }),
        SmokeAction::VisualCheckpoint(checkpoint) => {
            now.visual_checkpoint_complete
                && now
                    .counters
                    .visual
                    .get(&checkpoint.id)
                    .is_some_and(|visual| {
                        visual.checkpoint_id == checkpoint.id
                            && visual.state == checkpoint.state
                            && visual.layout == checkpoint.expected_mode
                            && visual.facts.contract_passes()
                    })
        }
        SmokeAction::InterruptTerminal => now
            .counters
            .terminal
            .interrupt
            .is_some_and(|evidence| interrupt_complete(before.terminal.interrupt, evidence)),
        SmokeAction::ResetDisplay => now
            .counters
            .terminal
            .interrupt
            .is_some_and(|evidence| reset_complete(before.terminal.interrupt, evidence)),
        SmokeAction::ResizeWindow {
            width,
            height,
            expected_mode,
        } => now.counters.window_resize.is_some_and(|evidence| {
            evidence.sequence > before.window_resize.map_or(0, |prior| prior.sequence)
                && evidence.requested_width == *width
                && evidence.requested_height == *height
                && evidence.realized_width > 0
                && evidence.realized_height > 0
                && evidence.expected_layout == *expected_mode
                && evidence.layout == *expected_mode
        }),
        SmokeAction::WaitFrameContains(text) => contains(text),
        SmokeAction::SplitHorizontal | SmokeAction::SplitVertical => {
            now.counters.panes > before.panes
        }
        SmokeAction::SwitchTab(index) => now.active_tab == now.tab_ids.get(*index).copied(),
        SmokeAction::CopySelection => now.counters.terminal.clipboard.is_some_and(|evidence| {
            evidence.sequence > clipboard_sequence(before)
                && evidence.actor_exact
                && evidence.gtk_written
        }),
        SmokeAction::PreviewImport {
            source,
            expected: Some(expected),
            ..
        } => preview_complete(*source, expected, before, now),
        SmokeAction::PreviewImport { .. } => false,
        SmokeAction::CommitImport => commit_complete(before, now),
        SmokeAction::CancelImport => {
            now.counters.import_cancellations > before.import_cancellations
                && now.counters.imports.cancel_sequence > before.imports.cancel_sequence
                && now.counters.imports.cancel_pending_zero
                && now.counters.imports.cancelled_preview_matches
        }
        SmokeAction::CloseAll => {
            shutdown_sent
                && now.shutdown_complete
                && now.counters.panes == 0
                && now.counters.sessions == 0
        }
    };
    complete
        && (!context.binding_required
            || now
                .binding
                .as_ref()
                .is_some_and(|binding| binding.verified && binding.component_verified))
}
