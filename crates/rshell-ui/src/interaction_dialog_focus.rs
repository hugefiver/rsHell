//! Reconcile pre-allocation initial focus once, using the allocated outer control.
use gtk::prelude::*;
use relm4::gtk;
use std::{cell::RefCell, rc::Rc};

#[path = "interaction_dialog_focus_diagnostics.rs"]
mod diagnostics;
use diagnostics::Phase;

pub(super) fn reveal_after_paint(input: &gtk::Widget, clock: &gtk::gdk::FrameClock) {
    let trace = diagnostics::begin(input);
    let input = input.downgrade();
    let handler = Rc::new(RefCell::new(None));
    let completed = handler.clone();
    *handler.borrow_mut() = Some(clock.connect_after_paint(move |clock| {
        // Disconnect even if the interaction closed or focus moved before paint.
        let handler = completed.borrow_mut().take();
        if let Some(handler) = handler {
            clock.disconnect(handler);
        }
        if trace.is_some() {
            diagnostics::record(trace, Phase::AfterPaint, input.upgrade().as_ref());
        }
        let Some(input) = input
            .upgrade()
            .filter(|w| w.is_mapped() && w.is_sensitive())
        else {
            if trace.is_some() {
                diagnostics::record(trace, Phase::Unavailable, input.upgrade().as_ref());
            }
            return;
        };
        let focused = input
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root));
        if !focused.is_some_and(|w| w == input || w.is_ancestor(&input)) {
            diagnostics::record(trace, Phase::NotFocused, Some(&input));
            return;
        }
        let Some(scroll) = input
            .ancestor(gtk::ScrolledWindow::static_type())
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        else {
            diagnostics::record(trace, Phase::NoScroller, Some(&input));
            return; // Host-key actions are in the fixed footer, not the body.
        };
        let Some(viewport) = scroll.child() else {
            diagnostics::record(trace, Phase::NoViewport, Some(&input));
            return;
        };
        let Some(bounds) = input.compute_bounds(&viewport) else {
            diagnostics::record(trace, Phase::NoBounds, Some(&input));
            return;
        };
        let adjustment = scroll.vadjustment();
        let top = adjustment.value() + f64::from(bounds.y());
        diagnostics::record(trace, Phase::BeforeClamp, Some(&input));
        adjustment.clamp_page(top, top + f64::from(bounds.height()));
        diagnostics::record(trace, Phase::AfterClamp, Some(&input));
    }));
}
