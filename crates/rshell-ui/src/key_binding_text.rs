use rshell_core::{KeyBinding, KeyCode, KeyModifiers};

pub fn parse_bindings(text: &str) -> Result<Vec<KeyBinding>, &'static str> {
    text.split(';')
        .map(str::trim)
        .filter(|binding| !binding.is_empty())
        .map(parse_binding)
        .collect()
}

fn parse_binding(binding: &str) -> Result<KeyBinding, &'static str> {
    let (chord, action) = binding
        .split_once('=')
        .ok_or("Use Chord=action for each key binding")?;
    if action.trim().is_empty() {
        return Err("Key binding action cannot be blank");
    }
    let mut parts = chord.split('+').map(str::trim).peekable();
    let mut modifiers = KeyModifiers::default();
    let mut key = None;
    while let Some(part) = parts.next() {
        match part.to_ascii_lowercase().as_str() {
            "shift" => modifiers.shift = true,
            "ctrl" | "control" => modifiers.control = true,
            "alt" => modifiers.alt = true,
            "super" | "meta" => modifiers.super_key = true,
            _ if parts.peek().is_none() => key = Some(parse_key(part)?),
            _ => return Err("Invalid key chord"),
        }
    }
    Ok(KeyBinding {
        code: key.ok_or("Key chord requires a key")?,
        modifiers,
        action: action.trim().into(),
    })
}

fn parse_key(value: &str) -> Result<KeyCode, &'static str> {
    let lower = value.to_ascii_lowercase();
    let named = match lower.as_str() {
        "enter" => Some(KeyCode::Enter),
        "escape" | "esc" => Some(KeyCode::Escape),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "delete" => Some(KeyCode::Delete),
        "insert" => Some(KeyCode::Insert),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "up" => Some(KeyCode::ArrowUp),
        "down" => Some(KeyCode::ArrowDown),
        "left" => Some(KeyCode::ArrowLeft),
        "right" => Some(KeyCode::ArrowRight),
        _ => None,
    };
    if let Some(code) = named {
        return Ok(code);
    }
    if let Some(number) = lower.strip_prefix('f').and_then(|value| value.parse().ok()) {
        return Ok(KeyCode::F(number));
    }
    let mut characters = value.chars();
    let character = characters.next().ok_or("Key cannot be blank")?;
    if characters.next().is_some() || character.is_control() {
        Err("Invalid key chord")
    } else {
        Ok(KeyCode::Character(character))
    }
}

pub fn display_bindings(bindings: &[KeyBinding]) -> String {
    bindings
        .iter()
        .map(|binding| {
            let mut parts = Vec::new();
            if binding.modifiers.control {
                parts.push("Ctrl".to_owned());
            }
            if binding.modifiers.shift {
                parts.push("Shift".to_owned());
            }
            if binding.modifiers.alt {
                parts.push("Alt".to_owned());
            }
            if binding.modifiers.super_key {
                parts.push("Super".to_owned());
            }
            parts.push(display_key(&binding.code));
            format!("{}={}", parts.join("+"), binding.action)
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn display_key(code: &KeyCode) -> String {
    match code {
        KeyCode::Character(value) => value.to_string(),
        KeyCode::F(value) => format!("F{value}"),
        value => format!("{value:?}").replace("Arrow", ""),
    }
}
