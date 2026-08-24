use rshell_core::{
    KeyCode, KeyModifiers, MouseButton, MouseEventKind, TerminalInput, TerminalMouseEvent,
};

use crate::EngineError;

pub(crate) fn encode_input(input: TerminalInput) -> Result<Vec<u8>, EngineError> {
    match input {
        TerminalInput::CommittedText(text) => Ok(text.into_bytes()),
        TerminalInput::Key { code, modifiers } => encode_key(code, modifiers),
    }
}

pub(crate) fn encode_mouse(
    input: TerminalMouseEvent,
    reporting_enabled: bool,
) -> Result<Vec<u8>, EngineError> {
    if !reporting_enabled {
        return Err(EngineError::UnsupportedMouse("mouse reporting is disabled"));
    }
    let modifier = modifier_bits(input.modifiers);
    let (button, suffix) = match (input.kind, input.button) {
        (MouseEventKind::Press, Some(button)) => (button_code(button)? + modifier, 'M'),
        (MouseEventKind::Release, _) => (3 + modifier, 'm'),
        (MouseEventKind::Move, Some(button)) => (button_code(button)? + 32 + modifier, 'M'),
        (MouseEventKind::Move, None) => (35 + modifier, 'M'),
        (MouseEventKind::Scroll, Some(MouseButton::WheelUp)) => (64 + modifier, 'M'),
        (MouseEventKind::Scroll, Some(MouseButton::WheelDown)) => (65 + modifier, 'M'),
        (MouseEventKind::Scroll, _) => {
            return Err(EngineError::UnsupportedMouse(
                "scroll has no wheel direction",
            ));
        }
        (MouseEventKind::Press, None) => {
            return Err(EngineError::UnsupportedMouse("press has no button"));
        }
    };
    let column = u32::from(input.cell.column) + 1;
    let row = u32::from(input.viewport_row) + 1;
    Ok(format!("\x1b[<{button};{column};{row}{suffix}").into_bytes())
}

fn encode_key(code: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>, EngineError> {
    if modifiers.super_key {
        return Err(EngineError::UnsupportedInput("super-modified key"));
    }
    if let KeyCode::Character(character) = code {
        return encode_character(character, modifiers);
    }
    let modifier = xterm_modifier(modifiers);
    let plain = modifier == 1;
    let sequence = match code {
        KeyCode::Enter if plain => "\r".into(),
        KeyCode::Escape if plain => "\x1b".into(),
        KeyCode::Tab if plain => "\t".into(),
        KeyCode::Tab if modifiers.shift && !modifiers.control && !modifiers.alt => "\x1b[Z".into(),
        KeyCode::Backspace if plain => "\x7f".into(),
        KeyCode::ArrowUp => csi_final('A', modifier),
        KeyCode::ArrowDown => csi_final('B', modifier),
        KeyCode::ArrowRight => csi_final('C', modifier),
        KeyCode::ArrowLeft => csi_final('D', modifier),
        KeyCode::Home => csi_final('H', modifier),
        KeyCode::End => csi_final('F', modifier),
        KeyCode::Insert => csi_tilde(2, modifier),
        KeyCode::Delete => csi_tilde(3, modifier),
        KeyCode::PageUp => csi_tilde(5, modifier),
        KeyCode::PageDown => csi_tilde(6, modifier),
        KeyCode::F(number) => function_key(number, modifier)?,
        _ => return Err(EngineError::UnsupportedInput("modified control key")),
    };
    Ok(sequence.into_bytes())
}

fn encode_character(character: char, modifiers: KeyModifiers) -> Result<Vec<u8>, EngineError> {
    let mut bytes = Vec::new();
    if modifiers.alt {
        bytes.push(0x1b);
    }
    if modifiers.control {
        if character.is_ascii_alphabetic() {
            bytes.push(character.to_ascii_uppercase() as u8 - b'@');
        } else {
            return Err(EngineError::UnsupportedInput(
                "control modifier requires an ASCII letter",
            ));
        }
    } else {
        let mut encoded = [0; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
    }
    Ok(bytes)
}

fn xterm_modifier(modifiers: KeyModifiers) -> u8 {
    1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(modifiers.control)
}

fn modifier_bits(modifiers: KeyModifiers) -> u8 {
    4 * u8::from(modifiers.shift) + 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control)
}

fn button_code(button: MouseButton) -> Result<u8, EngineError> {
    match button {
        MouseButton::Left => Ok(0),
        MouseButton::Middle => Ok(1),
        MouseButton::Right => Ok(2),
        MouseButton::Back => Ok(128),
        MouseButton::Forward => Ok(129),
        MouseButton::WheelUp | MouseButton::WheelDown => {
            Err(EngineError::UnsupportedMouse("wheel used outside scroll"))
        }
    }
}

fn csi_final(final_character: char, modifier: u8) -> String {
    if modifier == 1 {
        format!("\x1b[{final_character}")
    } else {
        format!("\x1b[1;{modifier}{final_character}")
    }
}

fn csi_tilde(number: u8, modifier: u8) -> String {
    if modifier == 1 {
        format!("\x1b[{number}~")
    } else {
        format!("\x1b[{number};{modifier}~")
    }
}

fn function_key(number: u8, modifier: u8) -> Result<String, EngineError> {
    let code = match number {
        1..=4 => return Ok(csi_final((b'P' + number - 1) as char, modifier)),
        5 => 15,
        6 => 17,
        7 => 18,
        8 => 19,
        9 => 20,
        10 => 21,
        11 => 23,
        12 => 24,
        13 => 25,
        14 => 26,
        15 => 28,
        16 => 29,
        17 => 31,
        18 => 32,
        19 => 33,
        20 => 34,
        _ => return Err(EngineError::UnsupportedInput("function key above F20")),
    };
    Ok(csi_tilde(code, modifier))
}
