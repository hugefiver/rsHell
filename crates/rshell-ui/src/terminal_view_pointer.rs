use gtk::{glib, prelude::*};
use relm4::ComponentSender;
use rshell_core::MouseButton;

use crate::{PointerEvent, TerminalView, TerminalViewMsg, terminal_input::modifiers};

pub(crate) fn connect_pointer(canvas: &gtk::DrawingArea, sender: &ComponentSender<TerminalView>) {
    let click = gtk::GestureClick::new();
    click.set_button(0);
    let press_sender = sender.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        if let Some(button) = mouse_button(gesture.current_button()) {
            let scale = gesture
                .widget()
                .map_or(1.0, |widget| f64::from(widget.scale_factor()));
            let event = PointerEvent::press(x, y, scale, button)
                .with_modifiers(modifiers(gesture.current_event_state()));
            press_sender.input(TerminalViewMsg::Pointer(event));
        }
    });
    let release_sender = sender.clone();
    click.connect_released(move |gesture, _, x, y| {
        if let Some(button) = mouse_button(gesture.current_button()) {
            let scale = gesture
                .widget()
                .map_or(1.0, |widget| f64::from(widget.scale_factor()));
            let event = PointerEvent::release(x, y, scale, button)
                .with_modifiers(modifiers(gesture.current_event_state()));
            release_sender.input(TerminalViewMsg::Pointer(event));
        }
    });
    canvas.add_controller(click);

    let motion = gtk::EventControllerMotion::new();
    let motion_sender = sender.clone();
    motion.connect_motion(move |controller, x, y| {
        let scale = controller
            .widget()
            .map_or(1.0, |widget| f64::from(widget.scale_factor()));
        let event = PointerEvent::movement(x, y, scale, None)
            .with_modifiers(modifiers(controller.current_event_state()));
        motion_sender.input(TerminalViewMsg::Pointer(event));
    });
    canvas.add_controller(motion);

    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    let scroll_sender = sender.clone();
    scroll.connect_scroll(move |controller, _, delta_y| {
        let delta = if delta_y < 0.0 {
            -3
        } else if delta_y > 0.0 {
            3
        } else {
            0
        };
        if delta != 0 {
            let (x, y) = controller
                .current_event()
                .and_then(|event| event.position())
                .unwrap_or((0.0, 0.0));
            let scale = controller
                .widget()
                .map_or(1.0, |widget| f64::from(widget.scale_factor()));
            let event = PointerEvent::scroll(x, y, scale, delta)
                .with_modifiers(modifiers(controller.current_event_state()));
            scroll_sender.input(TerminalViewMsg::Pointer(event));
        }
        glib::Propagation::Stop
    });
    canvas.add_controller(scroll);
}

fn mouse_button(button: u32) -> Option<MouseButton> {
    match button {
        1 => Some(MouseButton::Left),
        2 => Some(MouseButton::Middle),
        3 => Some(MouseButton::Right),
        8 => Some(MouseButton::Back),
        9 => Some(MouseButton::Forward),
        _ => None,
    }
}
