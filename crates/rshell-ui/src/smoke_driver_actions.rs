use std::{fmt, path::PathBuf};

use rshell_core::{AuthenticationKind, ImportSourceKind, TransportKind};

use crate::{EditorTextField, SmokeActionKind};

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
    VisualCheckpoint,
    PreviewImport {
        source: ImportSourceKind,
        path: PathBuf,
        expected: Option<SmokeImportExpectation>,
    },
    CommitImport,
    CancelImport,
    CloseAll,
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
    pub const ALL: [SmokeActionKind; 25] = [
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
            Self::VisualCheckpoint => SmokeActionKind::VisualCheckpoint,
            Self::PreviewImport { .. } => SmokeActionKind::PreviewImport,
            Self::CommitImport => SmokeActionKind::CommitImport,
            Self::CancelImport => SmokeActionKind::CancelImport,
            Self::CloseAll => SmokeActionKind::CloseAll,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
