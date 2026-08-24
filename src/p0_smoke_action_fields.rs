use rshell_core::{AuthenticationKind, TransportKind};
use rshell_ui::{EditorTextField, SmokeConnectionField, SmokeImportExpectation};
use serde::Deserialize;

use crate::{p0_smoke_actions::environment_name, p0_smoke_scenario::ScenarioError};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawImportExpectation {
    groups: usize,
    connections: usize,
    group_name: String,
    connection_name: String,
    host: String,
    authentication: AuthenticationKind,
    credential_reference_present: bool,
    terminal_override_present: bool,
    importable: bool,
    wildcard: bool,
}

impl RawImportExpectation {
    pub(crate) fn into_expectation(self) -> SmokeImportExpectation {
        SmokeImportExpectation {
            groups: self.groups,
            connections: self.connections,
            group_name: self.group_name,
            connection_name: self.connection_name,
            host: self.host,
            authentication: self.authentication,
            credential_reference_present: self.credential_reference_present,
            terminal_override_present: self.terminal_override_present,
            importable: self.importable,
            wildcard: self.wildcard,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RawConnectionField {
    Text { field: RawTextField, value: String },
    Port { port: u16 },
    Transport { transport: TransportKind },
    Authentication { authentication: AuthenticationKind },
    SecretFromEnv { env_var: String },
}

impl RawConnectionField {
    pub(crate) fn into_field(self) -> Result<SmokeConnectionField, ScenarioError> {
        Ok(match self {
            Self::Text { field, value } => SmokeConnectionField::Text {
                field: field.into_field(),
                value,
            },
            Self::Port { port } => SmokeConnectionField::Port(port),
            Self::Transport { transport } => SmokeConnectionField::Transport(transport),
            Self::Authentication { authentication } => {
                SmokeConnectionField::Authentication(authentication)
            }
            Self::SecretFromEnv { env_var } => SmokeConnectionField::SecretFromEnv {
                env_var: environment_name(env_var)?,
            },
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RawTextField {
    Name,
    Host,
    Username,
    IdentityFile,
    RemoteCommand,
    Note,
    Tags,
}

impl RawTextField {
    const fn into_field(self) -> EditorTextField {
        match self {
            Self::Name => EditorTextField::Name,
            Self::Host => EditorTextField::Host,
            Self::Username => EditorTextField::Username,
            Self::IdentityFile => EditorTextField::IdentityFile,
            Self::RemoteCommand => EditorTextField::RemoteCommand,
            Self::Note => EditorTextField::Note,
            Self::Tags => EditorTextField::Tags,
        }
    }
}
