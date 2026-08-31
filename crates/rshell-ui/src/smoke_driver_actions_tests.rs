use crate::{SmokeAction, SmokeActionKind, SmokeConnectionField};

#[test]
fn action_list_is_complete_and_secret_debug_is_redacted() {
    let names = SmokeAction::ALL
        .into_iter()
        .map(SmokeActionKind::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "wait_window_realized",
            "new_tab",
            "open_connection_editor",
            "set_connection_field",
            "submit_connection",
            "select_connection",
            "connect",
            "respond_host_key",
            "respond_auth",
            "send_terminal_text",
            "paste_text_from_env",
            "resize_terminal",
            "wait_frame_contains",
            "split_horizontal",
            "split_vertical",
            "switch_tab",
            "search_terminal",
            "select_range",
            "copy_selection",
            "reconnect",
            "visual_checkpoint",
            "preview_import",
            "commit_import",
            "cancel_import",
            "close_all",
            "interrupt_terminal",
            "reset_display",
            "resize_window",
        ]
    );
    let secret = SmokeAction::SetConnectionField(SmokeConnectionField::SecretFromEnv {
        env_var: "RSHELL_TEST_SECRET".into(),
    });
    assert!(!format!("{secret:?}").contains("RSHELL_TEST_SECRET"));
    assert!(
        !format!(
            "{:?}",
            SmokeConnectionField::SecretFromEnv {
                env_var: "RSHELL_TEST_SECRET".into(),
            }
        )
        .contains("RSHELL_TEST_SECRET")
    );
    assert!(
        !format!(
            "{:?}",
            SmokeAction::RespondAuth {
                prompt: 0,
                env_var: "RSHELL_TEST_SECRET".into(),
            }
        )
        .contains("RSHELL_TEST_SECRET")
    );
}

#[test]
fn runtime_selectors_do_not_require_generated_ids() {
    assert!(matches!(
        SmokeAction::SelectConnection("staging".into()),
        SmokeAction::SelectConnection(name) if name == "staging"
    ));
    assert!(matches!(
        SmokeAction::SwitchTab(0),
        SmokeAction::SwitchTab(0)
    ));
}
