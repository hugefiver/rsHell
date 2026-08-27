use alacritty_terminal::{Term, grid::Dimensions, term::TermMode, vte::ansi::Processor};
use rshell_core::ResolvedTerminalProfile;

use crate::{alacritty_event::EventSink, alacritty_rows};

pub(crate) fn advance(
    processor: &mut Processor,
    terminal: &mut Term<EventSink>,
    events: &EventSink,
    settings: &ResolvedTerminalProfile,
    primary_origin: &mut i64,
    bytes: &[u8],
) -> Vec<u8> {
    let mut remaining = bytes;
    while let Some(sequence) = next_alt_switch(remaining) {
        advance_segment(
            processor,
            terminal,
            events,
            settings,
            primary_origin,
            &remaining[..sequence.start],
        );
        advance_segment(
            processor,
            terminal,
            events,
            settings,
            primary_origin,
            &remaining[sequence.clone()],
        );
        remaining = &remaining[sequence.end..];
    }
    advance_segment(
        processor,
        terminal,
        events,
        settings,
        primary_origin,
        remaining,
    );
    events.take_outbound()
}

fn advance_segment(
    processor: &mut Processor,
    terminal: &mut Term<EventSink>,
    events: &EventSink,
    settings: &ResolvedTerminalProfile,
    primary_origin: &mut i64,
    bytes: &[u8],
) {
    if bytes.is_empty() {
        return;
    }
    let was_primary = !terminal.mode().contains(TermMode::ALT_SCREEN);
    let old_history = terminal.grid().history_size();
    let capacity_anchor = was_primary
        .then(|| alacritty_rows::capture(terminal, settings.scrollback_lines, bytes))
        .flatten();

    let mut remaining = bytes;
    while let Some(index) = remaining.iter().position(|byte| *byte == 0x05) {
        processor.advance(terminal, &remaining[..=index]);
        events.push_outbound(settings.answerback.as_bytes());
        remaining = &remaining[index + 1..];
    }
    processor.advance(terminal, remaining);

    if !terminal.mode().contains(TermMode::ALT_SCREEN) {
        let history = terminal.grid().history_size();
        if was_primary {
            let shift = history.saturating_sub(old_history).saturating_add(
                alacritty_rows::completed_shift(
                    terminal,
                    settings.scrollback_lines,
                    capacity_anchor,
                ),
            );
            *primary_origin = primary_origin.saturating_add(shift as i64);
        }
    }
}

fn next_alt_switch(bytes: &[u8]) -> Option<std::ops::Range<usize>> {
    for start in 0..bytes.len().saturating_sub(3) {
        if !bytes[start..].starts_with(b"\x1b[?") {
            continue;
        }
        let tail = &bytes[start + 3..];
        let final_offset = tail.iter().position(|byte| (0x40..=0x7e).contains(byte))?;
        let final_byte = tail[final_offset];
        if !matches!(final_byte, b'h' | b'l') {
            continue;
        }
        let parameters = &tail[..final_offset];
        if parameters
            .split(|byte| *byte == b';')
            .any(|value| value == b"1049")
        {
            return Some(start..start + 3 + final_offset + 1);
        }
    }
    None
}
