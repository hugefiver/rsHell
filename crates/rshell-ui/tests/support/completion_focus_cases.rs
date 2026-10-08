use super::terminal_focus_fixture::*;
use gtk::prelude::*;
use relm4::ComponentController;
use rshell_core::{
    AppEvent, AppFailure, AppFailureCategory, NewLocalTabIdentity, SessionId, UiCommand,
    UiCommandPort, UiPortError,
};
use rshell_ui::MainWindowMsg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
#[path = "terminal_focus_receipts.rs"]
mod receipts;
use receipts::RecordingPort as Port;
#[path = "completion_focus_order.rs"]
mod order;

pub(super) fn failure() -> AppEvent {
    AppEvent::OperationFailed(AppFailure::fatal(
        AppFailureCategory::Pty,
        "completion focus failure",
    ))
}
pub(super) fn add(window: &mut FocusWindow, title: &str, ready: bool) -> NewLocalTabIdentity {
    let session = SessionId::new();
    let (tab, pane) = window.add(title, session, ready);
    NewLocalTabIdentity { tab, pane, session }
}
pub(super) fn settled(window: &FocusWindow) {
    wait(
        || {
            let c = window.canvas();
            c.is_mapped() && c.width() > 0 && c.height() > 0
        },
        "completion view allocation",
    );
    let clock = window.root().frame_clock().unwrap();
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = done.clone();
    let id = clock.connect_after_paint(move |_| flag.set(true));
    clock.request_phase(gtk::gdk::FrameClockPhase::AFTER_PAINT);
    wait(|| done.get(), "completion GTK paint");
    clock.disconnect(id);
}
pub fn run() {
    order::run();
    failure_then_fresh();
    legacy_and_rejection();
    away_back_and_unmap();
    eprintln!(
        "COMPLETION_NATIVE exact_identity_orders_failure_recovery_legacy_cancel=true physical=false"
    );
}
fn failure_then_fresh() {
    for live in [false, true] {
        let port = Arc::new(Port::default());
        let mut window = FocusWindow::launch(port.clone());
        window.new_action(false);
        port.receipts
            .finish(0, rshell_core::NewLocalTabCompletion::NoCreation);
        drain();
        if live {
            let pending = Arc::new(AtomicBool::new(true));
            let unrelated = add(&mut window, "Failure attached view", true);
            window.controller.emit(MainWindowMsg::LiveEvent {
                view: Box::new(window.view.clone()),
                event: Box::new(failure()),
                pending: pending.clone(),
            });
            wait(
                || !pending.load(Ordering::Acquire),
                "failure processed before attached ready view",
            );
            settled(&window);
            assert!(
                !focused(window.root())
                    .unwrap()
                    .has_css_class("terminal-canvas")
            );
            assert_eq!(window.view.workspace.active_tab, Some(unrelated.tab));
        } else {
            window.controller.emit(MainWindowMsg::AppEvent(failure()));
            drain();
        }
        window.new_action(true);
        let c = add(&mut window, "Fresh C", true);
        window.publish();
        port.receipts.created(1, c);
        window.assert_canvas("A failure permits fresh C exact receipt focus");
        window.new_action(false);
        window.controller.emit(MainWindowMsg::AppEvent(failure()));
        drain();
        let d = add(&mut window, "Unrelated cancelled D", true);
        port.receipts.created(2, d);
        window.publish();
        settled(&window);
        assert!(
            !focused(window.root())
                .unwrap()
                .has_css_class("terminal-canvas")
        );
        window.new_action(false);
        let e = add(&mut window, "Healthy E", true);
        port.receipts.created(3, e);
        window.publish();
        window.assert_canvas("unrelated failure cancels once, never poisons later E");
    }
}
#[derive(Default)]
struct Legacy {
    plain: AtomicUsize,
}
impl UiCommandPort for Legacy {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        if matches!(command, UiCommand::NewLocalTab) {
            self.plain.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}
fn legacy_and_rejection() {
    let port = Arc::new(Legacy::default());
    let mut window = FocusWindow::launch(port.clone());
    window.new_action(false);
    assert_eq!(port.plain.load(Ordering::Relaxed), 1);
    add(&mut window, "Unsupported view", true);
    window.publish();
    settled(&window);
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas")
    );
    drop(window);
    let port = Arc::new(Port::default());
    port.reject.store(true, Ordering::Relaxed);
    let mut window = FocusWindow::launch(port.clone());
    window.new_action(false);
    assert_eq!(
        port.new_count(),
        1,
        "tracked rejection has no second plain enqueue"
    );
    assert_eq!(port.receipts.count(), 0);
    add(&mut window, "Rejected foreign view", true);
    window.publish();
    settled(&window);
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas")
    );
    port.reject.store(false, Ordering::Relaxed);
    window.new_action(false);
    port.receipts.close(0);
    drain();
    window.new_action(false);
    let healthy = add(&mut window, "After closed", true);
    port.receipts.created(1, healthy);
    window.publish();
    window.assert_canvas("closed receipt has no permanent state");
}
fn away_back_and_unmap() {
    let port = Arc::new(Port::default());
    let mut window = FocusWindow::launch(port.clone());
    let origin = window.new_action(false);
    assert!(button(window.root(), "Terminal settings").grab_focus());
    assert!(origin.grab_focus());
    let target = add(&mut window, "Away back", true);
    port.receipts.created(0, target);
    window.publish();
    settled(&window);
    assert_eq!(focused(window.root()).as_ref(), Some(origin.upcast_ref()));
    window.new_action(false);
    window.root().close();
    drain();
    let target = add(&mut window, "Closed", true);
    assert!(
        port.receipts.receiver_closed(1),
        "unmap aborts pending receipt receiver"
    );
    port.receipts.created(1, target);
    window.publish();
    assert!(!window.root().is_mapped());
}
