//! Opt-in numeric evidence from existing reveal callbacks; never schedules work.
use relm4::gtk::{self, prelude::*};
use std::{
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};

#[derive(Clone, Copy)]
pub(super) enum Phase {
    Scheduled = 0,
    AfterPaint = 1,
    Unavailable = 2,
    NotFocused = 3,
    NoScroller = 4,
    NoViewport = 5,
    NoBounds = 6,
    BeforeClamp = 7,
    AfterClamp = 8,
}

static ENABLED: OnceLock<bool> = OnceLock::new();
static STARTED: OnceLock<Instant> = OnceLock::new();
static EVENTS: AtomicUsize = AtomicUsize::new(0);
static CALLBACKS: AtomicUsize = AtomicUsize::new(0);

fn enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

fn event_number(counter: &AtomicUsize) -> Option<usize> {
    let mut n = counter.load(Ordering::Relaxed);
    while n < 64 {
        match counter.compare_exchange_weak(n, n + 1, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(n + 1),
            Err(current) => n = current,
        }
    }
    None
}

pub(super) fn begin(input: &gtk::Widget) -> Option<usize> {
    if !*ENABLED
        .get_or_init(|| enabled(std::env::var_os("RSHELL_NATIVE_MODAL_DIAGNOSTICS").as_deref()))
    {
        return None;
    }
    let callback = CALLBACKS.fetch_add(1, Ordering::Relaxed) + 1;
    record(Some(callback), Phase::Scheduled, Some(input));
    Some(callback)
}

#[derive(Debug, Default)]
struct State {
    kind: u8,
    mapped: bool,
    sensitive: bool,
    allocation: (i32, i32),
    focus_relation: u8,
    same_root: Option<bool>,
    active: Option<bool>,
    outer: Option<[f32; 4]>,
    viewport: Option<[f32; 4]>,
    adjustment_luvp: Option<[f64; 4]>,
}

fn state(input: Option<&gtk::Widget>) -> State {
    let Some(input) = input else {
        return State::default();
    };
    let root = input.root();
    let focused = root.as_ref().and_then(gtk::prelude::RootExt::focus);
    let mut state = State {
        kind: if input.is::<gtk::PasswordEntry>() {
            1
        } else if input.is::<gtk::Entry>() {
            2
        } else if input.is::<gtk::Button>() {
            3
        } else {
            5
        },
        mapped: input.is_mapped(),
        sensitive: input.is_sensitive(),
        allocation: (input.width(), input.height()),
        focus_relation: focused.as_ref().map_or(0, |w| {
            if w == input {
                1
            } else if w.is_ancestor(input) {
                2
            } else {
                3
            }
        }),
        same_root: focused.as_ref().map(|w| w.root() == root),
        active: root
            .as_ref()
            .and_then(|r| r.downcast_ref::<gtk::Window>())
            .map(|w| w.is_active()),
        ..Default::default()
    };
    if let Some(root) = root {
        let bounds = |w: &gtk::Widget| {
            w.compute_bounds(&root)
                .map(|b| [b.x(), b.y(), b.width(), b.height()])
        };
        state.outer = bounds(input);
        state.viewport = input
            .ancestor(gtk::Viewport::static_type())
            .and_then(|w| bounds(&w));
    }
    if let Some(scroll) = input
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
    {
        let a = scroll.vadjustment();
        state.adjustment_luvp = Some([a.lower(), a.upper(), a.value(), a.page_size()]);
    }
    state
}

fn line(event: usize, callback: usize, phase: Phase, elapsed_ms: u128, state: State) -> String {
    format!(
        "NATIVE_MODAL_REVEAL event={event} callback={callback} phase={} elapsed_ms={elapsed_ms} kind={} mapped={} sensitive={} allocation={:?} focus_relation={} same_root={:?} active={:?} outer={:?} viewport={:?} adjustment_luvp={:?}",
        phase as u8,
        state.kind,
        state.mapped,
        state.sensitive,
        state.allocation,
        state.focus_relation,
        state.same_root,
        state.active,
        state.outer,
        state.viewport,
        state.adjustment_luvp,
    )
}

pub(super) fn record(callback: Option<usize>, phase: Phase, input: Option<&gtk::Widget>) {
    let Some(callback) = callback else {
        return;
    };
    let Some(event) = event_number(&EVENTS) else {
        return;
    };
    let elapsed = STARTED.get_or_init(Instant::now).elapsed().as_millis();
    eprintln!("{}", line(event, callback, phase, elapsed, state(input)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_one_enables_and_budget_is_finite() {
        for value in [None, Some(""), Some("0"), Some("true"), Some("01")] {
            assert!(!enabled(value.map(std::ffi::OsStr::new)));
        }
        assert!(enabled(Some(std::ffi::OsStr::new("1"))));
        let counter = AtomicUsize::new(0);
        for n in 1..=64 {
            assert_eq!(event_number(&counter), Some(n));
        }
        for _ in 0..100 {
            assert_eq!(event_number(&counter), None);
        }
        assert_eq!(counter.load(Ordering::Relaxed), 64);
    }

    #[test]
    fn formatter_contains_only_fixed_fields_and_numeric_phases() {
        for (phase, code) in [
            (Phase::Scheduled, 0),
            (Phase::AfterPaint, 1),
            (Phase::Unavailable, 2),
            (Phase::NotFocused, 3),
            (Phase::NoScroller, 4),
            (Phase::NoViewport, 5),
            (Phase::NoBounds, 6),
            (Phase::BeforeClamp, 7),
            (Phase::AfterClamp, 8),
        ] {
            let output = line(1, 2, phase, 3, State::default());
            assert!(output.starts_with(&format!(
                "NATIVE_MODAL_REVEAL event=1 callback=2 phase={code} elapsed_ms=3 kind=0"
            )));
            assert!(output.ends_with("adjustment_luvp=None"));
            assert!(!output.contains("0x"));
        }
        record(None, Phase::Scheduled, None); // disabled routing never consumes the process budget
        assert_eq!(EVENTS.load(Ordering::Relaxed), 0);
    }
}
