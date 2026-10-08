use super::*;

pub(super) fn run() {
    for receipt_first in [false, true] {
        let port = Arc::new(Port::default());
        let mut window = FocusWindow::launch(port.clone());
        window.new_action(false);
        let a = add(&mut window, "Baseline A", true);
        port.receipts.latest(a);
        window.publish();
        window.assert_canvas("healthy baseline A");
        window.new_action(true);
        let c = add(&mut window, "Target C", false);
        if receipt_first {
            port.receipts.created(1, c);
            drain();
        } else {
            window.publish();
            drain();
            assert!(
                !focused(window.root())
                    .unwrap()
                    .has_css_class("terminal-canvas")
            );
        }
        if receipt_first {
            window.publish();
        } else {
            port.receipts.created(1, c);
        }
        window.ready(c.session, true);
        window.publish();
        window.assert_canvas("both receipt/view orders Pending -> Terminal");
        window.publish();
        assert_eq!(focused(window.root()).as_ref(), Some(&window.canvas()));
    }
    for receipt_first in [false, true] {
        let port = Arc::new(Port::default());
        let mut window = FocusWindow::launch(port.clone());
        window.new_action(false);
        window.new_action(false); // A + B independent producers
        port.receipts
            .finish(0, rshell_core::NewLocalTabCompletion::NoCreation);
        window.new_action(true); // C replaces B UI task but does not cancel B producer
        assert!(
            port.receipts.receiver_closed(1),
            "replacement aborts only B UI receipt consumer"
        );
        let b = add(&mut window, "Late B", true);
        if receipt_first {
            port.receipts.created(1, b);
            drain();
            window.publish();
        } else {
            window.publish();
            settled(&window);
            port.receipts.created(1, b);
        }
        settled(&window);
        assert!(
            !focused(window.root())
                .unwrap()
                .has_css_class("terminal-canvas"),
            "C can never claim B"
        );
        let c = add(&mut window, "Exact C", true);
        if receipt_first {
            port.receipts.created(2, c);
            drain();
            window.publish();
        } else {
            window.publish();
            settled(&window);
            port.receipts.created(2, c);
        }
        window.assert_canvas("coalesced B+C view uses only C producer identity");
        assert_eq!(window.view.workspace.active_tab, Some(c.tab));
    }
    let port = Arc::new(Port::default());
    let mut window = FocusWindow::launch(port.clone());
    window.new_action(false);
    let a = add(&mut window, "Activate A", true);
    port.receipts.created(0, a);
    window.publish();
    window.assert_canvas("A ready");
    window.new_action(false);
    let b = add(&mut window, "Activate B", true);
    port.receipts.created(1, b);
    window.publish();
    window.assert_canvas("B ready");
    window.new_action(false);
    window.activate("Activate A");
    window.assert_canvas("explicit tab A cancels pending creation intent");
    let c = add(&mut window, "Late after activate", true);
    port.receipts.created(2, c);
    window.publish();
    settled(&window);
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas"),
        "late C cannot revive cancelled generation"
    );
}
