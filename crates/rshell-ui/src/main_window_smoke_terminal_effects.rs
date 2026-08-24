use std::collections::BTreeSet;

use rshell_core::{Color, RenderFrame, SessionId, SessionUiCommand, TerminalInput};
use rshell_platform::ClipboardPolicy;

use crate::{MainWindow, SmokeColorEvidence, SmokePasteEvidence};

pub(crate) struct PreparedPaste {
    expected: String,
    effect_marker: String,
}

pub(crate) struct PendingPaste {
    session: SessionId,
    generation: u64,
    effect_marker: String,
    baseline_occurrences: BTreeSet<(i64, u16)>,
}

pub(crate) struct PreparedColor {
    text: String,
    marker: String,
}

pub(crate) struct PendingColor {
    session: SessionId,
    generation: u64,
    marker: String,
}

impl MainWindow {
    pub(crate) fn prepare_smoke_paste(
        &mut self,
        expected: String,
        effect_marker: String,
    ) -> Result<(), &'static str> {
        let expected =
            ClipboardPolicy::normalize_text(&expected).map_err(|_| "invalid_paste_environment")?;
        self.smoke_state.prepared_paste = Some(PreparedPaste {
            expected,
            effect_marker,
        });
        Ok(())
    }

    pub(crate) fn prepare_smoke_color(&mut self, text: String, marker: String) {
        self.smoke_state.prepared_color = Some(PreparedColor { text, marker });
    }

    pub(crate) fn observe_smoke_terminal_effect_command(
        &mut self,
        session: SessionId,
        command: &SessionUiCommand,
        generation: u64,
        baseline_frame: Option<&RenderFrame>,
    ) {
        if let Some(prepared) = self.smoke_state.prepared_paste.as_ref()
            && command.paste_len().is_some()
        {
            let actual_bytes = command.paste_len().unwrap_or_default();
            let command_exact = command.paste_matches(&prepared.expected);
            self.smoke_state.terminal.paste = Some(SmokePasteEvidence {
                sequence: 0,
                expected_bytes: prepared.expected.len(),
                actual_bytes,
                command_exact,
                frame_effect: false,
            });
            if command_exact {
                let prepared = self
                    .smoke_state
                    .prepared_paste
                    .take()
                    .expect("prepared paste");
                self.smoke_state.pending_paste = Some(PendingPaste {
                    session,
                    generation,
                    baseline_occurrences: baseline_frame.map_or_else(BTreeSet::new, |frame| {
                        marker_occurrences(frame, &prepared.effect_marker)
                    }),
                    effect_marker: prepared.effect_marker,
                });
            }
        }

        if let Some(prepared) = self.smoke_state.prepared_color.as_ref()
            && matches!(command, SessionUiCommand::Input(TerminalInput::CommittedText(text)) if text == &prepared.text)
        {
            let prepared = self
                .smoke_state
                .prepared_color
                .take()
                .expect("prepared color");
            self.smoke_state.pending_color = Some(PendingColor {
                session,
                generation,
                marker: prepared.marker,
            });
        }
    }

    pub(crate) fn observe_smoke_terminal_effect_frame(
        &mut self,
        session: SessionId,
        frame: &RenderFrame,
    ) {
        let paste_complete = self
            .smoke_state
            .pending_paste
            .as_ref()
            .is_some_and(|pending| {
                pending.session == session
                    && frame.generation > pending.generation
                    && has_new_marker_occurrence(
                        frame,
                        &pending.effect_marker,
                        &pending.baseline_occurrences,
                    )
            });
        if paste_complete {
            let sequence = self.next_smoke_evidence_sequence();
            if let Some(evidence) = &mut self.smoke_state.terminal.paste {
                evidence.sequence = sequence;
                evidence.frame_effect = true;
            }
            self.smoke_state.pending_paste = None;
        }

        let color = self.smoke_state.pending_color.as_ref().and_then(|pending| {
            (pending.session == session && frame.generation > pending.generation)
                .then(|| marker_style(frame, &pending.marker))
                .flatten()
        });
        if let Some((marker_cells, non_default_foreground, red_foreground)) = color {
            let marker_bytes = self
                .smoke_state
                .pending_color
                .as_ref()
                .map_or(0, |pending| pending.marker.len());
            let sequence = self.next_smoke_evidence_sequence();
            self.smoke_state.terminal.color = Some(SmokeColorEvidence {
                sequence,
                marker_bytes,
                marker_cells,
                non_default_foreground,
                red_foreground,
            });
            self.smoke_state.pending_color = None;
        }
    }
}

pub(crate) fn frame_contains_text(frame: &RenderFrame, needle: &str) -> bool {
    frame.rows.iter().any(|row| {
        row.cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
            .contains(needle)
    })
}

pub(crate) fn has_new_marker_occurrence(
    frame: &RenderFrame,
    marker: &str,
    baseline: &BTreeSet<(i64, u16)>,
) -> bool {
    if !frame_contains_text(frame, marker) {
        return false;
    }
    marker_occurrences(frame, marker)
        .iter()
        .any(|occurrence| !baseline.contains(occurrence))
}

pub(crate) fn marker_occurrences(frame: &RenderFrame, marker: &str) -> BTreeSet<(i64, u16)> {
    let mut occurrences = BTreeSet::new();
    for row in frame.rows.iter() {
        for start in 0..row.cells.len() {
            let mut text = String::new();
            for end in start..row.cells.len() {
                text.push_str(&row.cells[end].text);
                if text == marker {
                    let column = row.cells[..start]
                        .iter()
                        .map(|cell| u16::from(cell.width.max(1)))
                        .fold(0u16, u16::saturating_add);
                    occurrences.insert((row.stable_row, column));
                    break;
                }
                if text.len() >= marker.len() || !marker.starts_with(&text) {
                    break;
                }
            }
        }
    }
    occurrences
}

fn marker_style(frame: &RenderFrame, marker: &str) -> Option<(usize, bool, bool)> {
    for row in frame.rows.iter() {
        for start in 0..row.cells.len() {
            let mut text = String::new();
            for end in start..row.cells.len() {
                text.push_str(&row.cells[end].text);
                if text == marker {
                    let cells = &row.cells[start..=end];
                    let matched = (
                        cells.len(),
                        cells.iter().all(|cell| cell.foreground != Color::Default),
                        cells.iter().all(|cell| cell.foreground == Color::Ansi(1)),
                    );
                    if matched.2 {
                        return Some(matched);
                    }
                    break;
                }
                if text.len() >= marker.len() || !marker.starts_with(&text) {
                    break;
                }
            }
        }
    }
    None
}
