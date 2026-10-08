use super::terminal_focus_fixture::*;
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{AppBootstrapState, AppSettings, ApplicationService, TerminalProfile};
use rshell_ui::{MainWindow, MainWindowInit};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
#[path = "completion_bridge_ports.rs"]
mod completion_bridge_ports;
#[path = "completion_startup_construction.rs"]
mod construction;
#[path = "completion_startup_probe.rs"]
mod startup;
#[path = "completion_startup_cases.rs"]
mod startup_cases;
#[path = "completion_bridge_stubs.rs"]
mod stubs;
use completion_bridge_ports::Ports;

pub async fn run() {
    scenario(Scenario::Retry(false)).await;
    scenario(Scenario::Retry(true)).await;
    scenario(Scenario::Startup).await;
    for case in startup_cases::Case::ALL {
        scenario(Scenario::Gated(*case)).await;
    }
    scenario(Scenario::Construction(true)).await;
    scenario(Scenario::Construction(false)).await;
}
#[derive(Clone, Copy)]
enum Scenario {
    Retry(bool),
    Startup,
    Gated(startup_cases::Case),
    Construction(bool),
}
async fn scenario(kind: Scenario) {
    let ports = if matches!(kind, Scenario::Gated(_) | Scenario::Construction(true)) {
        Ports::pending_initial()
    } else {
        Ports::new()
    };
    let application = Arc::new(
        ApplicationService::start(
            ports.dependencies(),
            AppBootstrapState {
                catalog: Default::default(),
                settings: AppSettings::default(),
                terminal_profiles: vec![TerminalProfile::default()],
            },
        )
        .await
        .unwrap(),
    );
    let local = tokio::task::LocalSet::new();
    let app = application.clone();
    let fixture = ports.clone();
    let scenario = local.spawn_local(async move {
        gtk::init().expect("real application bridge GTK");
        if let Scenario::Construction(foreign) = kind {
            construction::run(&app, &fixture, foreign).await;
            return;
        }
        let controller = MainWindow::builder().launch(MainWindowInit::from_application(&app)).detach();
        controller.widget().set_default_size(1_000, 700);
        let window = FocusWindow { controller, view: app.view_model() };
        window.root().present(); until(|| window.root().is_mapped(), "owned real bridge window mapped").await;
        if matches!(kind, Scenario::Startup) { startup::observe(&window, &app, &fixture).await; return; }
        if let Scenario::Gated(case) = kind { startup_cases::run(window, &app, &fixture, case).await; return; }
        window.new_action(false);
        until(|| fixture.state.lock().unwrap().attempts == 2, "actual A command loop launch started").await;
        if matches!(kind, Scenario::Retry(true)) { panic!("intentional gated service/window cleanup probe"); }
        fixture.release.try_send(false).unwrap();
        until(|| descendants(window.root()).into_iter().filter_map(|w| w.downcast::<gtk::Label>().ok())
            .any(|l| l.has_css_class("command-status") && l.text().as_str() == "session operation failed"), "real failure AppEvent consumed by MainWindow").await;
        assert_eq!(app.view_model().workspace.tabs.len(), 1, "A failed with no creation");
        assert_eq!(fixture.state.lock().unwrap().failures, 1);
        window.new_action(true);
        until(|| fixture.state.lock().unwrap().attempts == 3, "actual C loop launch started").await;
        fixture.release.try_send(true).unwrap();
        until(|| app.view_model().workspace.tabs.len() == 2 && find_css(window.root(), "active-pane").is_some_and(|pane|
            find_css(&pane, "terminal-canvas").is_some_and(|c| c.is_mapped() && focused(window.root()).as_ref() == Some(&c))), "real receipt/view C native Root focus").await;
        let view = app.view_model(); let current = view.workspace.active_tab().unwrap();
        let session = current.pane_tree.session_ids()[0]; let pane = current.active_pane;
        assert_eq!(fixture.state.lock().unwrap().launched.last().copied(), Some((pane, session)));
        startup_cases::painted(&window).await;
        assert_retry_input(&window, &fixture, session).await;
        eprintln!("COMPLETION_BRIDGE real_application_init=true failure_A_then_C=true exact_pane_session=true root_canvas=true inputs=2 identity_correct=true scale={} physical=false", window.root().scale_factor());
    });
    let result = local.run_until(scenario).await;
    ports.release.close(); // Release only this fixture's blocked launches on an assertion failure.
    let shutdown = tokio::time::timeout(Duration::from_secs(5), application.shutdown()).await;
    assert!(
        matches!(shutdown, Ok(Ok(()))),
        "actual service shutdown on success/failure"
    );
    assert!(ports.state.lock().unwrap().live.is_empty());
    eprintln!(
        "COMPLETION_BRIDGE_CLEANUP loop_shutdown=true live_bindings=0 assertion_unwind={}",
        result.is_err()
    );
    if matches!(kind, Scenario::Retry(true)) {
        assert!(result.is_err(), "owned cleanup probe actually panicked");
        return;
    }
    if let Err(error) = result {
        if error.is_panic() {
            std::panic::resume_unwind(error.into_panic());
        }
        panic!("native bridge task cancelled");
    }
}
async fn assert_retry_input(window: &FocusWindow, ports: &Ports, session: rshell_core::SessionId) {
    let root_selected = focused(window.root()).expect("actual C Root-selected focus");
    assert_eq!(root_selected, window.canvas());
    let controllers = root_selected.observe_controllers();
    let keys = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
        .expect("Root-selected C native key controller");
    keys.im_context()
        .unwrap()
        .emit_by_name::<()>("commit", &[&"bridge-C"]);
    assert!(keys.emit_by_name::<bool>(
        "key-pressed",
        &[
            &gtk::gdk::Key::Return,
            &0u32,
            &gtk::gdk::ModifierType::empty()
        ],
    ));
    until(
        || ports.state.lock().unwrap().inputs.len() == 2,
        "actual C IM commit and Return routed",
    )
    .await;
    let inputs = ports.state.lock().unwrap().inputs.clone();
    assert_eq!(
        inputs,
        [session, session],
        "actual C input identity after A failure"
    );
}
async fn until(mut condition: impl FnMut() -> bool, stage: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        drain();
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "{stage}");
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}
