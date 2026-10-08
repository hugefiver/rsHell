use super::*;
use rshell_core::{ApplicationHandle, SessionState};

pub(super) async fn observe(window: &FocusWindow, app: &ApplicationHandle, ports: &Ports) {
    let initial = app.initial_view_model();
    let tab = initial.workspace.active_tab().unwrap();
    let pane = tab.active_pane;
    let session = tab.pane_tree.session_id(pane).unwrap().unwrap();
    assert_eq!(initial.workspace.tabs.len(), 1);
    assert_eq!(
        ports.state.lock().unwrap().launched.as_slice(),
        &[(pane, session)]
    );
    until(
        || {
            let view = app.view_model();
            view.session_states.get(&session) == Some(&SessionState::Connected)
                && view.latest_frames.contains_key(&session)
                && find_css(window.root(), "terminal-canvas").is_some_and(|c| {
                    c.is_mapped() && c.is_sensitive() && c.width() > 0 && c.height() > 0
                })
        },
        "initial producer already published / Connected frame / mapped positive canvas",
    )
    .await;
    let clock = window.root().frame_clock().unwrap();
    let painted = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = painted.clone();
    let handler = clock.connect_after_paint(move |_| flag.set(true));
    clock.request_phase(gtk::gdk::FrameClockPhase::AFTER_PAINT);
    until(|| painted.get(), "initial terminal actual paint").await;
    clock.disconnect(handler);
    let actual = focused(window.root());
    let canvas = window.canvas();
    let key = actual.as_ref().and_then(|w| {
        let controllers = w.observe_controllers();
        (0..controllers.n_items())
            .filter_map(|i| controllers.item(i))
            .find_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
    });
    let mut commits = 0;
    let mut handled = false;
    if let Some(key) = key {
        if let Some(im) = key.im_context() {
            im.emit_by_name::<()>("commit", &[&"echo OWNED_STARTUP_PROBE"]);
            commits += 1;
        }
        handled = key.emit_by_name::<bool>(
            "key-pressed",
            &[
                &gtk::gdk::Key::Return,
                &0u32,
                &gtk::gdk::ModifierType::empty(),
            ],
        );
    }
    for _ in 0..8 {
        drain();
        tokio::task::yield_now().await;
    }
    let state = ports.state.lock().unwrap();
    eprintln!(
        "STARTUP_BRIDGE initial_view_tab=true launch_count={} ready=true mapped=true sensitive=true positive=true painted=true root_type={:?} root_canvas={} commits={commits} handled={handled} inputs={} identity_correct={} scale={} no_focus_grab=true physical=false",
        state.attempts,
        actual.as_ref().map(|w| w.type_().name()),
        actual.as_ref() == Some(&canvas),
        state.inputs.len(),
        state.inputs.iter().all(|id| *id == session),
        window.root().scale_factor()
    );
    let attempts = state.attempts;
    let inputs = state.inputs.clone();
    drop(state); // A RED assertion must not poison the owned producer's shutdown mutex.
    assert_eq!(
        attempts, 1,
        "initial producer is outside NewLocal request path"
    );
    assert_eq!(
        actual.as_ref(),
        Some(&canvas),
        "startup actual Root must select initial terminal without new-local action or focus assistance"
    );
    assert_eq!(commits, 1);
    assert!(handled);
    assert_eq!(inputs.as_slice(), &[session, session]);
}
