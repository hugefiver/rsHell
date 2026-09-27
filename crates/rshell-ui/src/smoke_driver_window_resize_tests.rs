use super::*;
use crate::{ShellLayoutMode, SmokeWindowResizeEvidence};

#[test]
fn window_resize_waits_for_fresh_matching_allocation_and_mode() {
    let action = SmokeAction::ResizeWindow {
        width: 800,
        height: 600,
        expected_mode: ShellLayoutMode::Compact,
    };
    let previous = SmokeWindowResizeEvidence {
        sequence: 7,
        requested_width: 800,
        requested_height: 600,
        realized_width: 800,
        realized_height: 600,
        expected_layout: ShellLayoutMode::Compact,
        layout: ShellLayoutMode::Compact,
    };
    let before = SmokeCounters {
        window_resize: Some(previous),
        ..Default::default()
    };
    assert!(!complete(&action, &before, &observation(before.clone())));

    let mut fresh = before.clone();
    let evidence = fresh.window_resize.as_mut().expect("resize evidence");
    evidence.sequence = 8;
    evidence.realized_width = 1_920;
    evidence.realized_height = 1_152;
    evidence.layout = ShellLayoutMode::Wide;
    assert!(!complete(&action, &before, &observation(fresh.clone())));

    fresh.window_resize.as_mut().unwrap().layout = ShellLayoutMode::Compact;
    assert!(
        !complete(&action, &before, &observation(fresh.clone())),
        "a fresh, positive allocation in the requested mode is still the wrong size"
    );

    let evidence = fresh.window_resize.as_mut().unwrap();
    evidence.realized_width = 800;
    evidence.realized_height = 600;
    evidence.expected_layout = ShellLayoutMode::Wide;
    assert!(!complete(&action, &before, &observation(fresh.clone())));
    fresh.window_resize.as_mut().unwrap().expected_layout = ShellLayoutMode::Compact;
    assert!(complete(&action, &before, &observation(fresh)));
}
