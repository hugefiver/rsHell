use rshell_core::UiPortError;

use crate::{ConnectionEditor, EditorValidationError};

#[derive(Clone, Copy)]
pub(crate) enum EditorRejection {
    OverrideInput,
    Validation(EditorValidationError),
    CommandPort(UiPortError),
    Operation,
}

pub(crate) fn rejection_line(rejection: EditorRejection) -> String {
    let (source, code) = match rejection {
        EditorRejection::OverrideInput => ("validation", "terminal_override_input"),
        EditorRejection::Validation(error) => ("validation", validation_code(error)),
        EditorRejection::CommandPort(UiPortError::Busy) => ("command_port", "busy"),
        EditorRejection::CommandPort(UiPortError::Closed) => ("command_port", "closed"),
        EditorRejection::Operation => ("application", "operation_failed"),
    };
    format!("P0_EDITOR source={source} code={code}")
}

impl ConnectionEditor {
    pub(crate) fn reject_override_input(&mut self, error: &'static str) {
        self.record_rejection(EditorRejection::OverrideInput, error.into());
    }

    pub(crate) fn reject_validation(&mut self, error: EditorValidationError) {
        self.record_rejection(EditorRejection::Validation(error), error.to_string());
    }

    pub(crate) fn reject_command_port(&mut self, error: UiPortError) {
        self.record_rejection(EditorRejection::CommandPort(error), error.to_string());
    }

    pub(crate) fn reject_operation(&mut self, context: &'static str) {
        self.record_rejection(EditorRejection::Operation, context.into());
    }

    fn record_rejection(&mut self, rejection: EditorRejection, display: String) {
        if std::env::var_os("RSHELL_QA_SMOKE").is_some() {
            eprintln!("{}", rejection_line(rejection));
        }
        self.error = Some(display);
    }
}

fn validation_code(error: EditorValidationError) -> &'static str {
    match error {
        EditorValidationError::MissingName => "missing_name",
        EditorValidationError::InvalidHost => "invalid_host",
        EditorValidationError::InvalidPort => "invalid_port",
        EditorValidationError::UnsupportedAuthentication => "unsupported_authentication",
        EditorValidationError::IdentityRequired => "identity_required",
        EditorValidationError::SecretRequired => "secret_required",
        EditorValidationError::InvalidTerminalOverride(_) => "invalid_terminal_override",
    }
}
