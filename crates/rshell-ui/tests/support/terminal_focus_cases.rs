use super::terminal_focus_fixture::*;
use gtk::prelude::*;
use relm4::ComponentController;
use rshell_core::{
    AppEvent, AppFailure, AppFailureCategory, DisplayRecoveryNotice, NewLocalTabIdentity,
    SessionId, SessionUiEvent, TerminalDisplayModes,
};
use rshell_ui::MainWindowMsg;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "terminal_focus_receipts.rs"]
mod receipts;
use receipts::RecordingPort;
#[path = "terminal_focus_cancellation.rs"]
mod cancellation;
#[path = "terminal_focus_pending.rs"]
mod pending;
#[path = "terminal_focus_structure.rs"]
mod structure;

pub fn run() {
    structure::run();
    cancellation::run();
    pending::run();
    eprintln!(
        "FOCUS_NATIVE producer_receipts_structure_search_foreign_failure=true physical=false"
    );
}
fn action(window: &FocusWindow, port: &RecordingPort, tab_add: bool) -> gtk::Button {
    let expected = port.new_count() + 1;
    let origin = window.new_action(tab_add);
    wait(
        || port.new_count() == expected,
        "actual new-local atomic submit before view",
    );
    origin
}
fn add(
    window: &mut FocusWindow,
    port: &RecordingPort,
    title: &str,
    session: SessionId,
    ready: bool,
) -> (rshell_core::TabId, rshell_core::PaneId) {
    let (tab, pane) = window.add(title, session, ready);
    port.receipts
        .latest(NewLocalTabIdentity { tab, pane, session });
    (tab, pane)
}
fn structural(window: &FocusWindow, session: SessionId, show: bool) {
    let notice = DisplayRecoveryNotice {
        interrupted_generation: 1,
        observed_generation: 2,
        modes: TerminalDisplayModes {
            alternate_screen: true,
            ..Default::default()
        },
    };
    window
        .controller
        .emit(MainWindowMsg::AppEvent(AppEvent::Session {
            session,
            event: SessionUiEvent::RecoveryChanged(show.then_some(notice)),
        }));
    drain();
}
fn failure() -> AppEvent {
    AppEvent::OperationFailed(AppFailure::fatal(
        AppFailureCategory::Pty,
        "isolated focus fixture failure",
    ))
}
fn key(widget: &gtk::Widget, key: gtk::gdk::Key, state: gtk::gdk::ModifierType) -> bool {
    let controllers = widget.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .filter_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
        .any(|c| c.emit_by_name::<bool>("key-pressed", &[&key, &0u32, &state]))
}
