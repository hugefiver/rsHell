use gtk::gdk::{Key, ModifierType};
use rshell_core::{
    KeyCode, KeyModifiers, PaneId, SessionUiCommand, TerminalInput, TerminalOverrides,
    TerminalSettingsV1, UiCommand,
};
use rshell_ui::{FontMetrics, TerminalViewModel, TerminalViewMsg, map_gdk_key};

#[test]
fn maps_terminal_keys_and_f1_through_f24_deterministically() {
    let fixed = [
        (Key::Return, KeyCode::Enter),
        (Key::Escape, KeyCode::Escape),
        (Key::Tab, KeyCode::Tab),
        (Key::BackSpace, KeyCode::Backspace),
        (Key::Delete, KeyCode::Delete),
        (Key::Insert, KeyCode::Insert),
        (Key::Home, KeyCode::Home),
        (Key::End, KeyCode::End),
        (Key::Page_Up, KeyCode::PageUp),
        (Key::Page_Down, KeyCode::PageDown),
        (Key::Up, KeyCode::ArrowUp),
        (Key::Down, KeyCode::ArrowDown),
        (Key::Left, KeyCode::ArrowLeft),
        (Key::Right, KeyCode::ArrowRight),
    ];
    for (key, expected) in fixed {
        assert_eq!(
            map_gdk_key(key, ModifierType::empty()),
            Some(TerminalInput::Key {
                code: expected,
                modifiers: KeyModifiers::default(),
            })
        );
    }

    let functions = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
    ];
    for (index, key) in functions.into_iter().enumerate() {
        assert_eq!(
            map_gdk_key(key, ModifierType::empty()),
            Some(TerminalInput::Key {
                code: KeyCode::F((index + 1) as u8),
                modifiers: KeyModifiers::default(),
            })
        );
    }
}

#[test]
fn character_mapping_preserves_all_terminal_modifiers() {
    let state = ModifierType::SHIFT_MASK
        | ModifierType::CONTROL_MASK
        | ModifierType::ALT_MASK
        | ModifierType::SUPER_MASK;
    assert_eq!(
        map_gdk_key(Key::from_name("x").unwrap(), state),
        Some(TerminalInput::Key {
            code: KeyCode::Character('x'),
            modifiers: KeyModifiers {
                shift: true,
                control: true,
                alt: true,
                super_key: true,
            },
        })
    );
}

#[test]
fn committed_text_and_normalized_paste_are_redacted() {
    let model = TerminalViewModel::new(
        rshell_core::SessionId::new(),
        FontMetrics::new(9.0, 18.0).unwrap(),
    );
    let committed = model.committed_text("IME-SENSITIVE-秘密").unwrap();
    assert_session_input(&committed);
    assert!(!format!("{committed:?}").contains("IME-SENSITIVE"));
    assert_eq!(
        format!(
            "{:?}",
            TerminalViewMsg::CommittedText("IME-SENSITIVE".into())
        ),
        "CommittedText([REDACTED])"
    );

    let paste = model.paste("PASTE-SENSITIVE\r\nnext").unwrap();
    assert!(matches!(
        paste,
        UiCommand::Session {
            command: SessionUiCommand::Paste(_),
            ..
        }
    ));
    let debug = format!("{paste:?}");
    assert!(!debug.contains("PASTE-SENSITIVE"));
    assert_eq!(
        format!("{:?}", TerminalViewMsg::PasteText("PASTE-SENSITIVE".into())),
        "PasteText([REDACTED])"
    );
    assert!(model.paste("before\0after").is_err());
}

#[test]
fn physical_alt_side_respects_resolved_profile() {
    let left_disabled = alt_model(false, true);
    assert_alt_sides(left_disabled, false, true);

    let right_disabled = alt_model(true, false);
    assert_alt_sides(right_disabled, true, false);

    let mut defaults = alt_model(true, true);
    let command = defaults
        .key_pressed(Key::from_name("x").unwrap(), ModifierType::ALT_MASK)
        .unwrap()
        .expect("default aggregate Alt maps to terminal input");
    assert!(input_modifiers(&command).alt);
}

fn alt_model(left_alt_as_meta: bool, right_alt_as_meta: bool) -> TerminalViewModel {
    let profile = TerminalSettingsV1 {
        left_alt_as_meta,
        right_alt_as_meta,
        ..TerminalSettingsV1::default()
    }
    .resolve(&TerminalOverrides::default());
    TerminalViewModel::with_profile(
        PaneId::new(),
        rshell_core::SessionId::new(),
        profile,
        FontMetrics::new(9.0, 18.0).unwrap(),
    )
}

fn assert_alt_sides(mut model: TerminalViewModel, left_meta: bool, right_meta: bool) {
    assert!(
        model
            .key_pressed(Key::Alt_L, ModifierType::ALT_MASK)
            .unwrap()
            .is_none()
    );
    let left = model
        .key_pressed(Key::from_name("x").unwrap(), ModifierType::ALT_MASK)
        .unwrap()
        .expect("left Alt key command");
    assert_eq!(input_modifiers(&left).alt, left_meta);
    model.key_released(Key::Alt_L);

    assert!(
        model
            .key_pressed(Key::Alt_R, ModifierType::ALT_MASK)
            .unwrap()
            .is_none()
    );
    let right = model
        .key_pressed(Key::from_name("x").unwrap(), ModifierType::ALT_MASK)
        .unwrap()
        .expect("right Alt key command");
    assert_eq!(input_modifiers(&right).alt, right_meta);
    model.focus_lost();
    let reset = model
        .key_pressed(Key::from_name("x").unwrap(), ModifierType::ALT_MASK)
        .unwrap()
        .expect("focus-reset key command");
    assert!(!input_modifiers(&reset).alt);
}

fn input_modifiers(command: &UiCommand) -> KeyModifiers {
    match command {
        UiCommand::Session {
            command: SessionUiCommand::Input(TerminalInput::Key { modifiers, .. }),
            ..
        } => *modifiers,
        _ => panic!("expected terminal key input"),
    }
}

fn assert_session_input(command: &UiCommand) {
    assert!(matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Input(TerminalInput::CommittedText(_)),
            ..
        }
    ));
}
