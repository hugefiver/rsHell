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
    let deadline = Instant::now() + Duration::from_secs(2);
    let widget = widget.as_ref();
    let clock_ready = iterate_until(deadline, || widget.frame_clock().is_some());
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
    #[cfg(windows)]
    windows_geometry::report_if_failed(
        completed,
        windows_geometry::FailurePhase::AfterPaint,
        widget,
    );
    let observed = observation.get();
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
