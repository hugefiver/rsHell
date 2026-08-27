use wezterm_term::{
    KeyCode as WezKeyCode, KeyModifiers as WezKeyModifiers, MouseEvent as WezMouseEvent, Terminal,
};

fn compile_key_down(terminal: &mut Terminal, key: WezKeyCode, modifiers: WezKeyModifiers) {
    let _ = terminal.key_down(key, modifiers);
}

fn compile_mouse_event(terminal: &mut Terminal, event: WezMouseEvent) {
    let _ = terminal.mouse_event(event);
}

#[test]
fn pinned_wezterm_exposes_the_required_keyboard_api() {
    let function: fn(&mut Terminal, WezKeyCode, WezKeyModifiers) = compile_key_down;
    let mouse_function: fn(&mut Terminal, WezMouseEvent) = compile_mouse_event;
    let _ = function;
    let _ = mouse_function;
}
