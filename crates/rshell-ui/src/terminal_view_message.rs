use std::{fmt, sync::Arc};

use gtk::gdk;
use rshell_core::{RenderFrame, ResolvedTerminalProfile, SessionId, SessionUiEvent, UiCommand};

use crate::{FontMetrics, PointerEvent, TerminalViewError};

#[derive(Debug, Clone)]
pub struct TerminalViewInit {
    pub session: SessionId,
    pub profile: ResolvedTerminalProfile,
    pub metrics: FontMetrics,
}

pub enum TerminalViewMsg {
    ApplyFrame(Arc<RenderFrame>),
    Key {
        key: gdk::Key,
        state: gdk::ModifierType,
    },
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
            Self::Key { key, state } => formatter
                .debug_struct("Key")
                .field("key", key)
                .field("state", state)
                .finish(),
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
