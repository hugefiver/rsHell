use std::fmt;

use rshell_core::{
    HostKeyDecision, InteractionId, InteractionRequest, InteractionResponse, SessionId, UiCommand,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionAction {
    Reject,
    AcceptAndStore,
    CopyDiagnostics,
    Close,
    Submit,
    Cancel,
}

#[derive(Default)]
struct SecureAnswer {
    bytes: Vec<u8>,
    provided: bool,
}

impl SecureAnswer {
    fn set(&mut self, value: String) {
        self.clear();
        self.bytes = value.into_bytes();
        self.provided = true;
    }

    fn take_string(&mut self) -> String {
        let bytes = std::mem::take(&mut self.bytes);
        self.provided = false;
        String::from_utf8(bytes).expect("interaction input originated as UTF-8")
    }

    fn clear(&mut self) {
        self.bytes.fill(0);
        self.bytes.clear();
        self.provided = false;
    }
}

impl Drop for SecureAnswer {
    fn drop(&mut self) {
        self.clear();
    }
}

pub struct InteractionViewModel {
    session: SessionId,
    request: InteractionRequest,
    endpoint: Option<String>,
    fingerprint: Option<String>,
    answers: Vec<SecureAnswer>,
    handed_off: bool,
}

impl InteractionViewModel {
    pub fn new(session: SessionId, request: InteractionRequest) -> Self {
        let endpoint = match &request {
            InteractionRequest::HostKey(prompt) => Some(format!("{}:{}", prompt.host, prompt.port)),
            _ => None,
        };
        let fingerprint = match &request {
            InteractionRequest::HostKey(prompt) => {
                Some(format!("{} {}", prompt.algorithm, prompt.sha256))
            }
            _ => None,
        };
        let answer_count = match &request {
            InteractionRequest::KeyboardInteractive(prompt) => prompt.prompts.len(),
            InteractionRequest::Password(_) | InteractionRequest::PrivateKeyPassphrase(_) => 1,
            InteractionRequest::HostKey(_) => 0,
        };
        Self {
            session,
            request,
            endpoint,
            fingerprint,
            answers: (0..answer_count).map(|_| SecureAnswer::default()).collect(),
            handed_off: false,
        }
    }

    pub fn interaction_id(&self) -> InteractionId {
        match &self.request {
            InteractionRequest::HostKey(prompt) => prompt.id,
            InteractionRequest::Password(prompt)
            | InteractionRequest::PrivateKeyPassphrase(prompt) => prompt.id,
            InteractionRequest::KeyboardInteractive(prompt) => prompt.id,
        }
    }

    pub fn session_id(&self) -> SessionId {
        self.session
    }

    pub fn request(&self) -> &InteractionRequest {
        &self.request
    }

    pub fn actions(&self) -> &'static [InteractionAction] {
        match &self.request {
            InteractionRequest::HostKey(prompt) if prompt.changed => {
                &[InteractionAction::CopyDiagnostics, InteractionAction::Close]
            }
            InteractionRequest::HostKey(_) => {
                &[InteractionAction::Reject, InteractionAction::AcceptAndStore]
            }
            _ => &[InteractionAction::Cancel, InteractionAction::Submit],
        }
    }

    pub fn endpoint(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    pub fn fingerprint(&self) -> Option<&str> {
        self.fingerprint.as_deref()
    }

    pub fn set_answer(&mut self, index: usize, value: String) -> Result<(), &'static str> {
        let answer = self
            .answers
            .get_mut(index)
            .ok_or("unknown interaction prompt")?;
        if self.handed_off {
            return Err("interaction already completed");
        }
        answer.set(value);
        Ok(())
    }

    pub fn answer_lengths(&self) -> Vec<usize> {
        self.answers
            .iter()
            .map(|answer| answer.bytes.len())
            .collect()
    }

    pub(crate) fn prompt_count(&self) -> usize {
        self.answers.len()
    }

    pub(crate) fn answered_prompt_indices(&self) -> Vec<usize> {
        self.answers
            .iter()
            .enumerate()
            .filter_map(|(index, answer)| answer.provided.then_some(index))
            .collect()
    }

    pub fn response_command(&mut self) -> Option<UiCommand> {
        if self.handed_off || matches!(self.request, InteractionRequest::HostKey(_)) {
            return None;
        }
        let response = match &self.request {
            InteractionRequest::Password(_) | InteractionRequest::PrivateKeyPassphrase(_) => {
                InteractionResponse::Secret(self.answers.first_mut()?.take_string().into())
            }
            InteractionRequest::KeyboardInteractive(_) => InteractionResponse::Answers(
                self.answers
                    .iter_mut()
                    .map(|answer| answer.take_string().into())
                    .collect(),
            ),
            InteractionRequest::HostKey(_) => return None,
        };
        self.finish(response)
    }

    pub fn action_command(&mut self, action: InteractionAction) -> Option<UiCommand> {
        if self.handed_off {
            return None;
        }
        let response = match action {
            InteractionAction::AcceptAndStore => {
                InteractionResponse::HostKey(HostKeyDecision::AcceptAndStore)
            }
            InteractionAction::Reject => InteractionResponse::HostKey(HostKeyDecision::Reject),
            InteractionAction::Cancel | InteractionAction::Close => self.cancel_response(),
            InteractionAction::Submit => return self.response_command(),
            InteractionAction::CopyDiagnostics => return None,
        };
        self.finish(response)
    }

    pub fn cancel_command(&mut self) -> Option<UiCommand> {
        self.action_command(InteractionAction::Cancel)
    }

    pub fn submission_failed(&mut self) {
        for answer in &mut self.answers {
            answer.clear();
        }
        self.handed_off = false;
    }

    pub fn is_handed_off(&self) -> bool {
        self.handed_off
    }

    fn cancel_response(&self) -> InteractionResponse {
        if matches!(self.request, InteractionRequest::HostKey(_)) {
            InteractionResponse::HostKey(HostKeyDecision::Reject)
        } else {
            InteractionResponse::Cancel
        }
    }

    fn finish(&mut self, response: InteractionResponse) -> Option<UiCommand> {
        self.handed_off = true;
        for answer in &mut self.answers {
            answer.clear();
        }
        Some(UiCommand::Respond {
            session: self.session,
            interaction: self.interaction_id(),
            response,
        })
    }
}

impl fmt::Debug for InteractionViewModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InteractionViewModel")
            .field("session", &self.session)
            .field("interaction", &self.interaction_id())
            .field("answers", &"[REDACTED]")
            .field("handed_off", &self.handed_off)
            .finish()
    }
}
