use std::{fmt, sync::Arc};

use gtk::gdk;
use rshell_core::{
    PaneId, RenderFrame, ResolvedTerminalProfile, SessionId, SessionUiEvent, UiCommand,
};

use crate::{FontMetricEnvironment, MeasuredFontMetrics, PointerEvent, TerminalViewError};

#[derive(Debug, Clone)]
pub struct TerminalViewInit {
    pub pane: PaneId,
    pub session: SessionId,
    pub profile: ResolvedTerminalProfile,
    pub metrics: MeasuredFontMetrics,
}

pub enum TerminalViewMsg {
    ApplyFrame(Arc<RenderFrame>),
    RefreshMetrics(FontMetricEnvironment),
    UpdateProfile(ResolvedTerminalProfile),
    Key {
        key: gdk::Key,
        state: gdk::ModifierType,
    },
    KeyReleased(gdk::Key),
    FocusLost,
    CommittedText(String),
    Pointer(PointerEvent),
    Resize {
        width: i32,
        height: i32,
        scale: f64,
    },
    Selection {
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        rectangular: bool,
    },
    Search {
        text: String,
        case_sensitive: bool,
        regex: bool,
    },
    PasteText(String),
    ReadClipboard,
    Copy,
    SessionEvent(SessionUiEvent),
}

impl fmt::Debug for TerminalViewMsg {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApplyFrame(frame) => formatter
                .debug_tuple("ApplyFrame")
                .field(&frame.generation)
                .finish(),
            Self::RefreshMetrics(environment) => formatter
                .debug_tuple("RefreshMetrics")
                .field(environment)
                .finish(),
            Self::UpdateProfile(_) => formatter.write_str("UpdateProfile(..)"),
            Self::Key { key, state } => formatter
                .debug_struct("Key")
                .field("key", key)
                .field("state", state)
                .finish(),
            Self::KeyReleased(key) => formatter.debug_tuple("KeyReleased").field(key).finish(),
            Self::FocusLost => formatter.write_str("FocusLost"),
            Self::CommittedText(_) => formatter.write_str("CommittedText([REDACTED])"),
            Self::Pointer(event) => formatter.debug_tuple("Pointer").field(event).finish(),
            Self::Resize {
                width,
                height,
                scale,
            } => formatter
                .debug_struct("Resize")
                .field("width", width)
                .field("height", height)
                .field("scale", scale)
                .finish(),
            Self::Selection { .. } => formatter.write_str("Selection(..)"),
            Self::Search { .. } => formatter.write_str("Search(..)"),
            Self::PasteText(_) => formatter.write_str("PasteText([REDACTED])"),
            Self::ReadClipboard => formatter.write_str("ReadClipboard"),
            Self::Copy => formatter.write_str("Copy"),
            Self::SessionEvent(_) => formatter.write_str("SessionEvent(..)"),
        }
    }
}

#[derive(Debug)]
pub enum TerminalViewOutput {
    Command(Box<UiCommand>),
    Error(TerminalViewError),
    ClipboardWritten { bytes: usize },
}
