//! Fixed safe post-verdict diagnostics; unavailable geometry is not an assertion.
use super::native;
use gtk::prelude::*;
use std::io::Write;

fn unavailable(node: u8) {
    let _ = writeln!(
        std::io::stderr(),
        "PROMPT_CACHE_NATIVE phase=2 state_available=false node={node}"
    );
}

pub(super) fn report(root: &gtk::Widget) {
    let Some(modal) = native::descendants(root)
        .into_iter()
        .find(|w| w.has_css_class("interaction-dialog"))
    else {
        unavailable(1);
        return;
    };
    let Some(input) = native::descendants(&modal)
        .into_iter()
        .find(|w| w.has_css_class("modal-focus-first") && w.is::<gtk::PasswordEntry>())
    else {
        unavailable(2);
        return;
    };
    let Some(prompts) = input.parent() else {
        unavailable(3);
        return;
    };
    let Some(label) = prompts
        .first_child()
        .and_then(|w| w.downcast::<gtk::Label>().ok())
    else {
        unavailable(4);
        return;
    };
    let Some(label_bounds) = label.compute_bounds(&prompts) else {
        unavailable(5);
        return;
    };
    let Some(entry_bounds) = input.compute_bounds(&prompts) else {
        unavailable(6);
        return;
    };
    let Some(viewport) = input
        .ancestor(gtk::Viewport::static_type())
        .and_then(|w| w.downcast::<gtk::Viewport>().ok())
    else {
        unavailable(7);
        return;
    };
    let Some(body) = viewport.child() else {
        unavailable(8);
        return;
    };
    let Some(a) = viewport.vadjustment() else {
        unavailable(9);
        return;
    };
    let Some(v) = viewport.compute_bounds(root) else {
        unavailable(10);
        return;
    };
    let Some(b) = input.compute_bounds(root) else {
        unavailable(11);
        return;
    };
    let outer_ready = b.x() >= v.x() - 2.0
        && b.y() >= v.y() - 2.0
        && b.x() + b.width() <= v.x() + v.width() + 2.0
        && b.y() + b.height() <= v.y() + v.height() + 2.0;
    // Capture modes and allocations before any explicit post-verdict measurement.
    let mode = prompts.request_mode();
    let leaf_mode = label.request_mode();
    let _ = writeln!(
        std::io::stderr(),
        "PROMPT_CACHE_NATIVE phase=2 prompts_mode={mode:?} leaf_mode={leaf_mode:?} leaf_wrap={} prompts_allocation={}x{} label_extent_yh=[{},{}] entry_extent_yh=[{},{}] body_allocation={}x{} adjustment={:?} focus={} outer_ready={outer_ready}",
        label.wraps(),
        prompts.width(),
        prompts.height(),
        label_bounds.y(),
        label_bounds.height(),
        entry_bounds.y(),
        entry_bounds.height(),
        body.width(),
        body.height(),
        [a.lower(), a.upper(), a.value(), a.page_size()],
        native::focus_within(&input)
    );
    let box_height = prompts.measure(gtk::Orientation::Vertical, prompts.width());
    let body_height = body.measure(gtk::Orientation::Vertical, body.width());
    let leaf_height = label.measure(gtk::Orientation::Vertical, prompts.width());
    let _ = writeln!(
        std::io::stderr(),
        "PROMPT_CACHE_NATIVE post_verdict_measure box={box_height:?} body={body_height:?} leaf={leaf_height:?}"
    );
}
