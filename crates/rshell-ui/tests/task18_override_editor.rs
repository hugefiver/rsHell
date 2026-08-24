use rshell_core::{
    CatalogMutation, ColorScheme, KeyBinding, KeyCode, KeyModifiers, TerminalOverrides,
    TerminalSettingsV1, UiCommand,
};
use rshell_ui::{ConnectionEditorDraft, TerminalOverrideKey};

const ALL_KEYS: [TerminalOverrideKey; TerminalOverrides::FIELD_COUNT] = [
    TerminalOverrideKey::TerminalType,
    TerminalOverrideKey::InitialCols,
    TerminalOverrideKey::InitialRows,
    TerminalOverrideKey::ScrollbackLines,
    TerminalOverrideKey::FontFamily,
    TerminalOverrideKey::FontSize,
    TerminalOverrideKey::ColorScheme,
    TerminalOverrideKey::KeyBindings,
    TerminalOverrideKey::LeftAltAsMeta,
    TerminalOverrideKey::RightAltAsMeta,
    TerminalOverrideKey::EnableCsiU,
    TerminalOverrideKey::EnableKittyKeyboard,
    TerminalOverrideKey::MouseReporting,
    TerminalOverrideKey::ScrollOnOutput,
    TerminalOverrideKey::ScrollOnKeypress,
    TerminalOverrideKey::Answerback,
];

#[test]
fn every_connection_override_has_explicit_inherit_semantics_and_saves_exactly() {
    let base = distinctive_base();
    let mut draft = valid_draft();

    for key in ALL_KEYS {
        assert!(draft.is_inherited(key));
        draft.set_explicit_from_base(key, &base);
        assert!(!draft.is_inherited(key));
    }
    assert_eq!(draft.view().terminal_overrides, overrides_from_base(&base));

    for key in ALL_KEYS {
        draft.set_inherited(key);
        assert!(draft.is_inherited(key));
        assert_eq!(
            draft.view().terminal_overrides.explicit_field_count(),
            TerminalOverrides::FIELD_COUNT - 1
        );
        draft.set_explicit_from_base(key, &base);
    }

    let command = draft.save_command().expect("all explicit values are valid");
    let UiCommand::ApplyCatalog { mutation, .. } = command else {
        panic!("connection editor must emit ApplyCatalog");
    };
    let CatalogMutation::Create(profile) = mutation else {
        panic!("new connection must emit Create");
    };
    assert_eq!(profile.terminal_overrides, overrides_from_base(&base));

    draft.clear_all_overrides();
    assert_eq!(
        draft.view().terminal_overrides,
        TerminalOverrides::default()
    );
}

#[test]
fn explicit_initialization_uses_the_current_selected_profile_base() {
    let mut first = distinctive_base();
    first.terminal_type = "profile-one".into();
    let mut second = distinctive_base();
    second.terminal_type = "profile-two".into();
    second.initial_cols = 211;

    let mut draft = valid_draft();
    draft.set_explicit_from_base(TerminalOverrideKey::TerminalType, &first);
    draft.set_inherited(TerminalOverrideKey::TerminalType);
    draft.set_explicit_from_base(TerminalOverrideKey::TerminalType, &second);
    draft.set_explicit_from_base(TerminalOverrideKey::InitialCols, &second);

    assert_eq!(
        draft.view().terminal_overrides.terminal_type.as_deref(),
        Some("profile-two")
    );
    assert_eq!(draft.view().terminal_overrides.initial_cols, Some(211));
}

fn valid_draft() -> ConnectionEditorDraft {
    let mut draft = ConnectionEditorDraft::create(None);
    draft.view_mut().name = "Override host".into();
    draft.view_mut().host = "override.example.test".into();
    draft
}

fn distinctive_base() -> TerminalSettingsV1 {
    TerminalSettingsV1 {
        terminal_type: "xterm-task18".into(),
        initial_cols: 173,
        initial_rows: 47,
        scrollback_lines: 12_345,
        font_family: "Cascadia Code".into(),
        font_size: 17.5,
        color_scheme: ColorScheme::TokyoNight,
        key_bindings: vec![KeyBinding {
            code: KeyCode::Character('k'),
            modifiers: KeyModifiers {
                control: true,
                ..Default::default()
            },
            action: "clear_scrollback".into(),
        }],
        left_alt_as_meta: false,
        right_alt_as_meta: true,
        enable_csi_u: true,
        enable_kitty_keyboard: true,
        mouse_reporting: false,
        scroll_on_output: false,
        scroll_on_keypress: true,
        answerback: "task18-answerback".into(),
        ..TerminalSettingsV1::default()
    }
}

fn overrides_from_base(base: &TerminalSettingsV1) -> TerminalOverrides {
    TerminalOverrides {
        terminal_type: Some(base.terminal_type.clone()),
        initial_cols: Some(base.initial_cols),
        initial_rows: Some(base.initial_rows),
        scrollback_lines: Some(base.scrollback_lines),
        font_family: Some(base.font_family.clone()),
        font_size: Some(base.font_size),
        color_scheme: Some(base.color_scheme),
        key_bindings: Some(base.key_bindings.clone()),
        left_alt_as_meta: Some(base.left_alt_as_meta),
        right_alt_as_meta: Some(base.right_alt_as_meta),
        enable_csi_u: Some(base.enable_csi_u),
        enable_kitty_keyboard: Some(base.enable_kitty_keyboard),
        mouse_reporting: Some(base.mouse_reporting),
        scroll_on_output: Some(base.scroll_on_output),
        scroll_on_keypress: Some(base.scroll_on_keypress),
        answerback: Some(base.answerback.clone()),
    }
}
