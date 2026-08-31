use std::collections::BTreeSet;

use rshell_core::{InteractionId, SessionState};

use crate::{
    SmokeAction, SmokeCounters,
    smoke_driver_completion::{CompletionContext, action_is_complete},
    smoke_driver_observation::SmokeObservation,
};

fn observation(counters: SmokeCounters) -> SmokeObservation {
    SmokeObservation {
        window_realized: false,
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
        visual_checkpoint_complete: false,
        binding: None,
        counters,
    }
}

#[test]
fn auth_prompt_completion_requires_its_own_dialog_progress() {
    let interaction = InteractionId::new();
    let before = SmokeCounters::default();
    let unrelated_response = observation(SmokeCounters {
        interaction_responses: 1,
        ..Default::default()
    });
    let mut context = CompletionContext::new(&before, &unrelated_response);
    context.auth_interaction = Some(interaction);
    assert!(!action_is_complete(
        &SmokeAction::RespondAuth {
            prompt: 0,
            env_var: "TEST_SECRET".into(),
        },
        &context,
        |_| false,
    ));

    let first_answered = SmokeObservation {
        active_interaction: Some(interaction),
        answered_prompts: vec![0],
        ..unrelated_response
    };
    let mut context = CompletionContext::new(&before, &first_answered);
    context.auth_interaction = Some(interaction);
    assert!(action_is_complete(
        &SmokeAction::RespondAuth {
            prompt: 0,
            env_var: "TEST_SECRET".into(),
        },
        &context,
        |_| false,
    ));

    let other_interaction = InteractionId::new();
    let final_response = SmokeObservation {
        last_interaction_response: Some(other_interaction),
        ..first_answered
    };
    let mut context = CompletionContext::new(&before, &final_response);
    context.auth_interaction = Some(interaction);
    context.auth_submits = true;
    assert!(!action_is_complete(
        &SmokeAction::RespondAuth {
            prompt: 1,
            env_var: "TEST_SECRET".into(),
        },
        &context,
        |_| false,
    ));
    let final_response = SmokeObservation {
        last_interaction_response: Some(interaction),
        counters: SmokeCounters {
            interaction_responses: 1,
            active_session_state: Some(SessionState::Connecting),
            ..Default::default()
        },
        ..final_response
    };
    let mut context = CompletionContext::new(&before, &final_response);
    context.auth_interaction = Some(interaction);
    context.auth_submits = true;
    assert!(!action_is_complete(
        &SmokeAction::RespondAuth {
            prompt: 1,
            env_var: "TEST_SECRET".into(),
        },
        &context,
        |_| false,
    ));
    let final_response = SmokeObservation {
        counters: SmokeCounters {
            interaction_responses: 1,
            active_session_state: Some(SessionState::Connected),
            ..Default::default()
        },
        ..final_response
    };
    let mut context = CompletionContext::new(&before, &final_response);
    context.auth_interaction = Some(interaction);
    context.auth_submits = true;
    assert!(action_is_complete(
        &SmokeAction::RespondAuth {
            prompt: 1,
            env_var: "TEST_SECRET".into(),
        },
        &context,
        |_| false,
    ));
}
