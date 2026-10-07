use std::collections::BTreeSet;

use rshell_core::InteractionId;

use crate::{
    ShellLayoutMode, SmokeAction, SmokeBindingEvidence, SmokeCounters, SmokeDriverInit,
    SmokeReportHandle, SmokeScenario, SmokeStep, SmokeStepState,
    smoke_driver_observation::SmokeObservation,
    smoke_driver_routing::action_route_ready,
    smoke_driver_state::{SmokeDecision, SmokeDriver},
};

fn observation(active_interaction: Option<InteractionId>) -> SmokeObservation {
    SmokeObservation {
        window_realized: false,
        window_mapped: false,
        window_allocation: (0, 0),
        editor_open: false,
        sidebar_selection: None,
        connection_panes: BTreeSet::new(),
        import_preview_ready: false,
        active_tab: None,
        tab_ids: Vec::new(),
        shutdown_complete: false,
        active_interaction,
        answered_prompts: Vec::new(),
        last_interaction_response: active_interaction,
        binding: None,
        counters: SmokeCounters::default(),
    }
}

#[test]
fn passive_wait_actions_are_never_routed() {
    let observed = observation(None);
    assert!(!action_route_ready(
        &SmokeAction::WaitWindowRealized,
        &observed,
        None,
    ));
    assert!(!action_route_ready(
        &SmokeAction::WaitFrameContains("marker".into()),
        &observed,
        None,
    ));
}

#[test]
fn window_readiness_wait_stays_active_and_routes_resize_once_when_ready() {
    for surface in [None, Some("gtk".to_owned())] {
        let resize = SmokeAction::ResizeWindow {
            width: 800,
            height: 600,
            expected_mode: ShellLayoutMode::Compact,
        };
        let scenario = SmokeScenario::with_steps(
            "window-readiness",
            vec![
                SmokeStep {
                    surface,
                    ..SmokeStep::new(SmokeAction::WaitWindowRealized)
                },
                SmokeStep::new(resize),
                SmokeStep::new(SmokeAction::CloseAll),
            ],
        );
        let init = SmokeDriverInit::new(scenario);
        let report = SmokeReportHandle::new(&init);
        let mut driver = SmokeDriver::new(init, report.clone());
        let mut observed = observation(None);
        observed.binding = Some(SmokeBindingEvidence {
            verified: true,
            component_verified: true,
            ..Default::default()
        });
        assert!(driver.tick(&observed, |_| false).is_none());
        observed.window_realized = true;
        for (mapped, allocation) in [
            (false, (800, 600)),
            (true, (0, 0)),
            (true, (0, 600)),
            (true, (800, 0)),
        ] {
            observed.window_mapped = mapped;
            observed.window_allocation = allocation;
            assert!(driver.tick(&observed, |_| false).is_none());
            assert!(driver.is_active());
            assert_eq!(report.report().steps[0].state, SmokeStepState::Running);
            assert_eq!(report.report().steps[1].state, SmokeStepState::Pending);
        }
        observed.window_mapped = true;
        observed.window_allocation = (800, 600);
        assert!(matches!(
            driver.tick(&observed, |_| false),
            Some(SmokeDecision::Route(SmokeAction::ResizeWindow {
                width: 800,
                height: 600,
                expected_mode: ShellLayoutMode::Compact,
            }))
        ));
        assert_eq!(report.report().steps[0].state, SmokeStepState::Passed);
        assert_eq!(report.report().steps[1].state, SmokeStepState::Running);
        assert!(driver.is_active());
        assert!(driver.tick(&observed, |_| false).is_none());
    }
}

#[test]
fn next_auth_step_waits_for_a_new_interaction_after_submission() {
    let interaction = InteractionId::new();
    let observed = observation(Some(interaction));
    let action = SmokeAction::RespondAuth {
        prompt: 0,
        env_var: "TEST_SECRET".into(),
    };

    assert!(!action_route_ready(&action, &observed, Some(interaction)));
    assert!(action_route_ready(
        &action,
        &observed,
        Some(InteractionId::new())
    ));
}
