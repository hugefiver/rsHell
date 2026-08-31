use std::path::PathBuf;

use rshell_core::ImportSourceKind;
use rshell_ui::{ShellLayoutMode, SmokeAction, SmokeVisualCheckpoint, SmokeVisualState};
use serde::Deserialize;

use crate::{
    p0_smoke_action_fields::{RawConnectionField, RawImportExpectation},
    p0_smoke_scenario::ScenarioError,
};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RawAction {
    WaitWindowRealized,
    NewTab,
    OpenConnectionEditor,
    SetConnectionField {
        field: RawConnectionField,
    },
    SubmitConnection,
    SelectConnection {
        connection: String,
    },
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
        #[serde(default)]
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
    WaitFrameContains {
        text: String,
    },
    SplitHorizontal,
    SplitVertical,
    SwitchTab {
        tab: usize,
    },
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
        #[serde(default)]
        expected_text: Option<String>,
        #[serde(default)]
        expect_wide_midpoint: bool,
    },
    CopySelection,
    Reconnect,
    VisualCheckpoint {
        id: String,
        state: String,
        width: i32,
        height: i32,
        expected_mode: String,
    },
    PreviewImport {
        source: ImportSourceKind,
        path: PathBuf,
        #[serde(default)]
        expected: Option<RawImportExpectation>,
    },
    CommitImport,
    CancelImport,
    CloseAll,
    InterruptTerminal,
    ResetDisplay,
    ResizeWindow {
        width: i32,
        height: i32,
        expected_mode: String,
    },
}

impl RawAction {
    pub(crate) fn secret_environment_name(&self) -> Option<&str> {
        match self {
            Self::RespondAuth { env_var, .. } | Self::PasteTextFromEnv { env_var, .. } => {
                Some(env_var)
            }
            Self::SetConnectionField {
                field: RawConnectionField::SecretFromEnv { env_var },
            } => Some(env_var),
            _ => None,
        }
    }

    pub(crate) fn into_action(self) -> Result<SmokeAction, ScenarioError> {
        let action = match self {
            Self::WaitWindowRealized => SmokeAction::WaitWindowRealized,
            Self::NewTab => SmokeAction::NewTab,
            Self::OpenConnectionEditor => SmokeAction::OpenConnectionEditor,
            Self::SetConnectionField { field } => {
                SmokeAction::SetConnectionField(field.into_field()?)
            }
            Self::SubmitConnection => SmokeAction::SubmitConnection,
            Self::SelectConnection { connection } => SmokeAction::SelectConnection(connection),
            Self::Connect => SmokeAction::Connect,
            Self::RespondHostKey { accept } => SmokeAction::RespondHostKey { accept },
            Self::RespondAuth { prompt, env_var } => SmokeAction::RespondAuth {
                prompt,
                env_var: environment_name(env_var)?,
            },
            Self::SendTerminalText {
                text,
                expected_color_marker,
            } => SmokeAction::SendTerminalText {
                text,
                expected_color_marker,
            },
            Self::PasteTextFromEnv {
                env_var,
                effect_marker,
            } => SmokeAction::PasteTextFromEnv {
                env_var: environment_name(env_var)?,
                effect_marker,
            },
            Self::ResizeTerminal {
                width,
                height,
                scale,
            } => SmokeAction::ResizeTerminal {
                width,
                height,
                scale,
            },
            Self::WaitFrameContains { text } => SmokeAction::WaitFrameContains(text),
            Self::SplitHorizontal => SmokeAction::SplitHorizontal,
            Self::SplitVertical => SmokeAction::SplitVertical,
            Self::SwitchTab { tab } => SmokeAction::SwitchTab(tab),
            Self::SearchTerminal {
                text,
                case_sensitive,
                regex,
            } => SmokeAction::SearchTerminal {
                text,
                case_sensitive,
                regex,
            },
            Self::SelectRange {
                start_x,
                start_y,
                end_x,
                end_y,
                rectangular,
                expected_text,
                expect_wide_midpoint,
            } => SmokeAction::SelectRange {
                start_x,
                start_y,
                end_x,
                end_y,
                rectangular,
                expected_text,
                expect_wide_midpoint,
            },
            Self::CopySelection => SmokeAction::CopySelection,
            Self::Reconnect => SmokeAction::Reconnect,
            Self::VisualCheckpoint {
                id,
                state,
                width,
                height,
                expected_mode,
            } => SmokeAction::VisualCheckpoint(SmokeVisualCheckpoint {
                id,
                state: SmokeVisualState::parse(&state).ok_or(ScenarioError::Invalid)?,
                width,
                height,
                expected_mode: ShellLayoutMode::parse(&expected_mode)
                    .ok_or(ScenarioError::Invalid)?,
            }),
            Self::PreviewImport {
                source,
                path,
                expected,
            } => SmokeAction::PreviewImport {
                source,
                path,
                expected: expected.map(RawImportExpectation::into_expectation),
            },
            Self::CommitImport => SmokeAction::CommitImport,
            Self::CancelImport => SmokeAction::CancelImport,
            Self::CloseAll => SmokeAction::CloseAll,
            Self::InterruptTerminal => SmokeAction::InterruptTerminal,
            Self::ResetDisplay => SmokeAction::ResetDisplay,
            Self::ResizeWindow {
                width,
                height,
                expected_mode,
            } => SmokeAction::ResizeWindow {
                width,
                height,
                expected_mode: ShellLayoutMode::parse(&expected_mode)
                    .ok_or(ScenarioError::Invalid)?,
            },
        };
        Ok(action)
    }
}

pub(crate) fn environment_name(value: String) -> Result<String, ScenarioError> {
    let valid = !value.is_empty()
        && value.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphanumeric() && (index > 0 || !byte.is_ascii_digit())
        });
    valid.then_some(value).ok_or(ScenarioError::Invalid)
}
