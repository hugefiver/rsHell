use rshell_core::{
    RenderFrame, SearchQuery, SelectionRange, SessionId, SessionUiCommand, SessionUiEvent,
    UiCommand,
};

use crate::{
    MainWindow, SmokeCellRangeEvidence, SmokeClipboardEvidence, SmokeSearchEvidence,
    SmokeSelectionEvidence,
};

pub(crate) struct PendingSearch {
    session: SessionId,
    query: SearchQuery,
}

pub(crate) struct PendingSelection {
    session: SessionId,
}

impl MainWindow {
    pub(crate) fn prepare_smoke_selection(
        &mut self,
        expected_text: Option<String>,
        expect_wide_midpoint: bool,
        input_bits: [u64; 4],
    ) {
        self.smoke_state.copy_expected = expected_text;
        self.smoke_state.selection_expect_wide = expect_wide_midpoint;
        self.smoke_state.selection_input_bits = Some(input_bits);
    }

    pub(crate) fn observe_smoke_terminal_command(&mut self, command: &UiCommand) {
        let UiCommand::Session { session, command } = command else {
            return;
        };
        let latest_frame = self.view_model.latest_frames.get(session).cloned();
        let generation = latest_frame.as_ref().map_or(0, |frame| frame.generation);
        self.observe_smoke_terminal_effect_command(
            *session,
            command,
            generation,
            latest_frame.as_deref(),
        );
        match command {
            SessionUiCommand::Resize(size) => {
                self.observe_smoke_resize_command(*session, *size, generation);
            }
            SessionUiCommand::Search(query) => {
                self.smoke_state.pending_search = Some(PendingSearch {
                    session: *session,
                    query: query.clone(),
                });
            }
            SessionUiCommand::Select(range) => {
                let wide_midpoint = self
                    .view_model
                    .latest_frames
                    .get(session)
                    .is_some_and(|frame| position_is_wide_midpoint(frame, range.start));
                self.smoke_state.terminal.selection = Some(SmokeSelectionEvidence {
                    sequence: 0,
                    start_x_bits: self
                        .smoke_state
                        .selection_input_bits
                        .map_or(0, |bits| bits[0]),
                    start_y_bits: self
                        .smoke_state
                        .selection_input_bits
                        .map_or(0, |bits| bits[1]),
                    end_x_bits: self
                        .smoke_state
                        .selection_input_bits
                        .map_or(0, |bits| bits[2]),
                    end_y_bits: self
                        .smoke_state
                        .selection_input_bits
                        .map_or(0, |bits| bits[3]),
                    range: range_evidence(*range),
                    rectangular: range.rectangular,
                    wide_midpoint,
                    frame_confirmed: false,
                });
                self.smoke_state.pending_selection = Some(PendingSelection { session: *session });
            }
            SessionUiCommand::CopySelection => {
                self.smoke_state.copy_session = Some(*session);
            }
            _ => {}
        }
    }

    pub(crate) fn observe_smoke_session_event(
        &mut self,
        session: SessionId,
        event: &SessionUiEvent,
    ) {
        match event {
            SessionUiEvent::Frame(frame) => self.observe_smoke_frame(session, frame),
            SessionUiEvent::Search(matches) => {
                let query = self
                    .smoke_state
                    .pending_search
                    .as_ref()
                    .filter(|pending| pending.session == session)
                    .map(|pending| pending.query.clone());
                if let Some(query) = query {
                    let sequence = self.next_smoke_evidence_sequence();
                    self.smoke_state.terminal.search = Some(SmokeSearchEvidence {
                        sequence,
                        query_bytes: query.needle.len(),
                        case_sensitive: query.case_sensitive,
                        regex: query.regex,
                        match_count: matches.len(),
                        current: matches.first().copied().map(search_evidence),
                        completed: true,
                    });
                    self.smoke_state.pending_search = None;
                }
            }
            SessionUiEvent::Copy(text) if self.smoke_state.copy_session == Some(session) => {
                let expected = self.smoke_state.copy_expected.as_deref().unwrap_or("");
                self.smoke_state.terminal.clipboard = Some(SmokeClipboardEvidence {
                    sequence: 0,
                    expected_bytes: expected.len(),
                    actual_bytes: text.len(),
                    actor_exact: !expected.is_empty() && text == expected,
                    gtk_written: false,
                });
            }
            _ => {}
        }
    }

    pub(crate) fn observe_smoke_clipboard_write(&mut self, bytes: usize) {
        let completed = self
            .smoke_state
            .terminal
            .clipboard
            .as_ref()
            .is_some_and(|evidence| evidence.actor_exact && evidence.actual_bytes == bytes);
        let sequence = completed.then(|| self.next_smoke_evidence_sequence());
        if let Some(evidence) = &mut self.smoke_state.terminal.clipboard {
            evidence.gtk_written = evidence.actor_exact && evidence.actual_bytes == bytes;
            if let Some(sequence) = sequence {
                evidence.sequence = sequence;
            }
        }
    }

    fn observe_smoke_frame(&mut self, session: SessionId, frame: &RenderFrame) {
        self.observe_smoke_terminal_effect_frame(session, frame);
        self.observe_smoke_resize_frame(session, frame);
        if frame.alternate_screen {
            self.smoke_state.terminal.tui_entered = true;
        } else if self.smoke_state.terminal.tui_entered {
            self.smoke_state.terminal.tui_exited = true;
        }
        if let Some(pending) = self.smoke_state.pending_selection.as_ref()
            && selection_frame_confirms(pending.session, session, frame)
        {
            let sequence = self.next_smoke_evidence_sequence();
            if let Some(evidence) = &mut self.smoke_state.terminal.selection {
                evidence.frame_confirmed =
                    !self.smoke_state.selection_expect_wide || evidence.wide_midpoint;
                evidence.sequence = sequence;
            }
            self.smoke_state.pending_selection = None;
        }
    }
}

pub(crate) fn selection_frame_confirms(
    expected_session: SessionId,
    session: SessionId,
    frame: &RenderFrame,
) -> bool {
    expected_session == session
        && frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .any(|cell| cell.selected)
}

fn range_evidence(range: SelectionRange) -> SmokeCellRangeEvidence {
    SmokeCellRangeEvidence {
        start_row: range.start.stable_row,
        start_col: range.start.column,
        end_row: range.end.stable_row,
        end_col: range.end.column,
    }
}

fn search_evidence(found: rshell_core::SearchMatch) -> SmokeCellRangeEvidence {
    SmokeCellRangeEvidence {
        start_row: found.start.stable_row,
        start_col: found.start.column,
        end_row: found.end.stable_row,
        end_col: found.end.column,
    }
}

fn position_is_wide_midpoint(frame: &RenderFrame, position: rshell_core::CellPosition) -> bool {
    let Some(row) = frame
        .rows
        .iter()
        .find(|row| row.stable_row == position.stable_row)
    else {
        return false;
    };
    let mut column = 0u16;
    row.cells.iter().any(|cell| {
        let width = u16::from(cell.width.max(1));
        let matches =
            width > 1 && position.column > column && position.column < column.saturating_add(width);
        column = column.saturating_add(width);
        matches
    })
}
