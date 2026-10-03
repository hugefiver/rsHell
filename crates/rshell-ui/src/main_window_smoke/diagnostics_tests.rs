use super::*;
use crate::{
    SmokeCounters, SmokeDriverInit, SmokeReportHandle, SmokeScenario, SmokeScenarioState,
    SmokeWindowResizeEvidence, smoke_driver_observation::SmokeObservation,
    smoke_driver_state::SmokeDecision,
};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

fn observed() -> SmokeObservation {
    SmokeObservation {
        window_realized: true,
        editor_open: false,
        sidebar_selection: None,
        connection_panes: BTreeSet::new(),
        import_preview_ready: false,
        active_tab: None,
        tab_ids: Vec::new(),
        shutdown_complete: false,
        active_interaction: None,
        answered_prompts: Vec::new(),
        last_interaction_response: None,
        binding: None,
        counters: SmokeCounters::default(),
    }
}

fn resize_driver() -> (SmokeDriver, SmokeObservation, Option<ResizeDiagnostics>) {
    let resize = SmokeAction::ResizeWindow {
        width: 800,
        height: 600,
        expected_mode: ShellLayoutMode::Compact,
    };
    let init = SmokeDriverInit::new(SmokeScenario::new(vec![
        SmokeAction::WaitWindowRealized,
        resize.clone(),
        SmokeAction::CloseAll,
    ]));
    let report = SmokeReportHandle::new(&init);
    let mut driver = SmokeDriver::new(init, report.clone());
    let observed = observed();
    assert!(driver.tick(&observed, |_| false).is_none());
    assert!(matches!(
        driver.tick(&observed, |_| false),
        Some(SmokeDecision::Route(SmokeAction::ResizeWindow { .. }))
    ));
    let pending = ResizeDiagnostics::for_route(&resize, Some(1), true);
    (driver, observed, pending)
}

#[test]
fn first_resize_timeout_keeps_the_failure_and_yields_one_terminal_snapshot() {
    let (mut driver, observed, mut pending) = resize_driver();
    let action = &driver.current.as_ref().unwrap().action;
    assert!(ResizeDiagnostics::for_route(action, Some(1), false).is_none());
    driver.current.as_mut().unwrap().started = Instant::now() - Duration::from_secs(11);
    assert!(matches!(
        driver.tick(&observed, |_| false),
        Some(SmokeDecision::Quit)
    ));
    let terminal = ResizeDiagnostics::terminal_snapshot(&mut pending, &driver)
        .expect("timeout must produce the terminal diagnostic before Quit");
    assert_eq!(terminal.1, "terminal_failed");
    assert_eq!((terminal.0.width, terminal.0.height), (800, 600));
    assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
    let report = driver.report.report();
    assert_eq!(
        report.failure.as_ref().map(|failure| failure.code),
        Some("step_timeout")
    );
    assert_eq!(
        report.failure.as_ref().and_then(|failure| failure.step),
        Some(1)
    );
}

#[test]
fn asynchronous_failure_flushes_first_resize_terminal_snapshot_once() {
    let (mut driver, observed, mut pending) = resize_driver();
    assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
    driver.fail(&observed, "command_rejected");
    let (_, phase) = ResizeDiagnostics::terminal_snapshot(&mut pending, &driver)
        .expect("asynchronous failure must flush before Quit");
    assert_eq!(phase, "terminal_failed");
    assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
    let report = driver.report.report();
    assert_eq!(report.state, SmokeScenarioState::Failed);
    assert_eq!(
        report.failure.as_ref().map(|failure| failure.code),
        Some("command_rejected")
    );
}

#[test]
fn first_resize_success_is_reported_after_driver_advances() {
    let (mut driver, mut observed, mut pending) = resize_driver();
    observed.counters.window_resize = Some(SmokeWindowResizeEvidence {
        sequence: 1,
        requested_width: 800,
        requested_height: 600,
        realized_width: 800,
        realized_height: 600,
        expected_layout: ShellLayoutMode::Compact,
        layout: ShellLayoutMode::Compact,
    });
    assert!(matches!(
        driver.tick(&observed, |_| false),
        Some(SmokeDecision::Route(SmokeAction::CloseAll))
    ));
    assert_eq!(driver.current.as_ref().map(|step| step.index), Some(2));
    assert_eq!(
        ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).map(|(_, phase)| phase),
        Some("terminal_passed")
    );
}

#[test]
fn disabled_later_and_non_resize_routes_do_not_arm() {
    let action = SmokeAction::ResizeWindow {
        width: 800,
        height: 600,
        expected_mode: ShellLayoutMode::Compact,
    };
    assert!(ResizeDiagnostics::for_route(&action, Some(1), false).is_none());
    assert!(ResizeDiagnostics::for_route(&action, Some(2), true).is_none());
    assert!(ResizeDiagnostics::for_route(&action, None, true).is_none());
    assert!(ResizeDiagnostics::for_route(&SmokeAction::CloseAll, Some(1), true).is_none());
}
