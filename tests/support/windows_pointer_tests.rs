use super::*;

fn event(kind: EventKind, x: f64) -> PointerEvent {
    PointerEvent {
        kind,
        button: 1,
        x,
        y: 10.0,
    }
}

#[test]
fn paint_before_native_drag_motion_does_not_allow_release() {
    let mut trace = PointerTrace::default();
    let down = gtk::gdk::ModifierType::BUTTON1_MASK;
    trace.record_motion(130.0, 10.0, down);
    assert!(
        !trace.saw_drag_movement(),
        "motion before press is not a drag"
    );
    trace.record(event(EventKind::Press, 30.0));
    assert!(
        !trace.saw_drag_movement(),
        "a painted frame after press must not release before native motion"
    );
    trace.record_motion(130.0, 10.0, gtk::gdk::ModifierType::empty());
    assert!(!trace.saw_drag_movement(), "button must still be held");
    trace.record_motion(30.0, 10.0, down);
    assert!(!trace.saw_drag_movement(), "must reach the drag target");
    trace.record_motion(131.01, 10.0, down);
    assert!(!trace.saw_drag_movement(), "keep the strict ±1 tolerance");
    trace.record_motion(130.0, 11.01, down);
    assert!(!trace.saw_drag_movement());
    trace.record_motion(131.0, 11.0, down);
    assert!(trace.saw_drag_movement());

    let mut late = PointerTrace::default();
    late.record(event(EventKind::Press, 30.0));
    late.record(event(EventKind::Release, 130.0));
    late.record_motion(130.0, 10.0, down);
    assert!(
        !late.saw_drag_movement(),
        "motion after release is too late"
    );
}

#[test]
fn strict_observations_fail_after_cleanup_without_native_input() {
    let press = event(EventKind::Press, 30.0);
    let release = event(EventKind::Release, 130.0);
    let trace = |first, second| PointerTrace {
        events: [Some(first), Some(second)],
        count: 2,
        ..Default::default()
    };
    assert!(validate_trace(trace(PointerEvent { x: 31.0, ..press }, release)).is_ok());
    for invalid in [
        trace(PointerEvent { x: 31.01, ..press }, release),
        trace(PointerEvent { button: 2, ..press }, release),
        trace(release, press),
        PointerTrace {
            count: 1,
            ..trace(press, release)
        },
    ] {
        assert!(validate_trace(invalid).is_err(), "{invalid:?}");
    }
    let mut cleaned = false;
    let mark_clean = |done: &mut bool| {
        *done = true;
        true
    };
    let failed_coordinate = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let bad = cleanup_before_report(
            &mut cleaned,
            |_| trace(PointerEvent { x: 31.01, ..press }, release),
            mark_clean,
        );
        assert!(validate_trace(bad).is_ok(), "{bad:?}");
    }));
    assert!(failed_coordinate.is_err() && cleaned);
    cleaned = false;
    let failure = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        cleanup_before_report(
            &mut cleaned,
            |_| panic!("after-paint wait failed"),
            mark_clean,
        );
    }));
    assert!(failure.is_err() && cleaned);
}
