use rshell_core::{
    AuthPrompt, HostKeyPrompt, InteractionId, InteractionRequest, KeyboardInteractivePrompt,
    SessionId,
};
use rshell_ui::{InteractionAction, InteractionViewModel};

#[test]
fn changed_host_key_has_no_accept_action_and_unknown_key_shows_algorithm_and_sha256() {
    let changed = InteractionViewModel::new(
        SessionId::new(),
        InteractionRequest::HostKey(host_key(true)),
    );
    assert_eq!(
        changed.actions(),
        &[InteractionAction::CopyDiagnostics, InteractionAction::Close]
    );

    let unknown = InteractionViewModel::new(
        SessionId::new(),
        InteractionRequest::HostKey(host_key(false)),
    );
    assert_eq!(
        unknown.actions(),
        &[InteractionAction::Reject, InteractionAction::AcceptAndStore]
    );
    assert_eq!(unknown.endpoint(), Some("server.example.test:2222"));
    assert_eq!(unknown.fingerprint(), Some("ssh-ed25519 SHA256:abc"));
}

#[test]
fn keyboard_interactive_masks_non_echo_answers_and_clears_after_send() {
    let first = AuthPrompt {
        id: InteractionId::new(),
        label: "Visible answer".into(),
        echo: true,
    };
    let interaction = first.id;
    let second = AuthPrompt {
        id: InteractionId::new(),
        label: "Secret answer".into(),
        echo: false,
    };
    let mut vm = InteractionViewModel::new(
        SessionId::new(),
        InteractionRequest::KeyboardInteractive(KeyboardInteractivePrompt {
            id: interaction,
            name: "Challenge".into(),
            instruction: "Answer in order".into(),
            prompts: vec![first, second],
        }),
    );
    vm.set_answer(0, "visible".into()).expect("first answer");
    vm.set_answer(1, "secret".into()).expect("second answer");

    let command = vm.response_command().expect("response command");

    assert_eq!(vm.answer_lengths(), vec![0, 0]);
    assert!(!format!("{command:?}").contains("visible"));
    assert!(!format!("{command:?}").contains("secret"));
    assert!(
        vm.response_command().is_none(),
        "secret handoff is once-only"
    );
}

#[test]
fn auth_cancel_is_one_shot_and_redacted_even_after_input() {
    let prompt = AuthPrompt {
        id: InteractionId::new(),
        label: "Password".into(),
        echo: false,
    };
    let mut vm = InteractionViewModel::new(SessionId::new(), InteractionRequest::Password(prompt));
    vm.set_answer(0, "do-not-print".into()).unwrap();
    let cancel = vm.cancel_command().expect("first cancel");
    assert!(!format!("{vm:?} {cancel:?}").contains("do-not-print"));
    assert_eq!(vm.answer_lengths(), [0]);
    assert!(vm.cancel_command().is_none());
}

#[test]
fn rejected_secret_response_stays_wiped_and_allows_fresh_retry() {
    let prompt = AuthPrompt {
        id: InteractionId::new(),
        label: "Password".into(),
        echo: false,
    };
    let mut vm = InteractionViewModel::new(SessionId::new(), InteractionRequest::Password(prompt));
    vm.set_answer(0, "first-secret".into()).unwrap();
    let first = vm.response_command().expect("first submission");
    assert_eq!(vm.answer_lengths(), [0]);

    vm.submission_failed();
    assert_eq!(vm.answer_lengths(), [0], "failed secrets stay wiped");
    vm.set_answer(0, "replacement-secret".into()).unwrap();
    let retry = vm.response_command().expect("fresh input may retry");

    let debug = format!("{vm:?} {first:?} {retry:?}");
    assert!(!debug.contains("first-secret"));
    assert!(!debug.contains("replacement-secret"));
}

fn host_key(changed: bool) -> HostKeyPrompt {
    HostKeyPrompt {
        id: InteractionId::new(),
        host: "server.example.test".into(),
        port: 2222,
        algorithm: "ssh-ed25519".into(),
        sha256: "SHA256:abc".into(),
        changed,
    }
}
