use super::terminal_focus_fixture::*;
use gtk::prelude::*;
use rshell_core::{NewLocalTabIdentity, SessionId, SessionUiCommand, UiCommand};
use std::sync::Arc;
#[path = "terminal_focus_receipts.rs"]
mod receipts;
use receipts::RecordingPort as Port;
#[path = "terminal_focus_search.rs"]
mod search;

pub fn run() {
    closed_then_healthy();
    wrong_rendered_action();
    search::inactive_search();
    eprintln!("FOCUS_REVIEW rendered_identity_search_receipt_recovery=true physical=false");
}
fn add(
    window: &mut FocusWindow,
    port: &Port,
    title: &str,
    session: SessionId,
) -> (rshell_core::TabId, rshell_core::PaneId) {
    let (tab, pane) = window.add(title, session, true);
    port.receipts
        .latest(NewLocalTabIdentity { tab, pane, session });
    (tab, pane)
}
fn closed_then_healthy() {
    let port = Arc::new(Port::default());
    let mut window = FocusWindow::launch(port.clone());
    eprintln!(
        "FOCUS_REVIEW_ENV scale={} logical={}x{} gdk_scale_override={:?}",
        window.root().scale_factor(),
        window.root().width(),
        window.root().height(),
        std::env::var("GDK_SCALE").ok()
    );
    window.new_action(false);
    assert_eq!(port.new_count(), 1);
    port.receipts.close(0);
    assert!(port.receipts.receiver_closed(0));
    drain();
    window.new_action(false);
    add(&mut window, &port, "Fresh C", SessionId::new());
    window.publish();
    window.assert_canvas("receipt closed does not permanently poison C");
}
fn settled(window: &FocusWindow) {
    wait(
        || {
            let canvas = window.canvas();
            canvas.is_mapped() && canvas.width() > 0 && canvas.height() > 0
        },
        "review mapped geometry",
    );
    wait(
        || {
            let pane = css(window.root(), "active-pane");
            let region = css(&pane, "pane-action-region");
            let desired = rshell_ui::PaneActionLayout::for_width(
                &[
                    rshell_ui::PaneAction::SplitHorizontal,
                    rshell_ui::PaneAction::SplitVertical,
                    rshell_ui::PaneAction::Reconnect,
                    rshell_ui::PaneAction::Close,
                ],
                pane.width(),
            );
            let children = region.observe_children();
            let widgets = (0..children.n_items())
                .filter_map(|i| children.item(i))
                .filter_map(|w| w.downcast::<gtk::Widget>().ok())
                .collect::<Vec<_>>();
            widgets.iter().filter(|w| w.is::<gtk::Button>()).count() == desired.visible.len()
                && widgets
                    .iter()
                    .all(|w| w.is_mapped() && w.width() > 0 && w.height() > 0)
        },
        "review actual toolbar band",
    );
    let clock = window.root().frame_clock().unwrap();
    let painted = std::rc::Rc::new(std::cell::Cell::new(false));
    let observed = painted.clone();
    let id = clock.connect_after_paint(move |_| observed.set(true));
    clock.request_phase(gtk::gdk::FrameClockPhase::AFTER_PAINT);
    wait(|| painted.get(), "review GTK layout/paint");
    clock.disconnect(id);
}
fn wrong_rendered_action() {
    let port = Arc::new(Port::default());
    let mut window = FocusWindow::launch(port.clone());
    window.new_action(false);
    add(&mut window, &port, "Rendered A", SessionId::new());
    window.publish();
    window.assert_canvas("review A healthy native creation");
    settled(&window);
    window.new_action(false);
    let a_action = button(window.root(), "Close");
    assert!(a_action.grab_focus());
    add(&mut window, &port, "Rendered B", SessionId::new());
    window.publish();
    settled(&window);
    let b_pane = css(window.root(), "active-pane");
    assert!(!focused(window.root()).is_some_and(|w| w.tooltip_text().as_deref() == Some("Close") && w.is_ancestor(&b_pane)), "A action cannot authorize B same-tooltip action");
    assert!(
        !focused(window.root()).is_some_and(|w| w.has_css_class("terminal-canvas")),
        "cancelled B gets no focus"
    );
    window.activate("Rendered A");
    window.assert_canvas("explicit A activation is independent valid intent");
}
fn key(widget: &gtk::Widget, key: gtk::gdk::Key, modifiers: gtk::gdk::ModifierType) -> bool {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .filter_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
        .any(|c| c.emit_by_name::<bool>("key-pressed", &[&key, &0u32, &modifiers]))
}
