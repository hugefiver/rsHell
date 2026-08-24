use rshell_core::{
    AppSettings, ColorScheme, KeyBinding, SettingsValidationCode, TerminalOverrides,
    TerminalProfile, UiCommand,
};
use rshell_ui::SettingsViewModel;

#[test]
fn connection_override_can_inherit_or_replace_each_global_terminal_field() {
    let overrides = TerminalOverrides {
        terminal_type: Some("screen-256color".into()),
        initial_cols: Some(132),
        initial_rows: Some(48),
        scrollback_lines: Some(20_000),
        font_family: Some("Cascadia Mono".into()),
        font_size: Some(14.0),
        color_scheme: Some(ColorScheme::OneDark),
        key_bindings: Some(Vec::<KeyBinding>::new()),
        left_alt_as_meta: Some(false),
        right_alt_as_meta: Some(false),
        enable_csi_u: Some(true),
        enable_kitty_keyboard: Some(true),
        mouse_reporting: Some(false),
        scroll_on_output: Some(false),
        scroll_on_keypress: Some(true),
        answerback: Some("terminal-ready".into()),
    };

    assert_eq!(overrides.inherited_field_count(), 0);
    assert_eq!(
        overrides.explicit_field_count(),
        TerminalOverrides::FIELD_COUNT
    );
    assert_eq!(overrides.clear_all().explicit_field_count(), 0);
}

#[test]
fn settings_stay_dirty_until_the_matching_authoritative_event_and_failure_preserves_draft() {
    let profile = TerminalProfile::default();
    let mut vm = SettingsViewModel::new(AppSettings::default(), vec![profile.clone()]);
    vm.active_profile_mut().unwrap().settings.initial_cols = 132;

    let command = vm.save_profile_command().expect("valid profile save");
    assert!(matches!(command, UiCommand::SaveTerminalProfile(_)));
    assert!(vm.pending());
    assert!(vm.profile_dirty());
    vm.failed("storage operation failed");
    assert!(!vm.pending());
    assert!(vm.profile_dirty());
    assert_eq!(vm.active_profile().unwrap().settings.initial_cols, 132);

    let mut accepted = profile;
    accepted.settings.initial_cols = 132;
    vm.save_profile_command().expect("retry save");
    vm.accept_profiles(vec![accepted]);
    assert!(!vm.pending());
    assert!(!vm.profile_dirty());
}

#[test]
fn settings_ui_rejects_blank_range_nonfinite_and_unknown_default_before_command() {
    let profile = TerminalProfile::default();
    let mut vm = SettingsViewModel::new(AppSettings::default(), vec![profile]);
    vm.active_profile_mut().unwrap().settings.initial_cols = 0;
    assert_eq!(
        vm.save_profile_command().unwrap_err().code,
        SettingsValidationCode::OutOfRange
    );
    vm.active_profile_mut().unwrap().settings.initial_cols = 80;
    vm.active_profile_mut().unwrap().settings.font_size = f32::NAN;
    assert_eq!(
        vm.save_profile_command().unwrap_err().code,
        SettingsValidationCode::NonFinite
    );
    vm.active_profile_mut().unwrap().settings.font_size = 14.0;
    vm.active_profile_mut().unwrap().settings.answerback = " ".into();
    assert_eq!(
        vm.save_profile_command().unwrap_err().code,
        SettingsValidationCode::Blank
    );
    vm.app_settings_mut().default_terminal_profile = rshell_core::TerminalProfileId::new();
    assert_eq!(
        vm.save_settings_command().unwrap_err().code,
        SettingsValidationCode::UnknownProfile
    );
}
