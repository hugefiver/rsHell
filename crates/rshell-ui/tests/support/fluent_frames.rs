//! Shared bounded GTK frame pump; no dependency on a particular test binary.
use relm4::gtk::{self, prelude::*};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

#[cfg(windows)]
#[path = "windows_geometry.rs"]
mod windows_geometry;

#[derive(Clone, Copy, Default)]
struct FrameObservation {
    after_paint_callbacks: usize,
    last_mapped: Option<bool>,
    last_allocation: Option<(i32, i32)>,
    last_predicate: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailurePhase {
    FrameClock = 1,
    AfterPaint = 2,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameFailure {
    pub phase: FailurePhase,
    pub paints: usize,
    pub predicate: Option<bool>,
    pub elapsed_ms: u128,
    pub budget_ms: u128,
}

fn report_failure(completed: bool, failure: FrameFailure, report: impl FnOnce(FrameFailure)) {
    if !completed {
        report(failure);
    }
}

pub(crate) fn iterate_until(deadline: Instant, mut ready: impl FnMut() -> bool) -> bool {
    let context = gtk::glib::MainContext::default();
    loop {
        if Instant::now() >= deadline {
            return false;
        }
        let completed = ready();
        if Instant::now() >= deadline {
            return false;
        }
        if completed {
            return true;
        }
        // One nonblocking dispatch: continuously-ready sources cannot defeat the deadline.
        if !context.iteration(false) {
            std::thread::yield_now();
        }
    }
}

pub(crate) fn wait_for_frame(
    widget: &impl IsA<gtk::Widget>,
    description: &str,
    ready: impl Fn(&gtk::Widget) -> bool + 'static,
) {
    wait_for_frame_with_failure_report(widget, description, ready, |_| {});
}

pub(crate) fn wait_for_frame_with_failure_report(
    widget: &impl IsA<gtk::Widget>,
    description: &str,
    ready: impl Fn(&gtk::Widget) -> bool + 'static,
    report: impl FnOnce(FrameFailure),
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let started = deadline - Duration::from_secs(2);
    let mut report = Some(report);
    let widget = widget.as_ref();
    let clock_ready = iterate_until(deadline, || widget.frame_clock().is_some());
    if !clock_ready {
        report_failure(
            clock_ready,
            FrameFailure {
                phase: FailurePhase::FrameClock,
                paints: 0,
                predicate: None,
                elapsed_ms: started.elapsed().as_millis(),
                budget_ms: 2_000,
            },
            report.take().unwrap(),
        );
    }
    #[cfg(windows)]
    windows_geometry::report_if_failed(
        clock_ready,
        windows_geometry::FailurePhase::FrameClock,
        widget,
    );
    assert!(
        clock_ready,
        "{description}: frame clock unavailable before deadline"
    );
    let clock = widget.frame_clock().unwrap();
    let painted = Rc::new(Cell::new(false));
    let signal_painted = painted.clone();
    let observation = Rc::new(Cell::new(FrameObservation::default()));
    let signal_observation = observation.clone();
    let target = widget.clone();
    let signal = clock.connect_after_paint(move |_| {
        let mapped = target.is_mapped();
        let width = target.width();
        let height = target.height();
        let predicate = (mapped && width > 0 && height > 0).then(|| ready(&target));
        signal_observation.set(FrameObservation {
            after_paint_callbacks: signal_observation.get().after_paint_callbacks + 1,
            last_mapped: Some(mapped),
            last_allocation: Some((width, height)),
            last_predicate: predicate,
        });
        if predicate == Some(true) {
            signal_painted.set(true);
        }
    });
    widget.queue_draw();
    clock.request_phase(gtk::gdk::FrameClockPhase::PAINT | gtk::gdk::FrameClockPhase::AFTER_PAINT);
    let completed = iterate_until(deadline, || painted.get());
    clock.disconnect(signal);
    let observed = observation.get();
    report_failure(
        completed,
        FrameFailure {
            phase: FailurePhase::AfterPaint,
            paints: observed.after_paint_callbacks,
            predicate: observed.last_predicate,
            elapsed_ms: started.elapsed().as_millis(),
            budget_ms: 2_000,
        },
        report.unwrap(),
    );
    #[cfg(windows)]
    windows_geometry::report_if_failed(
        completed,
        windows_geometry::FailurePhase::AfterPaint,
        widget,
    );
    assert!(
        completed,
        "{description}: ready frame missed deadline; after_paint_callbacks={} last_mapped={:?} last_allocation={:?} last_predicate={:?} (None = not evaluated); current_mapped={} current_allocation={}x{}",
        observed.after_paint_callbacks,
        observed.last_mapped,
        observed.last_allocation,
        observed.last_predicate,
        widget.is_mapped(),
        widget.width(),
        widget.height()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_routing_is_once_per_phase_and_success_is_quiet() {
        for phase in [FailurePhase::FrameClock, FailurePhase::AfterPaint] {
            let failure = FrameFailure {
                phase,
                paints: 127,
                predicate: Some(true),
                elapsed_ms: 2_001,
                budget_ms: 2_000,
            };
            let calls = Cell::new(0);
            report_failure(true, failure, |_| calls.set(calls.get() + 1));
            assert_eq!(calls.get(), 0);
            report_failure(false, failure, |observed| {
                calls.set(calls.get() + 1);
                assert_eq!(observed.phase, phase);
                assert_eq!(observed.paints, 127);
                assert_eq!(observed.predicate, Some(true)); // true after deadline remains failure
                assert_eq!(observed.elapsed_ms, 2_001);
                assert_eq!(observed.budget_ms, 2_000);
            });
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn shared_deadline_checks_and_disconnect_before_report_are_preserved() {
        let source = include_str!("fluent_frames.rs");
        let pump = source
            .split("pub(crate) fn iterate_until")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn wait_for_frame")
            .next()
            .unwrap();
        assert_eq!(pump.matches("Instant::now() >= deadline").count(), 2);
        assert!(
            pump.find("let completed = ready();").unwrap()
                < pump.rfind("Instant::now() >= deadline").unwrap()
        );
        let wait = source
            .split("pub(crate) fn wait_for_frame_with_failure_report")
            .nth(1)
            .unwrap()
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        assert_eq!(
            wait.matches("let deadline = Instant::now() + Duration::from_secs(2);")
                .count(),
            1
        );
        assert!(wait.contains("iterate_until(deadline, || widget.frame_clock().is_some())"));
        assert!(wait.contains("iterate_until(deadline, || painted.get())"));
        assert!(
            wait.find("clock.disconnect(signal);").unwrap()
                < wait.rfind("report_failure(").unwrap()
        );
        assert!(wait.contains("(mapped && width > 0 && height > 0).then(|| ready(&target))"));
    }
}
