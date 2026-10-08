use super::*;

pub(super) fn run() {
    for (tab_add, away_back, pending) in [
        (false, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let port = Arc::new(RecordingPort::default());
        let mut window = FocusWindow::launch(port.clone());
        let origin = action(&window, &port, tab_add);
        let session = SessionId::new();
        if pending {
            add(&mut window, &port, "Delayed", session, false);
            window.publish();
        }
        assert!(button(window.root(), "Terminal settings").grab_focus());
        if away_back {
            assert!(origin.grab_focus());
        }
        let retained = focused(window.root());
        if !pending {
            add(&mut window, &port, "Delayed", session, false);
            window.publish();
        }
        window.ready(session, true);
        window.publish();
        wait(
            || window.canvas().is_mapped(),
            "cancelled receipt target mapped",
        );
        assert_eq!(
            focused(window.root()),
            retained,
            "origin departure never revives"
        );
    }
    let port = Arc::new(RecordingPort::default());
    let mut window = FocusWindow::launch(port.clone());
    let foreign = button(window.root(), "Terminal settings");
    assert!(foreign.grab_focus());
    let retained = focused(window.root());
    window.add("Snapshot only", SessionId::new(), true);
    window.publish();
    wait(
        || window.canvas().is_mapped(),
        "snapshot-only target mapped",
    );
    assert_eq!(focused(window.root()), retained);
    let new = button(window.root(), "New local terminal tab");
    assert!(new.grab_focus());
    new.emit_clicked();
    assert!(foreign.grab_focus());
    let retained = focused(window.root());
    drain();
    add(&mut window, &port, "Queued", SessionId::new(), true);
    window.publish();
    wait(
        || window.canvas().is_mapped(),
        "queued foreign target mapped",
    );
    assert_eq!(focused(window.root()), retained);
    for mode in 0..3 {
        let port = Arc::new(RecordingPort::default());
        let mut window = FocusWindow::launch(port.clone());
        port.reject.store(mode == 0, Ordering::Relaxed);
        let origin = action(&window, &port, false);
        if mode == 1 {
            window.controller.emit(MainWindowMsg::AppEvent(failure()));
            drain();
        }
        add(&mut window, &port, "Late failure", SessionId::new(), true);
        if mode == 2 {
            let pending = Arc::new(AtomicBool::new(true));
            window.controller.emit(MainWindowMsg::LiveEvent {
                view: Box::new(window.view.clone()),
                event: Box::new(failure()),
                pending: pending.clone(),
            });
            wait(
                || !pending.load(Ordering::Acquire),
                "failure before attached snapshot",
            );
        } else {
            window.publish();
        }
        wait(
            || window.canvas().is_mapped(),
            "failed generation target mapped",
        );
        assert_eq!(focused(window.root()).as_ref(), Some(origin.upcast_ref()));
    }
    for close in [false, true] {
        let port = Arc::new(RecordingPort::default());
        let mut window = FocusWindow::launch(port.clone());
        action(&window, &port, false);
        if close {
            port.receipts.close(0);
        } else {
            port.receipts
                .finish(0, rshell_core::NewLocalTabCompletion::NoCreation);
        }
        drain();
        window.add("No receipt", SessionId::new(), true);
        window.publish();
        wait(
            || window.canvas().is_mapped(),
            "closed/no-creation unrelated target mapped",
        );
        assert!(
            !focused(window.root())
                .unwrap()
                .has_css_class("terminal-canvas")
        );
        action(&window, &port, false);
        add(&mut window, &port, "Fresh C", SessionId::new(), true);
        window.publish();
        window.assert_canvas("closed/NoCreation does not poison fresh C");
    }
}
