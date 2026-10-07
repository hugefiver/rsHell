//! Paint-bound, read-only first-open evidence; no GTK handlers or retained widgets.
use super::{FrameFailure, measurements, wait_for_frame_with_failure_report};
use relm4::gtk;
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct State {
    pub modal: Option<(bool, i32, i32)>,
    pub first_kind: u8,
    pub first: Option<(bool, bool, i32, i32)>,
    pub focus_kind: u8,
    pub focus: Option<(bool, bool, i32, i32)>,
    pub focus_relation: u8,
    pub focus_in_modal: bool,
    pub same_root: Option<bool>,
    pub active: Option<bool>,
    pub modal_ready: bool,
    pub outer_ready: bool,
    pub input: Option<[f32; 4]>,
    pub viewport: Option<[f32; 4]>,
    pub overflow_ltrb: Option<[f32; 4]>,
    pub viewport_allocation: Option<(i32, i32)>,
    pub scroll_to_focus: Option<bool>,
    pub adjustment_luvp: Option<[f64; 4]>,
}

struct History {
    initial: State,
    last: State,
    changes: Vec<(u128, State)>,
    samples: usize,
    transitions: usize,
}

impl History {
    fn new(initial: State) -> Self {
        Self {
            last: initial.clone(),
            initial,
            changes: Vec::new(),
            samples: 0,
            transitions: 0,
        }
    }

    fn sample(&mut self, elapsed_ms: u128, state: State) {
        self.samples += 1;
        if self.last != state {
            self.transitions += 1;
            if self.changes.len() < 32 {
                self.changes.push((elapsed_ms, state.clone()));
            }
            self.last = state;
        }
    }

    fn failure_lines(
        &self,
        mode: u8,
        case: u8,
        failure: FrameFailure,
        final_sample: (u128, State),
    ) -> String {
        use std::fmt::Write;
        let (final_ms, final_state) = final_sample;
        let mut output = format!(
            "NATIVE_MODAL_FAILURE mode={mode} case={case} phase={} paints={} predicate={:?} elapsed_ms={} budget_ms={} tolerance_px=2 samples={} transitions={} retained={} dropped={}\ninitial t_ms=0 {:?}\n",
            failure.phase as u8,
            failure.paints,
            failure.predicate,
            failure.elapsed_ms,
            failure.budget_ms,
            self.samples,
            self.transitions,
            self.changes.len(),
            self.transitions - self.changes.len(),
            self.initial,
        );
        for (elapsed, state) in &self.changes {
            writeln!(output, "change t_ms={elapsed} {state:?}").unwrap();
        }
        writeln!(output, "final t_ms={final_ms} {final_state:?}").unwrap();
        output
    }
}

#[derive(Clone)]
struct Observation {
    mode: u8,
    case: u8,
    started: Instant,
    history: Rc<RefCell<History>>,
}

impl Observation {
    pub fn new(mode: &str, case: &str, root: &gtk::Widget) -> Self {
        Self {
            mode: fixed_mode(mode),
            case: fixed_case(case),
            started: Instant::now(),
            history: Rc::new(RefCell::new(History::new(measurements::observe(root)))),
        }
    }

    pub fn sample(&self, root: &gtk::Widget) {
        // GTK reads complete before borrowing; no borrow is held across callbacks.
        let state = measurements::observe(root);
        self.history
            .borrow_mut()
            .sample(self.started.elapsed().as_millis(), state);
    }

    pub fn report(&self, root: &gtk::Widget, failure: FrameFailure) {
        let final_state = measurements::observe(root);
        let final_sample = (self.started.elapsed().as_millis(), final_state);
        let output =
            self.history
                .borrow()
                .failure_lines(self.mode, self.case, failure, final_sample);
        eprint!("{output}");
    }
}

pub(super) fn wait(
    root: &gtk::Widget,
    mode: &str,
    case: &str,
    ready: impl Fn(&gtk::Widget) -> bool + 'static,
) {
    let observation = Observation::new(mode, case, root);
    let paints = observation.clone();
    wait_for_frame_with_failure_report(
        root,
        "secure modal first focus",
        move |root| {
            paints.sample(root);
            ready(root)
        },
        |failure| observation.report(root, failure),
    );
}

fn fixed_mode(mode: &str) -> u8 {
    match mode {
        "compact" => 1,
        "standard" => 2,
        "wide" => 3,
        _ => 0,
    }
}

fn fixed_case(case: &str) -> u8 {
    match case {
        "host-unknown-reject" => 1,
        "host-unknown-accept" => 2,
        "host-changed-close" => 3,
        "host-unknown-escape" => 4,
        "password" => 5,
        "passphrase" => 6,
        "keyboard" => 7,
        "auth-cancel" => 8,
        "auth-escape-adversarial" => 9,
        _ => 0,
    }
}

#[cfg(test)]
#[path = "fluent_interaction_diagnostics_tests.rs"]
mod tests;
