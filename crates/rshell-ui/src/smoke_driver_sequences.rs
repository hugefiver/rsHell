use crate::SmokeCounters;

pub(crate) fn resize_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .resize
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn search_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .search
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn selection_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .selection
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn clipboard_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .clipboard
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn reconnect_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .reconnect
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn paste_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .paste
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn color_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .terminal
        .color
        .map_or(0, |evidence| evidence.sequence)
}

pub(crate) fn import_preview_sequence(counters: &SmokeCounters) -> u64 {
    counters
        .imports
        .preview
        .as_ref()
        .map_or(0, |evidence| evidence.sequence)
}
