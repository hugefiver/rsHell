use rshell_core::{TerminalOverrides, TerminalSettingsV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalOverrideKey {
    TerminalType,
    InitialCols,
    InitialRows,
    ScrollbackLines,
    FontFamily,
    FontSize,
    ColorScheme,
    KeyBindings,
    LeftAltAsMeta,
    RightAltAsMeta,
    EnableCsiU,
    EnableKittyKeyboard,
    MouseReporting,
    ScrollOnOutput,
    ScrollOnKeypress,
    Answerback,
}

impl TerminalOverrideKey {
    pub(crate) fn is_inherited(self, values: &TerminalOverrides) -> bool {
        match self {
            Self::TerminalType => values.terminal_type.is_none(),
            Self::InitialCols => values.initial_cols.is_none(),
            Self::InitialRows => values.initial_rows.is_none(),
            Self::ScrollbackLines => values.scrollback_lines.is_none(),
            Self::FontFamily => values.font_family.is_none(),
            Self::FontSize => values.font_size.is_none(),
            Self::ColorScheme => values.color_scheme.is_none(),
            Self::KeyBindings => values.key_bindings.is_none(),
            Self::LeftAltAsMeta => values.left_alt_as_meta.is_none(),
            Self::RightAltAsMeta => values.right_alt_as_meta.is_none(),
            Self::EnableCsiU => values.enable_csi_u.is_none(),
            Self::EnableKittyKeyboard => values.enable_kitty_keyboard.is_none(),
            Self::MouseReporting => values.mouse_reporting.is_none(),
            Self::ScrollOnOutput => values.scroll_on_output.is_none(),
            Self::ScrollOnKeypress => values.scroll_on_keypress.is_none(),
            Self::Answerback => values.answerback.is_none(),
        }
    }

    pub(crate) fn inherit(self, values: &mut TerminalOverrides) {
        match self {
            Self::TerminalType => values.terminal_type = None,
            Self::InitialCols => values.initial_cols = None,
            Self::InitialRows => values.initial_rows = None,
            Self::ScrollbackLines => values.scrollback_lines = None,
            Self::FontFamily => values.font_family = None,
            Self::FontSize => values.font_size = None,
            Self::ColorScheme => values.color_scheme = None,
            Self::KeyBindings => values.key_bindings = None,
            Self::LeftAltAsMeta => values.left_alt_as_meta = None,
            Self::RightAltAsMeta => values.right_alt_as_meta = None,
            Self::EnableCsiU => values.enable_csi_u = None,
            Self::EnableKittyKeyboard => values.enable_kitty_keyboard = None,
            Self::MouseReporting => values.mouse_reporting = None,
            Self::ScrollOnOutput => values.scroll_on_output = None,
            Self::ScrollOnKeypress => values.scroll_on_keypress = None,
            Self::Answerback => values.answerback = None,
        }
    }

    pub(crate) fn explicit_from(self, values: &mut TerminalOverrides, base: &TerminalSettingsV1) {
        match self {
            Self::TerminalType => values.terminal_type = Some(base.terminal_type.clone()),
            Self::InitialCols => values.initial_cols = Some(base.initial_cols),
            Self::InitialRows => values.initial_rows = Some(base.initial_rows),
            Self::ScrollbackLines => values.scrollback_lines = Some(base.scrollback_lines),
            Self::FontFamily => values.font_family = Some(base.font_family.clone()),
            Self::FontSize => values.font_size = Some(base.font_size),
            Self::ColorScheme => values.color_scheme = Some(base.color_scheme),
            Self::KeyBindings => values.key_bindings = Some(base.key_bindings.clone()),
            Self::LeftAltAsMeta => values.left_alt_as_meta = Some(base.left_alt_as_meta),
            Self::RightAltAsMeta => values.right_alt_as_meta = Some(base.right_alt_as_meta),
            Self::EnableCsiU => values.enable_csi_u = Some(base.enable_csi_u),
            Self::EnableKittyKeyboard => {
                values.enable_kitty_keyboard = Some(base.enable_kitty_keyboard)
            }
            Self::MouseReporting => values.mouse_reporting = Some(base.mouse_reporting),
            Self::ScrollOnOutput => values.scroll_on_output = Some(base.scroll_on_output),
            Self::ScrollOnKeypress => values.scroll_on_keypress = Some(base.scroll_on_keypress),
            Self::Answerback => values.answerback = Some(base.answerback.clone()),
        }
    }
}
