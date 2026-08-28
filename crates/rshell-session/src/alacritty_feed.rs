use alacritty_terminal::{Term, grid::Dimensions, term::TermMode, vte::ansi::Processor};
use rshell_core::ResolvedTerminalProfile;

use crate::{alacritty_event::EventSink, alacritty_rows};

struct FeedWindow<'a> {
    bytes: &'a [u8],
    maximum_shift: usize,
    track_capacity: bool,
}

pub(crate) fn advance(
    processor: &mut Processor,
    terminal: &mut Term<EventSink>,
    events: &EventSink,
    settings: &ResolvedTerminalProfile,
    primary_origin: &mut i64,
    tracker: &mut alacritty_rows::ScrollTracker,
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
            tracker,
            &remaining[..sequence.start],
        );
        advance_segment(
            processor,
            terminal,
            events,
            settings,
            primary_origin,
            tracker,
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
        tracker,
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
    tracker: &mut alacritty_rows::ScrollTracker,
    bytes: &[u8],
) {
    if bytes.is_empty() {
        return;
    }
    let mut remaining = bytes;
    while let Some(index) = remaining.iter().position(|byte| *byte == 0x05) {
        advance_windows(
            processor,
            terminal,
            settings,
            primary_origin,
            tracker,
            &remaining[..=index],
        );
        events.push_outbound(settings.answerback.as_bytes());
        remaining = &remaining[index + 1..];
    }
    advance_windows(
        processor,
        terminal,
        settings,
        primary_origin,
        tracker,
        remaining,
    );
}

fn advance_windows(
    processor: &mut Processor,
    terminal: &mut Term<EventSink>,
    settings: &ResolvedTerminalProfile,
    primary_origin: &mut i64,
    tracker: &mut alacritty_rows::ScrollTracker,
    mut remaining: &[u8],
) {
    while !remaining.is_empty() {
        let was_primary = !terminal.mode().contains(TermMode::ALT_SCREEN);
        let (length, maximum_shift, track_capacity) = if was_primary {
            match alacritty_rows::bounded_prefix(
                tracker,
                remaining,
                terminal,
                settings.scrollback_lines,
            ) {
                Some(prefix) => (prefix.length, prefix.maximum_shift, true),
                None => (remaining.len(), 0, false),
            }
        } else {
            (remaining.len(), 0, false)
        };
        advance_window(
            processor,
            terminal,
            settings,
            primary_origin,
            tracker,
            FeedWindow {
                bytes: &remaining[..length],
                maximum_shift,
                track_capacity,
            },
        );
        remaining = &remaining[length..];
    }
}

fn advance_window(
    processor: &mut Processor,
    terminal: &mut Term<EventSink>,
    settings: &ResolvedTerminalProfile,
    primary_origin: &mut i64,
    tracker: &mut alacritty_rows::ScrollTracker,
    window: FeedWindow<'_>,
) {
    let FeedWindow {
        bytes,
        maximum_shift,
        track_capacity,
    } = window;
    let was_primary = !terminal.mode().contains(TermMode::ALT_SCREEN);
    let old_history = terminal.grid().history_size();
    let screen_lines = terminal.grid().screen_lines();
    let anchor_shift = if track_capacity { maximum_shift } else { 0 };
    let capacity_anchor = was_primary
        .then(|| alacritty_rows::capture(terminal, anchor_shift))
        .flatten();

    processor.advance(terminal, bytes);
    if !track_capacity {
        tracker.consume(bytes, screen_lines);
    }

    if !terminal.mode().contains(TermMode::ALT_SCREEN) {
        let history = terminal.grid().history_size();
        if was_primary {
            let completed = alacritty_rows::completed_shift(
                terminal,
                settings.scrollback_lines,
                capacity_anchor,
            );
            let shift = history
                .saturating_sub(old_history)
                .saturating_add(completed);
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
