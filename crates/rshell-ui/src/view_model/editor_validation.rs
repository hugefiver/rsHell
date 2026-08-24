use std::{fmt, path::PathBuf};

use rshell_core::{
    AuthenticationKind, ConnectionProfile, HostKeyPolicy, SettingsValidationError,
    validate_terminal_overrides,
};

use super::editor::{AuthenticationCapabilities, ConnectionEditorViewModel};

pub(super) fn validate_profile(
    view: &ConnectionEditorViewModel,
) -> Result<ConnectionProfile, EditorValidationError> {
    let name = view.name.trim();
    if name.is_empty() {
        return Err(EditorValidationError::MissingName);
    }
    let host = view.host.trim();
    if host.is_empty() || host.starts_with('-') {
        return Err(EditorValidationError::InvalidHost);
    }
    let port = view
        .port
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or(EditorValidationError::InvalidPort)?;
    if !AuthenticationCapabilities::for_transport(view.transport).allows(view.authentication) {
        return Err(EditorValidationError::UnsupportedAuthentication);
    }
    if view.authentication == AuthenticationKind::PublicKey && view.identity_file.trim().is_empty()
    {
        return Err(EditorValidationError::IdentityRequired);
    }
    validate_terminal_overrides(&view.terminal_overrides)
        .map_err(EditorValidationError::InvalidTerminalOverride)?;
    Ok(ConnectionProfile {
        id: view.id,
        group_id: view.group_id,
        name: name.into(),
        host: host.into(),
        port,
        username: view.username.trim().into(),
        transport: view.transport,
        authentication: view.authentication,
        credential_ref: None,
        identity_file: optional_path(&view.identity_file),
        host_key_policy: HostKeyPolicy::Strict,
        remote_command: optional_text(&view.remote_command),
        note: view.note.trim().into(),
        tags: view.tags.clone(),
        position: view.position,
        terminal_profile_id: view.terminal_profile_id,
        terminal_overrides: view.terminal_overrides.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorValidationError {
    MissingName,
    InvalidHost,
    InvalidPort,
    UnsupportedAuthentication,
    IdentityRequired,
    SecretRequired,
    InvalidTerminalOverride(SettingsValidationError),
}

impl fmt::Display for EditorValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingName => "connection name is required",
            Self::InvalidHost => "host is required and cannot begin with '-'",
            Self::InvalidPort => "port must be an integer from 1 to 65535",
            Self::UnsupportedAuthentication => "authentication is unsupported by this transport",
            Self::IdentityRequired => "an identity file is required for public-key authentication",
            Self::SecretRequired => "a secret is required for password authentication",
            Self::InvalidTerminalOverride(error) => return error.fmt(formatter),
        })
    }
}

impl std::error::Error for EditorValidationError {}

fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.into())
}

fn optional_path(value: &str) -> Option<PathBuf> {
    optional_text(value).map(PathBuf::from)
}
