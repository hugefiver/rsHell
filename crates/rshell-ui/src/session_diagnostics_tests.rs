use rshell_core::{SessionFailure, SessionUiEvent};

use crate::session_diagnostics::failure_line;

#[test]
fn qa_session_diagnostics_are_closed_and_redacted() {
    let cases = [
        (SessionFailure::Validation, "validation"),
        (SessionFailure::Storage, "storage"),
        (SessionFailure::Vault, "vault"),
        (SessionFailure::HostKeyRejected, "host_key_rejected"),
        (SessionFailure::HostKeyChanged, "host_key_changed"),
        (SessionFailure::Authentication, "authentication"),
        (SessionFailure::Network, "network"),
        (SessionFailure::Pty, "pty"),
        (SessionFailure::SshChannel, "ssh_channel"),
        (SessionFailure::Subprocess, "subprocess"),
        (SessionFailure::Platform, "platform"),
        (SessionFailure::Backpressure, "backpressure"),
        (SessionFailure::Timeout, "timeout"),
        (SessionFailure::Crashed, "crashed"),
    ];
    for (failure, code) in cases {
        assert_eq!(
            failure_line(&SessionUiEvent::Failed(failure)),
            Some(format!("P0_SESSION state=failed code={code}"))
        );
    }
    assert_eq!(
        failure_line(&SessionUiEvent::Crashed("redacted".into())),
        Some("P0_SESSION state=crashed code=crashed".into())
    );
    assert_eq!(
        failure_line(&SessionUiEvent::State(rshell_core::SessionState::Connected)),
        None
    );
}
