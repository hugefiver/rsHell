use std::{collections::BTreeSet, fmt};

use rshell_core::{
    AuthenticationKind, ConnectionId, ConnectionProfile, GroupId, TerminalProfile, TransportKind,
    UiCommand, UiPortError,
};

use crate::TerminalOverrideKey;

#[derive(Debug, Clone)]
pub struct ConnectionEditorInit {
    pub terminal_profiles: Vec<TerminalProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorTextField {
    Name,
    Host,
    Username,
    IdentityFile,
    RemoteCommand,
    Note,
    Tags,
}

pub enum ConnectionEditorMsg {
    OpenCreate(Option<GroupId>),
    OpenEdit(Box<ConnectionProfile>),
    SetTerminalProfiles(Vec<TerminalProfile>),
    TextChanged(EditorTextField, String),
    PortChanged(u16),
    TransportChanged(u32),
    AuthenticationChanged(AuthenticationKind),
    SecretChanged(String),
    ProfileChanged(u32),
    OverrideInheritance(TerminalOverrideKey, bool),
    OverrideText(TerminalOverrideKey, String),
    OverrideNumber(TerminalOverrideKey, f64),
    OverrideScheme(u32),
    OverrideBool(TerminalOverrideKey, bool),
    OverrideBindings(String),
    ClearOverrides,
    Save,
    Cancel,
    CommandAccepted,
    CommandRejected(UiPortError),
    OperationFailed(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEditorState {
    pub open: bool,
    pub pending: bool,
    pub has_error: bool,
    pub revision: u64,
    pub draft: Option<ConnectionEditorDraftState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEditorDraftState {
    pub id: ConnectionId,
    pub is_new: bool,
    pub name: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub transport: TransportKind,
    pub authentication: AuthenticationKind,
    pub identity_file: String,
    pub remote_command: String,
    pub note: String,
    pub tags: BTreeSet<String>,
    pub secret_changed: bool,
    pub secret_present: bool,
}

impl fmt::Debug for ConnectionEditorMsg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenCreate(group) => formatter.debug_tuple("OpenCreate").field(group).finish(),
            Self::OpenEdit(profile) => formatter.debug_tuple("OpenEdit").field(profile).finish(),
            Self::SetTerminalProfiles(profiles) => formatter
                .debug_tuple("SetTerminalProfiles")
                .field(profiles)
                .finish(),
            Self::TextChanged(field, value) => formatter
                .debug_tuple("TextChanged")
                .field(field)
                .field(value)
                .finish(),
            Self::PortChanged(port) => formatter.debug_tuple("PortChanged").field(port).finish(),
            Self::TransportChanged(index) => formatter
                .debug_tuple("TransportChanged")
                .field(index)
                .finish(),
            Self::AuthenticationChanged(authentication) => formatter
                .debug_tuple("AuthenticationChanged")
                .field(authentication)
                .finish(),
            Self::SecretChanged(_) => formatter.write_str("SecretChanged([REDACTED])"),
            Self::ProfileChanged(index) => formatter
                .debug_tuple("ProfileChanged")
                .field(index)
                .finish(),
            Self::OverrideInheritance(field, inherited) => formatter
                .debug_tuple("OverrideInheritance")
                .field(field)
                .field(inherited)
                .finish(),
            Self::OverrideText(field, _) => formatter
                .debug_tuple("OverrideText")
                .field(field)
                .field(&"[TEXT]")
                .finish(),
            Self::OverrideNumber(field, value) => formatter
                .debug_tuple("OverrideNumber")
                .field(field)
                .field(value)
                .finish(),
            Self::OverrideScheme(index) => formatter
                .debug_tuple("OverrideScheme")
                .field(index)
                .finish(),
            Self::OverrideBool(field, value) => formatter
                .debug_tuple("OverrideBool")
                .field(field)
                .field(value)
                .finish(),
            Self::OverrideBindings(_) => formatter.write_str("OverrideBindings([TEXT])"),
            Self::ClearOverrides => formatter.write_str("ClearOverrides"),
            Self::Save => formatter.write_str("Save"),
            Self::Cancel => formatter.write_str("Cancel"),
            Self::CommandAccepted => formatter.write_str("CommandAccepted"),
            Self::CommandRejected(error) => formatter
                .debug_tuple("CommandRejected")
                .field(error)
                .finish(),
            Self::OperationFailed(context) => formatter
                .debug_tuple("OperationFailed")
                .field(context)
                .finish(),
        }
    }
}

#[derive(Debug)]
pub enum ConnectionEditorOutput {
    Command(Box<UiCommand>),
    Closed,
    StateChanged(Box<ConnectionEditorState>),
}
