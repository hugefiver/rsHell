#![cfg(windows)]

use relm4::gtk::{self, prelude::*};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[path = "support/terminal_input_focus_fixture.rs"]
mod owned_pty;
#[path = "../crates/rshell-ui/tests/support/terminal_focus_fixture.rs"]
mod ui_focus;
use owned_pty::OwnedPty;
use ui_focus::{FocusWindow, drain, focused, wait};

#[test]
fn actual_main_window_root_input_reaches_only_current_owned_pty() {
    let pty = OwnedPty::new();
    let manager = pty.port.manager().clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || run(pty)));
    assert_eq!(
        manager.active_session_count(),
        0,
        "owned actors after success or assertion unwind"
    );
    assert_eq!(
        manager.active_child_process_count(),
        0,
        "owned children after success or assertion unwind"
    );
    eprintln!(
        "FOCUS_SCENARIO_CLEANUP actors=0 children=0 assertion_unwind={}",
        result.is_err()
    );
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

fn run(mut pty: OwnedPty) {
    let mut window = FocusWindow::launch(pty.port.clone());
    window.new_action(false);
    wait(
        || pty.port.new_count() == 1,
        "actual NewLocalTab producer received before snapshot",
    );
    let a = pty.launch();
    let (tab, pane) = window.add("Owned A", a, false);
    pty.port.created(rshell_core::NewLocalTabIdentity {
        tab,
        pane,
        session: a,
    });
    window.publish();
    settle(&mut pty, &mut window, a);
    wait(
        || {
            let canvas = window.canvas();
            canvas.is_mapped() && canvas.width() > 0 && canvas.height() > 0
        },
        "first real terminal mapped before Root-selected input",
    );
    send_and_assert(&mut pty, &mut window, a, None);
    window.new_action(true);
    wait(|| pty.port.new_count() == 2, "native tab-add producer");
    let b = pty.launch();
    let (tab, pane) = window.add("Owned B", b, false);
    pty.port.created(rshell_core::NewLocalTabIdentity {
        tab,
        pane,
        session: b,
    });
    window.publish();
    settle(&mut pty, &mut window, b);
    window.assert_canvas("second identity automatically focused");
    send_and_assert(&mut pty, &mut window, b, Some(a));
    window.activate("Owned A");
    window.assert_canvas("switch back to recreated A controller");
    send_and_assert(&mut pty, &mut window, a, Some(b));
    pty.shutdown();
    drop(window);
    drop(pty);
    let manager;
    {
        let cleanup = OwnedPty::new();
        manager = cleanup.port.manager().clone();
        let mut cleanup = cleanup;
        let _ = cleanup.launch();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = cleanup;
            panic!("intentional owned fixture unwind cleanup");
        }));
        assert!(result.is_err());
    }
    assert_eq!(manager.active_session_count(), 0);
    assert_eq!(manager.active_child_process_count(), 0);
    eprintln!(
        "FOCUS_REAL_PTY two_sessions=true root_selected_im_return=true inputs_each=2 identity=true expanded=true physical=false unwind_cleanup=true"
    );
}

fn settle(pty: &mut OwnedPty, window: &mut FocusWindow, session: rshell_core::SessionId) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pty.pump(window);
        if pty.ready(session, window) {
            window.publish();
            drain();
            return;
        }
        assert!(
            Instant::now() < deadline,
            "real owned PTY connected/frame readiness"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn send_and_assert(
    pty: &mut OwnedPty,
    window: &mut FocusWindow,
    target: rshell_core::SessionId,
    other: Option<rshell_core::SessionId>,
) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let token = format!("FOCUS_{nonce:x}");
    let command = format!("echo {token}_%OS%");
    let expanded = format!("{token}_Windows_NT");
    assert!(
        !command.contains(&expanded),
        "input echo is not shell execution evidence"
    );
    let before = pty.port.inputs.lock().unwrap().len();
    let root_focus = focused(window.root()).expect("actual Root-selected input target");
    let controllers = root_focus.observe_controllers();
    let keys = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::EventControllerKey>().ok());
    let mut commits = 0;
    let mut handled = false;
    if let Some(keys) = keys {
        if let Some(im) = keys.im_context() {
            im.emit_by_name::<()>("commit", &[&command]);
            commits += 1;
        }
        handled = keys.emit_by_name::<bool>(
            "key-pressed",
            &[
                &gtk::gdk::Key::Return,
                &0u32,
                &gtk::gdk::ModifierType::empty(),
            ],
        );
    }
    pty.pump(window);
    let arrivals = pty.port.inputs.lock().unwrap().len() - before;
    eprintln!(
        "FOCUS_INPUT root_type={} canvas={} mapped={} commits={commits} handled={handled} arrivals={arrivals} connected={} children={} physical=false",
        root_focus.type_().name(),
        root_focus == window.canvas(),
        window.canvas().is_mapped(),
        pty.ready(target, window),
        pty.port.manager().active_child_process_count()
    );
    assert_eq!(
        root_focus,
        window.canvas(),
        "Root-selected focus must be current terminal; no assistance or unfocused bypass"
    );
    assert_eq!(commits, 1, "Root-selected native IM commit");
    assert!(handled, "Root-selected native Return");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        pty.pump(window);
        if pty.contains(target, &expanded) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "expanded harmless command output missing from owned target PTY"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let inputs = pty.port.inputs.lock().unwrap();
    assert_eq!(
        &inputs[before..],
        &[target, target],
        "IM commit + Return must yield exactly two correct-session inputs"
    );
    assert!(
        other.is_none_or(|session| !pty.contains(session, &expanded)),
        "wrong session received expanded token"
    );
    eprintln!(
        "FOCUS_PTY_OUTPUT target={target:?} other={other:?} arrivals=2 identity_correct=true expanded_only_target=true"
    );
    window.assert_canvas("real output frame must not disturb focused current canvas");
}
