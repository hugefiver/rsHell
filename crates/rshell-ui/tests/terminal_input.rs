use gtk::gdk::{Key, ModifierType};
use rshell_core::{KeyCode, KeyModifiers, SessionUiCommand, TerminalInput, UiCommand};
use rshell_ui::{FontMetrics, TerminalViewModel, TerminalViewMsg, map_gdk_key};

#[test]
fn maps_terminal_keys_and_f1_through_f12_deterministically() {
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

fn assert_session_input(command: &UiCommand) {
    assert!(matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Input(TerminalInput::CommittedText(_)),
            ..
        }
    ));
}
