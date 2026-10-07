//! Finite post-verdict queries; these can refresh GTK caches, not prove prior cache state.
use crate::fluent_native::{FailurePhase, FrameFailure};
use std::{ffi::OsStr, fmt::Write};

const CHILD_CAP: usize = 8;

#[derive(Clone, Copy)]
struct Facts {
    allocation: [i32; 4],
    content: [i32; 2],
    relative_outer: Option<[f32; 4]>,
    visible: bool,
    mapped: bool,
    request_mode: u8, // 0 unknown, 1 constant, 2 height-for-width, 3 width-for-height.
}

#[derive(Default)]
struct Captured {
    version: [u32; 3],
    viewport: Option<Facts>,
    body: Option<Facts>,
    children: [Option<Facts>; CHILD_CAP],
    children_capped: bool,
    policies_hv: Option<[u8; 2]>, // 0 unknown, 1 minimum, 2 natural.
    adjustment_luvp: Option<[f64; 4]>,
}

trait Provider {
    fn capture(&mut self) -> Captured;
    fn vertical_measure(&mut self, node: usize, width: i32) -> Option<[i32; 4]>;
}

// None represents success; real callers get Some only from the existing failure FnOnce.
fn report_if<P: Provider>(
    windows: bool,
    opt_in: Option<&OsStr>,
    mode: u8,
    case: u8,
    failure: Option<FrameFailure>,
    provider: impl FnOnce() -> P,
) -> Option<String> {
    if !windows
        || opt_in != Some(OsStr::new("1"))
        || mode != 1
        || case != 5
        || !failure.is_some_and(|f| f.phase == FailurePhase::AfterPaint)
    {
        return None;
    }
    Some(collect(&mut provider()))
}

fn collect(provider: &mut impl Provider) -> String {
    // Finish every allocation/request-mode/adjustment query before ANY explicit measure.
    let captured = provider.capture();
    let body_width = captured.body.map(|f| f.content[0]);
    let body_at_width = body_width.and_then(|w| provider.vertical_measure(1, w));
    let body_unrestricted = captured.body.and_then(|_| provider.vertical_measure(1, -1));
    let children_measured: [Option<[i32; 4]>; CHILD_CAP] = std::array::from_fn(|i| {
        captured.children[i]
            .filter(|f| f.content[0] > 0)
            .and_then(|f| provider.vertical_measure(i + 2, f.content[0]))
    });
    let mut output = format!(
        "NATIVE_MODAL_MEASUREMENT post_verdict=1 cache_affecting=1 pre_measure_capture=1 historical_cache_proof=0 gtk={:?} child_cap=8 children_capped={} request_mode_codes=0/1/2/3 policy_codes=0/1/2\n",
        captured.version, captured.children_capped
    );
    // Nodes: 0 viewport (relative to root), 1 body (viewport), 2..9 children (body).
    facts_line(&mut output, 0, 0, captured.viewport);
    let _ = writeln!(
        output,
        " policies_hv={} adjustment_luvp={}",
        numeric(captured.policies_hv),
        numeric(captured.adjustment_luvp)
    );
    facts_line(&mut output, 1, 1, captured.body);
    let _ = writeln!(
        output,
        " vertical_for_size={} min_nat_baselines={} vertical_for_size=-1 min_nat_baselines={}",
        numeric(body_width),
        numeric(body_at_width),
        numeric(body_unrestricted)
    );
    for (i, measured) in children_measured.into_iter().enumerate() {
        facts_line(&mut output, i + 2, 2, captured.children[i]);
        let width = captured.children[i].map(|f| f.content[0]);
        let _ = writeln!(
            output,
            " vertical_for_size={} min_nat_baselines={}",
            numeric(width),
            numeric(measured)
        );
    }
    output
}

fn numeric<T: std::fmt::Debug>(value: Option<T>) -> String {
    value.map_or_else(|| "unavailable".into(), |v| format!("{v:?}"))
}

fn facts_line(output: &mut String, node: usize, relative: u8, facts: Option<Facts>) {
    let _ = write!(output, "node={node} relative={relative}");
    if let Some(f) = facts {
        let _ = write!(
            output,
            " allocation={:?} content={:?} outer={} visible={} mapped={} request_mode={}",
            f.allocation,
            f.content,
            numeric(f.relative_outer),
            f.visible,
            f.mapped,
            f.request_mode
        );
    } else {
        let _ = write!(output, " unavailable");
    }
}

#[cfg(windows)]
#[path = "fluent_interaction_measurement_gtk.rs"]
mod gtk_provider;

#[cfg(windows)]
pub(super) fn report(root: &relm4::gtk::Widget, mode: u8, case: u8, failure: FrameFailure) {
    let opt_in = std::env::var_os("RSHELL_NATIVE_MODAL_DIAGNOSTICS");
    if let Some(output) = report_if(true, opt_in.as_deref(), mode, case, Some(failure), || {
        gtk_provider::GtkProvider::new(root)
    }) {
        eprint!("{output}");
    }
}

#[cfg(test)]
#[path = "fluent_interaction_measurement_tests.rs"]
mod tests;
