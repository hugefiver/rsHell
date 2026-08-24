use std::collections::BTreeSet;

use rshell_core::{AuthenticationKind, ImportSourceKind};

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

fn complete(action: &SmokeAction, before: &SmokeCounters, now: &SmokeObservation) -> bool {
    action_is_complete(action, &CompletionContext::new(before, now), |_| false)
}

#[test]
fn openssh_preview_requires_new_exact_source_specific_evidence() {
    let expected = crate::SmokeImportExpectation {
        groups: 0,
        connections: 1,
        group_name: String::new(),
        connection_name: "p0-cancel".into(),
        host: "cancel.example.test".into(),
        authentication: AuthenticationKind::Agent,
        credential_reference_present: false,
        terminal_override_present: false,
        importable: true,
        wildcard: false,
    };
    let action = SmokeAction::PreviewImport {
        source: ImportSourceKind::OpenSshConfig,
        path: "openssh.conf".into(),
        expected: Some(expected),
    };
    let before = SmokeCounters::default();
    let mut counters = SmokeCounters {
        import_revisions: 1,
        ..SmokeCounters::default()
    };
    counters.imports.preview = Some(crate::SmokeImportPreviewEvidence {
        sequence: 1,
        source: ImportSourceKind::OpenSshConfig,
        expected_groups: 0,
        expected_candidates: 1,
        actual_groups: 0,
        actual_candidates: 0,
        actual_group_name: None,
        actual_candidate_name: Some("p0-cancel".into()),
        actual_host: Some("cancel.example.test".into()),
        authentication: Some(AuthenticationKind::Agent),
        credential_reference_present: Some(false),
        terminal_override_present: Some(false),
        importable: Some(true),
        wildcard: Some(false),
        exact_group: true,
        exact_candidate: false,
        authentication_matches: false,
        credential_reference_matches: false,
        terminal_override_matches: false,
        importable_matches: false,
        wildcard_matches: false,
    });
    let mut observed = observation(counters.clone());
    observed.import_preview_ready = true;
    assert!(!complete(&action, &before, &observed));

    let preview = counters.imports.preview.as_mut().expect("preview");
    preview.actual_candidates = 1;
    preview.exact_candidate = true;
    preview.authentication_matches = true;
    preview.credential_reference_matches = true;
    preview.terminal_override_matches = true;
    preview.importable_matches = true;
    preview.wildcard_matches = true;
    let mut observed = observation(counters);
    observed.import_preview_ready = true;
    assert!(complete(&action, &before, &observed));
}

#[test]
fn paste_and_color_require_their_own_real_frame_effects() {
    let before = SmokeCounters::default();
    let paste = SmokeAction::PasteTextFromEnv {
        env_var: "TEST_SECRET".into(),
        effect_marker: "paste-effect".into(),
    };
    let unrelated = observation(SmokeCounters {
        terminal_commands: 1,
        ..Default::default()
    });
    assert!(!complete(&paste, &before, &unrelated));

    let mut counters = SmokeCounters::default();
    counters.terminal.paste = Some(crate::SmokePasteEvidence {
        sequence: 1,
        expected_bytes: 12,
        actual_bytes: 12,
        command_exact: true,
        frame_effect: false,
    });
    assert!(!complete(&paste, &before, &observation(counters.clone())));
    counters
        .terminal
        .paste
        .as_mut()
        .expect("paste")
        .frame_effect = true;
    assert!(complete(&paste, &before, &observation(counters)));

    let color = SmokeAction::SendTerminalText {
        text: "fixture".into(),
        expected_color_marker: Some("color-marker".into()),
    };
    let mut counters = SmokeCounters::default();
    counters.terminal.color = Some(crate::SmokeColorEvidence {
        sequence: 1,
        marker_bytes: 12,
        marker_cells: 12,
        non_default_foreground: false,
        red_foreground: false,
    });
    assert!(!complete(&color, &before, &observation(counters.clone())));
    let evidence = counters.terminal.color.as_mut().expect("color");
    evidence.non_default_foreground = true;
    evidence.red_foreground = true;
    assert!(complete(&color, &before, &observation(counters)));
}
