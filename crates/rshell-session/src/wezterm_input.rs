use rshell_core::{
    KeyCode, KeyModifiers, MouseButton, MouseEventKind, TerminalMouseEvent, TerminalSize,
};
use wezterm_term::{
    KeyCode as WezKeyCode, KeyModifiers as WezKeyModifiers, MouseButton as WezMouseButton,
    MouseEvent as WezMouseEvent, MouseEventKind as WezMouseEventKind,
};

use crate::EngineError;

pub(crate) fn map_key(code: KeyCode) -> Result<WezKeyCode, EngineError> {
    Ok(match code {
        KeyCode::Character(character) => WezKeyCode::Char(character),
        KeyCode::Enter => WezKeyCode::Enter,
        KeyCode::Escape => WezKeyCode::Escape,
        KeyCode::Tab => WezKeyCode::Tab,
        KeyCode::Backspace => WezKeyCode::Backspace,
        KeyCode::Delete => WezKeyCode::Delete,
        KeyCode::Insert => WezKeyCode::Insert,
        KeyCode::Home => WezKeyCode::Home,
        KeyCode::End => WezKeyCode::End,
        KeyCode::PageUp => WezKeyCode::PageUp,
        KeyCode::PageDown => WezKeyCode::PageDown,
        KeyCode::ArrowUp => WezKeyCode::UpArrow,
        KeyCode::ArrowDown => WezKeyCode::DownArrow,
        KeyCode::ArrowLeft => WezKeyCode::LeftArrow,
        KeyCode::ArrowRight => WezKeyCode::RightArrow,
        KeyCode::F(number) if (1..=24).contains(&number) => WezKeyCode::Function(number),
        KeyCode::F(_) => return Err(EngineError::UnsupportedInput("function key outside F1-F24")),
    })
}

pub(crate) fn map_key_modifiers(modifiers: KeyModifiers) -> Result<WezKeyModifiers, EngineError> {
    if modifiers.super_key {
        return Err(EngineError::UnsupportedInput("super-modified key"));
    }
    Ok(map_modifiers(modifiers, false))
}

pub(crate) fn map_mouse(
    event: TerminalMouseEvent,
    size: TerminalSize,
) -> Result<WezMouseEvent, EngineError> {
    let kind = match event.kind {
        MouseEventKind::Press | MouseEventKind::Scroll => WezMouseEventKind::Press,
        MouseEventKind::Release => WezMouseEventKind::Release,
        MouseEventKind::Move => WezMouseEventKind::Move,
    };
    let button = map_mouse_button(event.kind, event.button)?;
    let x = usize::from(event.cell.column);
    Ok(WezMouseEvent {
        kind,
        x,
        y: i64::from(event.viewport_row),
        x_pixel_offset: pixel_offset(event.pixel_x, x, size.pixel_width, size.cols),
        y_pixel_offset: pixel_offset(
            event.pixel_y,
            usize::from(event.viewport_row),
            size.pixel_height,
            size.rows,
        ),
        button,
        modifiers: map_modifiers(event.modifiers, true),
    })
}

fn map_modifiers(modifiers: KeyModifiers, include_super: bool) -> WezKeyModifiers {
    let mut mapped = WezKeyModifiers::NONE;
    mapped.set(WezKeyModifiers::SHIFT, modifiers.shift);
    mapped.set(WezKeyModifiers::CTRL, modifiers.control);
    mapped.set(WezKeyModifiers::ALT, modifiers.alt);
    mapped.set(WezKeyModifiers::SUPER, include_super && modifiers.super_key);
    mapped
}

fn map_mouse_button(
    kind: MouseEventKind,
    button: Option<MouseButton>,
) -> Result<WezMouseButton, EngineError> {
    match (kind, button) {
        (MouseEventKind::Press, Some(MouseButton::Left))
        | (MouseEventKind::Release, Some(MouseButton::Left))
        | (MouseEventKind::Move, Some(MouseButton::Left)) => Ok(WezMouseButton::Left),
        (MouseEventKind::Press, Some(MouseButton::Middle))
        | (MouseEventKind::Release, Some(MouseButton::Middle))
        | (MouseEventKind::Move, Some(MouseButton::Middle)) => Ok(WezMouseButton::Middle),
        (MouseEventKind::Press, Some(MouseButton::Right))
        | (MouseEventKind::Release, Some(MouseButton::Right))
        | (MouseEventKind::Move, Some(MouseButton::Right)) => Ok(WezMouseButton::Right),
        (MouseEventKind::Scroll, Some(MouseButton::WheelUp)) => Ok(WezMouseButton::WheelUp(1)),
        (MouseEventKind::Scroll, Some(MouseButton::WheelDown)) => Ok(WezMouseButton::WheelDown(1)),
        (MouseEventKind::Move | MouseEventKind::Release, None) => Ok(WezMouseButton::None),
        (MouseEventKind::Press, None) => Err(EngineError::UnsupportedMouse("press has no button")),
        (MouseEventKind::Scroll, _) => Err(EngineError::UnsupportedMouse(
            "scroll has no wheel direction",
        )),
        (_, Some(MouseButton::Back | MouseButton::Forward)) => Err(EngineError::UnsupportedMouse(
            "back and forward mouse buttons are unsupported",
        )),
        (_, Some(MouseButton::WheelUp | MouseButton::WheelDown)) => {
            Err(EngineError::UnsupportedMouse("wheel used outside scroll"))
        }
    }
}

fn pixel_offset(pixel: u32, cell: usize, total_pixels: u32, cells: u16) -> isize {
    let cell_pixels = usize::try_from(total_pixels).unwrap_or(usize::MAX) / usize::from(cells);
    let pixel = usize::try_from(pixel).unwrap_or(usize::MAX);
    let offset = pixel.saturating_sub(cell.saturating_mul(cell_pixels));
    isize::try_from(offset).unwrap_or(isize::MAX)
}
