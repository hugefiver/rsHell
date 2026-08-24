mod resolution;
mod settings;
mod validation;

pub use resolution::{ResolvedTerminalProfile, TerminalOverrides};
pub use settings::{
    AppSettings, ColorScheme, KeyBinding, KeyCode, KeyModifiers, TerminalProfile,
    TerminalSettingsV1, TerminalSettingsVersion,
};
pub use validation::{
    SettingsValidationCode, SettingsValidationError, validate_app_settings,
    validate_terminal_overrides, validate_terminal_profile, validate_terminal_settings,
};
