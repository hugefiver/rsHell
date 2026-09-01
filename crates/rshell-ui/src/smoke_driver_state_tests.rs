use std::collections::BTreeSet;

use rshell_core::InteractionId;

use crate::{
    SmokeAction, SmokeCounters, smoke_driver_observation::SmokeObservation,
    smoke_driver_routing::action_route_ready,
};

fn observation(active_interaction: Option<InteractionId>) -> SmokeObservation {
    SmokeObservation {
        window_realized: false,
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
