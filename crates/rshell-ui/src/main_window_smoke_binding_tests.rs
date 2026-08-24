use rshell_core::{AuthenticationKind, ConnectionId, TransportKind};

use crate::{
    ConnectionEditorDraftState, EditorTextField, SmokeAction, SmokeConnectionField,
    SmokeVisualEvidence, SmokeVisualFacts,
    main_window_smoke_binding::editor_action_matches,
    main_window_smoke_visual::{
        VisualCheckpointPhase, visual_checkpoint_binding, visual_checkpoint_component_verified,
    },
};

fn draft() -> ConnectionEditorDraftState {
    ConnectionEditorDraftState {
        id: ConnectionId::new(),
        is_new: true,
        name: "native_password".into(),
        host: "127.0.0.1".into(),
        port: "2222".into(),
        username: "smoke".into(),
        transport: TransportKind::NativeSsh,
        authentication: AuthenticationKind::Password,
        identity_file: String::new(),
        remote_command: String::new(),
        note: String::new(),
        tags: Default::default(),
        secret_changed: true,
        secret_present: true,
    }
}

#[test]
fn visual_checkpoint_binding_is_global_verified_main_window_evidence() {
    let visual = SmokeVisualEvidence {
        facts: SmokeVisualFacts {
            requested_width: 1_360,
            requested_height: 860,
            realized_width: 1_360,
            realized_height: 852,
            command_bar: true,
            dense_sidebar: true,
            tab_strip: true,
            pane_command_row: true,
            terminal_canvas: true,
            content_dialog: true,
            embedded_icon_count: 13,
            focus_or_selection_treatment: true,
        },
        png: None,
    };
    for phase in [
        VisualCheckpointPhase::Idle,
        VisualCheckpointPhase::Opening,
        VisualCheckpointPhase::Observed,
        VisualCheckpointPhase::Closing,
    ] {
        assert!(!visual_checkpoint_component_verified(phase, Some(&visual)));
    }
    assert!(!visual_checkpoint_component_verified(
        VisualCheckpointPhase::Complete,
        None
    ));
    assert!(visual_checkpoint_component_verified(
        VisualCheckpointPhase::Complete,
        Some(&visual)
    ));

    let binding = visual_checkpoint_binding(Some("gtk"), None, true);
    assert!(binding.verified && binding.component_verified);
    assert_eq!(binding.actual_label.as_deref(), Some("main_window"));
    assert!(binding.connection_id.is_none());
    assert!(binding.pane_id.is_none());
    assert!(binding.session_id.is_none());
    assert!(!binding.local);
    assert!(!visual_checkpoint_binding(Some("imports"), None, true).verified);
    assert!(!visual_checkpoint_binding(Some("gtk"), Some("other"), true).verified);
    assert!(!visual_checkpoint_binding(Some("gtk"), None, false).verified);
}

#[test]
fn editor_field_binding_uses_the_actual_component_draft() {
    let draft = draft();
    assert!(editor_action_matches(
        &SmokeAction::SetConnectionField(SmokeConnectionField::Text {
            field: EditorTextField::Name,
            value: "native_password".into(),
        }),
        &draft,
    ));
    assert!(!editor_action_matches(
        &SmokeAction::SetConnectionField(SmokeConnectionField::Text {
            field: EditorTextField::Name,
            value: "native_key".into(),
        }),
        &draft,
    ));
    assert!(editor_action_matches(
        &SmokeAction::SetConnectionField(SmokeConnectionField::SecretFromEnv {
            env_var: "RSHELL_P0_SECRET".into(),
        }),
        &draft,
    ));
}
