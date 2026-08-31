use std::{fmt, path::PathBuf};

use rshell_core::{AuthenticationKind, ImportSourceKind, TransportKind};

use crate::{EditorTextField, ShellLayoutMode, SmokeActionKind, SmokeVisualCheckpoint};

#[derive(Clone)]
pub enum SmokeAction {
    WaitWindowRealized,
    NewTab,
    OpenConnectionEditor,
    SetConnectionField(SmokeConnectionField),
    SubmitConnection,
    SelectConnection(String),
    Connect,
    RespondHostKey {
        accept: bool,
    },
    RespondAuth {
        prompt: usize,
        env_var: String,
    },
    SendTerminalText {
        text: String,
        expected_color_marker: Option<String>,
    },
    PasteTextFromEnv {
        env_var: String,
        effect_marker: String,
    },
    ResizeTerminal {
        width: i32,
        height: i32,
        scale: f64,
    },
    WaitFrameContains(String),
    SplitHorizontal,
    SplitVertical,
    SwitchTab(usize),
    SearchTerminal {
        text: String,
        case_sensitive: bool,
        regex: bool,
    },
    SelectRange {
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        rectangular: bool,
        expected_text: Option<String>,
        expect_wide_midpoint: bool,
    },
    CopySelection,
    Reconnect,
    VisualCheckpoint(SmokeVisualCheckpoint),
    PreviewImport {
        source: ImportSourceKind,
        path: PathBuf,
        expected: Option<SmokeImportExpectation>,
    },
    CommitImport,
    CancelImport,
    CloseAll,
    InterruptTerminal,
    ResetDisplay,
    ResizeWindow {
        width: i32,
        height: i32,
        expected_mode: ShellLayoutMode,
    },
}

#[derive(Clone, Debug)]
pub struct SmokeImportExpectation {
    pub groups: usize,
    pub connections: usize,
    pub group_name: String,
    pub connection_name: String,
    pub host: String,
    pub authentication: AuthenticationKind,
    pub credential_reference_present: bool,
    pub terminal_override_present: bool,
    pub importable: bool,
    pub wildcard: bool,
}

impl SmokeAction {
    pub const ALL: [SmokeActionKind; 28] = [
        SmokeActionKind::WaitWindowRealized,
        SmokeActionKind::NewTab,
        SmokeActionKind::OpenConnectionEditor,
        SmokeActionKind::SetConnectionField,
        SmokeActionKind::SubmitConnection,
        SmokeActionKind::SelectConnection,
        SmokeActionKind::Connect,
        SmokeActionKind::RespondHostKey,
        SmokeActionKind::RespondAuth,
        SmokeActionKind::SendTerminalText,
        SmokeActionKind::PasteTextFromEnv,
        SmokeActionKind::ResizeTerminal,
        SmokeActionKind::WaitFrameContains,
        SmokeActionKind::SplitHorizontal,
        SmokeActionKind::SplitVertical,
        SmokeActionKind::SwitchTab,
        SmokeActionKind::SearchTerminal,
        SmokeActionKind::SelectRange,
        SmokeActionKind::CopySelection,
        SmokeActionKind::Reconnect,
        SmokeActionKind::VisualCheckpoint,
        SmokeActionKind::PreviewImport,
        SmokeActionKind::CommitImport,
        SmokeActionKind::CancelImport,
        SmokeActionKind::CloseAll,
        SmokeActionKind::InterruptTerminal,
        SmokeActionKind::ResetDisplay,
        SmokeActionKind::ResizeWindow,
    ];

    pub fn kind(&self) -> SmokeActionKind {
        match self {
            Self::WaitWindowRealized => SmokeActionKind::WaitWindowRealized,
            Self::NewTab => SmokeActionKind::NewTab,
            Self::OpenConnectionEditor => SmokeActionKind::OpenConnectionEditor,
            Self::SetConnectionField(_) => SmokeActionKind::SetConnectionField,
            Self::SubmitConnection => SmokeActionKind::SubmitConnection,
            Self::SelectConnection(_) => SmokeActionKind::SelectConnection,
            Self::Connect => SmokeActionKind::Connect,
            Self::RespondHostKey { .. } => SmokeActionKind::RespondHostKey,
            Self::RespondAuth { .. } => SmokeActionKind::RespondAuth,
            Self::SendTerminalText { .. } => SmokeActionKind::SendTerminalText,
            Self::PasteTextFromEnv { .. } => SmokeActionKind::PasteTextFromEnv,
            Self::ResizeTerminal { .. } => SmokeActionKind::ResizeTerminal,
            Self::WaitFrameContains(_) => SmokeActionKind::WaitFrameContains,
            Self::SplitHorizontal => SmokeActionKind::SplitHorizontal,
            Self::SplitVertical => SmokeActionKind::SplitVertical,
            Self::SwitchTab(_) => SmokeActionKind::SwitchTab,
            Self::SearchTerminal { .. } => SmokeActionKind::SearchTerminal,
            Self::SelectRange { .. } => SmokeActionKind::SelectRange,
            Self::CopySelection => SmokeActionKind::CopySelection,
            Self::Reconnect => SmokeActionKind::Reconnect,
            Self::VisualCheckpoint(_) => SmokeActionKind::VisualCheckpoint,
            Self::PreviewImport { .. } => SmokeActionKind::PreviewImport,
            Self::CommitImport => SmokeActionKind::CommitImport,
            Self::CancelImport => SmokeActionKind::CancelImport,
            Self::CloseAll => SmokeActionKind::CloseAll,
            Self::InterruptTerminal => SmokeActionKind::InterruptTerminal,
            Self::ResetDisplay => SmokeActionKind::ResetDisplay,
            Self::ResizeWindow { .. } => SmokeActionKind::ResizeWindow,
        }
    }
}

impl fmt::Debug for SmokeAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple(self.kind().as_str())
            .field(&"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub enum SmokeConnectionField {
    Text {
        field: EditorTextField,
        value: String,
    },
    Port(u16),
    Transport(TransportKind),
    Authentication(AuthenticationKind),
    SecretFromEnv {
        env_var: String,
    },
}

impl fmt::Debug for SmokeConnectionField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Text { .. } => "Text",
            Self::Port(_) => "Port",
            Self::Transport(_) => "Transport",
            Self::Authentication(_) => "Authentication",
            Self::SecretFromEnv { .. } => "SecretFromEnv",
        };
        formatter.debug_tuple(name).field(&"[REDACTED]").finish()
    }
}
