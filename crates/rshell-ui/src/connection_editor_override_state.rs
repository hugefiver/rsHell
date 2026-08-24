use rshell_core::ColorScheme;

use crate::{ConnectionEditorViewModel, TerminalOverrideKey};

pub(crate) fn set_override_text(
    view: &mut ConnectionEditorViewModel,
    key: TerminalOverrideKey,
    value: String,
) {
    match key {
        TerminalOverrideKey::TerminalType => view.terminal_overrides.terminal_type = Some(value),
        TerminalOverrideKey::FontFamily => view.terminal_overrides.font_family = Some(value),
        TerminalOverrideKey::Answerback => view.terminal_overrides.answerback = Some(value),
        _ => {}
    }
}

pub(crate) fn set_override_number(
    view: &mut ConnectionEditorViewModel,
    key: TerminalOverrideKey,
    value: f64,
) {
    match key {
        TerminalOverrideKey::InitialCols => {
            view.terminal_overrides.initial_cols = Some(value as u16)
        }
        TerminalOverrideKey::InitialRows => {
            view.terminal_overrides.initial_rows = Some(value as u16)
        }
        TerminalOverrideKey::ScrollbackLines => {
            view.terminal_overrides.scrollback_lines = Some(value as usize)
        }
        TerminalOverrideKey::FontSize => view.terminal_overrides.font_size = Some(value as f32),
        _ => {}
    }
}

pub(crate) fn set_override_bool(
    view: &mut ConnectionEditorViewModel,
    key: TerminalOverrideKey,
    value: bool,
) {
    match key {
        TerminalOverrideKey::LeftAltAsMeta => {
            view.terminal_overrides.left_alt_as_meta = Some(value)
        }
        TerminalOverrideKey::RightAltAsMeta => {
            view.terminal_overrides.right_alt_as_meta = Some(value)
        }
        TerminalOverrideKey::EnableCsiU => view.terminal_overrides.enable_csi_u = Some(value),
        TerminalOverrideKey::EnableKittyKeyboard => {
            view.terminal_overrides.enable_kitty_keyboard = Some(value)
        }
        TerminalOverrideKey::MouseReporting => {
            view.terminal_overrides.mouse_reporting = Some(value)
        }
        TerminalOverrideKey::ScrollOnOutput => {
            view.terminal_overrides.scroll_on_output = Some(value)
        }
        TerminalOverrideKey::ScrollOnKeypress => {
            view.terminal_overrides.scroll_on_keypress = Some(value)
        }
        _ => {}
    }
}

pub(crate) fn set_override_scheme(view: &mut ConnectionEditorViewModel, index: u32) {
    view.terminal_overrides.color_scheme = Some(match index {
        1 => ColorScheme::OneDark,
        2 => ColorScheme::SolarizedDark,
        3 => ColorScheme::SolarizedLight,
        4 => ColorScheme::Dracula,
        5 => ColorScheme::Monokai,
        6 => ColorScheme::Nord,
        7 => ColorScheme::GruvboxDark,
        8 => ColorScheme::TokyoNight,
        9 => ColorScheme::CampbellPowershell,
        _ => ColorScheme::Default,
    });
}
