use std::collections::BTreeSet;

use rshell_core::{InteractionId, SessionState};

use crate::{
    SmokeAction, SmokeBindingEvidence, SmokeConnectionField, SmokeCounters,
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
        binding: None,
        counters,
    }
}

#[test]
fn labeled_editor_action_waits_for_its_exact_component_binding() {
    let before = SmokeCounters::default();
    let mut observed = observation(SmokeCounters {
        editor_revisions: 1,
        ..Default::default()
    });
    observed.editor_open = true;
    let action = SmokeAction::SetConnectionField(SmokeConnectionField::Port(2222));
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
    observed.binding = Some(SmokeBindingEvidence {
        verified: true,
        component_verified: true,
        ..Default::default()
    });
    assert!(action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
}

fn complete(action: &SmokeAction, before: &SmokeCounters, now: &SmokeObservation) -> bool {
    action_is_complete(action, &CompletionContext::new(before, now), |_| false)
}

#[test]
fn completion_waits_for_monotonic_app_events_not_transient_dialog_state() {
    let before = SmokeCounters::default();
    let unchanged = observation(before.clone());
    for action in [
        SmokeAction::SubmitConnection,
        SmokeAction::RespondHostKey { accept: true },
        SmokeAction::RespondAuth {
            prompt: 0,
            env_var: "TEST_SECRET".into(),
        },
        SmokeAction::CommitImport,
        SmokeAction::CancelImport,
    ] {
        assert!(!complete(&action, &before, &unchanged));
    }

    let counters = SmokeCounters {
        catalog_changes: 1,
        ..Default::default()
    };
    assert!(complete(
        &SmokeAction::SubmitConnection,
        &before,
        &observation(counters),
    ));
    let counters = SmokeCounters {
        interaction_responses: 1,
        active_session_state: Some(SessionState::Connecting),
        ..Default::default()
    };
    let host_interaction = InteractionId::new();
    let mut host_response = observation(counters);
    host_response.last_interaction_response = Some(host_interaction);
    let mut host_context = CompletionContext::new(&before, &host_response);
    host_context.auth_interaction = Some(host_interaction);
    host_context.auth_submits = true;
    assert!(!action_is_complete(
        &SmokeAction::RespondHostKey { accept: true },
        &host_context,
        |_| false,
    ));
    host_response.counters.active_session_state = Some(SessionState::Connected);
    let mut host_context = CompletionContext::new(&before, &host_response);
    host_context.auth_interaction = Some(host_interaction);
    host_context.auth_submits = true;
    assert!(action_is_complete(
        &SmokeAction::RespondHostKey { accept: true },
        &host_context,
        |_| false,
    ));
    let counters = SmokeCounters {
        interaction_responses: 1,
        ..Default::default()
    };
    assert!(!complete(
        &SmokeAction::RespondAuth {
            prompt: 0,
            env_var: "TEST_SECRET".into(),
        },
        &before,
        &observation(counters),
    ));
    let counters = SmokeCounters {
        import_completions: 1,
        ..Default::default()
    };
    assert!(!complete(
        &SmokeAction::CommitImport,
        &before,
        &observation(counters),
    ));
    let counters = SmokeCounters {
        import_cancellations: 1,
        ..Default::default()
    };
    assert!(!complete(
        &SmokeAction::CancelImport,
        &before,
        &observation(counters),
    ));
    let counters = SmokeCounters {
        import_completions: 1,
        import_cancellations: 1,
        imports: crate::SmokeImportEvidence {
            sequence: 1,
            completed: true,
            commit_source: Some(rshell_core::ImportSourceKind::LegacyRshellJson),
            expected_groups: 1,
            expected_connections: 1,
            imported_groups: 1,
            imported_connections: 1,
            exact_group: true,
            exact_connection: true,
            authentication: Some(rshell_core::AuthenticationKind::Agent),
            authentication_matches: true,
            credential_reference_matches: true,
            terminal_override_matches: true,
            pending_preview_count: 0,
            cancel_pending_zero: true,
            preview: Some(crate::SmokeImportPreviewEvidence {
                sequence: 1,
                source: rshell_core::ImportSourceKind::OpenSshConfig,
                expected_groups: 0,
                expected_candidates: 1,
                actual_groups: 0,
                actual_candidates: 1,
                actual_group_name: None,
                actual_candidate_name: Some("p0-cancel".into()),
                actual_host: Some("cancel.example.test".into()),
                authentication: Some(rshell_core::AuthenticationKind::Agent),
                credential_reference_present: Some(false),
                terminal_override_present: Some(false),
                importable: Some(true),
                wildcard: Some(false),
                exact_group: true,
                exact_candidate: true,
                authentication_matches: true,
                credential_reference_matches: true,
                terminal_override_matches: true,
                importable_matches: true,
                wildcard_matches: true,
            }),
            cancel_sequence: 1,
            cancelled_preview_matches: true,
        },
        ..Default::default()
    };
    let observed = observation(counters);
    assert!(complete(&SmokeAction::CommitImport, &before, &observed));
    assert!(complete(&SmokeAction::CancelImport, &before, &observed));
}

#[test]
fn stale_exact_resize_evidence_cannot_complete_a_new_resize_action() {
    let frame = crate::SmokeFrameEvidence {
        generation: 9,
        cols: 120,
        rows: 40,
        pixel_width: 960,
        pixel_height: 640,
        dpi: 96,
    };
    let resize = crate::SmokeResizeEvidence {
        sequence: 7,
        input_width: 960,
        input_height: 640,
        input_scale_bits: 1.0f64.to_bits(),
        requested: frame,
        observed: Some(frame),
        exact: true,
    };
    let before = SmokeCounters {
        terminal: crate::SmokeTerminalEvidence {
            resize: Some(resize),
            ..Default::default()
        },
        ..Default::default()
    };
    let action = SmokeAction::ResizeTerminal {
        width: 960,
        height: 640,
        scale: 1.0,
    };
    assert!(!complete(&action, &before, &observation(before.clone())));

    let mut fresh = before.clone();
    fresh.terminal.resize.as_mut().expect("resize").sequence = 8;
    assert!(complete(&action, &before, &observation(fresh)));
}
