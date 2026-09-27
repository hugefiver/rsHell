use super::*;

#[test]
fn window_resize_waits_for_a_positive_real_allocation() {
    let mut evidence = Some(SmokeWindowResizeEvidence {
        sequence: 1,
        requested_width: 800,
        requested_height: 600,
        realized_width: 0,
        realized_height: 0,
        expected_layout: ShellLayoutMode::Compact,
        layout: ShellLayoutMode::Compact,
    });
    update_window_allocation(&mut evidence, 0, 0, ShellLayoutMode::Compact);
    assert_eq!(evidence.as_ref().unwrap().realized_width, 0);
    update_window_allocation(&mut evidence, 798, 598, ShellLayoutMode::Compact);
    let evidence = evidence.unwrap();
    assert_eq!(
        (evidence.realized_width, evidence.realized_height),
        (798, 598)
    );
}

#[test]
fn window_resize_records_the_realized_mode_instead_of_the_requested_mode() {
    let mut evidence = Some(SmokeWindowResizeEvidence {
        sequence: 1,
        requested_width: 1_920,
        requested_height: 1_080,
        realized_width: 0,
        realized_height: 0,
        expected_layout: ShellLayoutMode::Wide,
        layout: ShellLayoutMode::Wide,
    });
    update_window_allocation(&mut evidence, 1_358, 811, ShellLayoutMode::Standard);
    let evidence = evidence.unwrap();
    assert_eq!(
        (evidence.realized_width, evidence.realized_height),
        (1_358, 811)
    );
    assert_eq!(evidence.layout, ShellLayoutMode::Standard);
}
