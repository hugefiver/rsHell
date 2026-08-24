use std::fmt;

use rshell_core::{InteractionId, InteractionRequest, SessionId, UiCommand, UiPortError};

use crate::InteractionAction;

#[derive(Debug)]
pub struct InteractionDialogInit;

pub enum InteractionDialogMsg {
    Open {
        session: SessionId,
        request: InteractionRequest,
    },
    Answer(usize, String),
    Action(InteractionAction),
    ResponseAccepted(InteractionId),
    DismissSession(SessionId),
    OperationFailed(InteractionId, &'static str),
    CommandRejected(InteractionId, UiPortError),
}

impl fmt::Debug for InteractionDialogMsg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open { session, request } => formatter
                .debug_struct("Open")
                .field("session", session)
                .field("request", request)
                .finish(),
            Self::Answer(index, _) => formatter
                .debug_tuple("Answer")
                .field(index)
                .field(&"[REDACTED]")
                .finish(),
            Self::Action(action) => formatter.debug_tuple("Action").field(action).finish(),
            Self::ResponseAccepted(interaction) => formatter
                .debug_tuple("ResponseAccepted")
                .field(interaction)
                .finish(),
            Self::DismissSession(session) => formatter
                .debug_tuple("DismissSession")
                .field(session)
                .finish(),
            Self::OperationFailed(interaction, context) => formatter
                .debug_tuple("OperationFailed")
                .field(interaction)
                .field(context)
                .finish(),
            Self::CommandRejected(interaction, error) => formatter
                .debug_tuple("CommandRejected")
                .field(interaction)
                .field(error)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionDialogState {
    pub interaction: Option<InteractionId>,
    pub pending: bool,
    pub has_error: bool,
    pub revision: u64,
    pub prompt_count: usize,
    pub answered_prompts: Vec<usize>,
}

#[derive(Debug)]
pub enum InteractionDialogOutput {
    Command(Box<UiCommand>),
    CopyDiagnostics(String),
    Closed,
    StateChanged(InteractionDialogState),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_debug_redacts_the_secure_value() {
        let debug = format!(
            "{:?}",
            InteractionDialogMsg::Answer(0, "test-secret".into())
        );
        assert!(!debug.contains("test-secret"));
        assert!(debug.contains("[REDACTED]"));
    }
}
