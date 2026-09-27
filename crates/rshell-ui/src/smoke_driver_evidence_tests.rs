use super::*;

#[test]
fn widget_allocation_tolerance_is_per_dimension_and_platform_specific() {
    let mut evidence = SmokeWindowResizeEvidence {
        sequence: 8,
        requested_width: 800,
        requested_height: 600,
        realized_width: 798,
        realized_height: 598,
        expected_layout: ShellLayoutMode::Compact,
        layout: ShellLayoutMode::Compact,
    };
    assert!(window_resize_matches_with_tolerance(evidence, 2));
    assert!(!window_resize_matches_with_tolerance(evidence, 0));
    assert_eq!(window_resize_matches(evidence), cfg!(windows));
    evidence.realized_width = 802;
    evidence.realized_height = 602;
    assert!(window_resize_matches_with_tolerance(evidence, 2));
    assert!(!window_resize_matches_with_tolerance(evidence, 0));
    evidence.realized_width = 803;
    assert!(!window_resize_matches_with_tolerance(evidence, 2));
    evidence.realized_width = 800;
    evidence.realized_height = 597;
    assert!(!window_resize_matches_with_tolerance(evidence, 2));
    evidence.realized_width = 1_920;
    evidence.realized_height = 1_152;
    assert!(!window_resize_matches_with_tolerance(evidence, 2));
    assert!(window_resize_pending(Some(evidence)));
    evidence.realized_width = 800;
    evidence.realized_height = 600;
    evidence.layout = ShellLayoutMode::Wide;
    assert!(!window_resize_matches_with_tolerance(evidence, 2));
    evidence.layout = ShellLayoutMode::Compact;
    assert!(window_resize_matches_with_tolerance(evidence, 0));
    assert!(window_resize_matches(evidence));
    assert!(!window_resize_pending(Some(evidence)));
    assert!(!window_resize_pending(None));
}
