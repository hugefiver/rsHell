use std::{error::Error, fmt};

use gtk::gdk::{Key, ModifierType};
use rshell_core::{KeyCode, KeyModifiers, TerminalInput};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetrics {
    pub cell_width: f64,
    pub cell_height: f64,
}

impl FontMetrics {
    pub fn new(cell_width: f64, cell_height: f64) -> Result<Self, TerminalViewError> {
        if !positive_finite(cell_width) || !positive_finite(cell_height) {
            return Err(TerminalViewError::InvalidFontMetrics);
        }
        Ok(Self {
            cell_width,
            cell_height,
        })
    }
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            cell_width: 9.0,
            cell_height: 18.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalViewError {
    InvalidFontMetrics,
    InvalidAllocation,
    InvalidScale,
    GeometryOverflow,
    OutOfBounds,
    InvalidText,
    ClipboardUnavailable,
    DrawingFailed,
}

impl fmt::Display for TerminalViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFontMetrics => "terminal font metrics must be positive and finite",
            Self::InvalidAllocation => "terminal allocation must be positive",
            Self::InvalidScale => "terminal scale must be positive and finite",
            Self::GeometryOverflow => "terminal geometry exceeds protocol limits",
            Self::OutOfBounds => "terminal pointer coordinate is out of bounds",
            Self::InvalidText => "terminal text failed clipboard policy",
            Self::ClipboardUnavailable => "terminal clipboard is unavailable",
            Self::DrawingFailed => "terminal frame drawing failed",
        })
    }
}

impl Error for TerminalViewError {}

pub fn map_gdk_key(key: Key, state: ModifierType) -> Option<TerminalInput> {
    let code = match key {
        Key::Return | Key::KP_Enter => KeyCode::Enter,
        Key::Escape => KeyCode::Escape,
        Key::Tab | Key::ISO_Left_Tab => KeyCode::Tab,
        Key::BackSpace => KeyCode::Backspace,
        Key::Delete | Key::KP_Delete => KeyCode::Delete,
        Key::Insert | Key::KP_Insert => KeyCode::Insert,
        Key::Home | Key::KP_Home => KeyCode::Home,
        Key::End | Key::KP_End => KeyCode::End,
        Key::Page_Up | Key::KP_Page_Up => KeyCode::PageUp,
        Key::Page_Down | Key::KP_Page_Down => KeyCode::PageDown,
        Key::Up | Key::KP_Up => KeyCode::ArrowUp,
        Key::Down | Key::KP_Down => KeyCode::ArrowDown,
        Key::Left | Key::KP_Left => KeyCode::ArrowLeft,
        Key::Right | Key::KP_Right => KeyCode::ArrowRight,
        Key::F1 => KeyCode::F(1),
        Key::F2 => KeyCode::F(2),
        Key::F3 => KeyCode::F(3),
        Key::F4 => KeyCode::F(4),
        Key::F5 => KeyCode::F(5),
        Key::F6 => KeyCode::F(6),
        Key::F7 => KeyCode::F(7),
        Key::F8 => KeyCode::F(8),
        Key::F9 => KeyCode::F(9),
        Key::F10 => KeyCode::F(10),
        Key::F11 => KeyCode::F(11),
        Key::F12 => KeyCode::F(12),
        _ => KeyCode::Character(key.to_unicode()?),
    };
    Some(TerminalInput::Key {
        code,
        modifiers: modifiers(state),
    })
}

pub(crate) fn modifiers(state: ModifierType) -> KeyModifiers {
    KeyModifiers {
        shift: state.contains(ModifierType::SHIFT_MASK),
        control: state.contains(ModifierType::CONTROL_MASK),
        alt: state.contains(ModifierType::ALT_MASK),
        super_key: state.contains(ModifierType::SUPER_MASK),
    }
}

pub(crate) fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}
