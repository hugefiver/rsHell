use gtk::gdk;
use relm4::ComponentSender;
use rshell_core::{MouseButton, MouseEventKind};

use super::{TerminalView, TerminalViewOutput, output};
use crate::{PointerEvent, terminal_view_clipboard as clipboard_io};

impl TerminalView {
    pub(super) fn handle_key(
        &mut self,
        key: gdk::Key,
        state: gdk::ModifierType,
        sender: &ComponentSender<Self>,
    ) {
        let value = key.to_unicode().map(|value| value.to_ascii_lowercase());
        let control_shift = state.contains(gdk::ModifierType::CONTROL_MASK)
            && state.contains(gdk::ModifierType::SHIFT_MASK);
        if control_shift && value == Some('c') {
            let _ = output::command(self.model.copy(), sender);
        } else if control_shift && value == Some('v') {
            clipboard_io::read(&self.clipboard, sender);
        } else {
            match self.model.key(key, state) {
                Ok(Some(command)) => {
                    let _ = output::command(command, sender);
                }
                Ok(None) => {}
                Err(error) => {
                    let _ = sender.output(TerminalViewOutput::Error(error));
                }
            }
        }
    }

    pub(super) fn handle_pointer(
        &mut self,
        mut event: PointerEvent,
        sender: &ComponentSender<Self>,
    ) {
        if event.kind == MouseEventKind::Move {
            event.button = self.pressed_button;
        }
        let reports_mouse = self.model.reports_mouse();
        if reports_mouse {
            if event.kind == MouseEventKind::Press {
                self.pressed_button = event.button;
            } else if event.kind == MouseEventKind::Release {
                self.pressed_button = None;
            }
            output::optional(self.model.mouse(event), sender);
            return;
        }
        match event.kind {
            MouseEventKind::Press if event.button == Some(MouseButton::Left) => {
                self.selection_anchor = Some((event.x, event.y));
            }
            MouseEventKind::Move => {
                if let Some((start_x, start_y)) = self.selection_anchor {
                    output::result(
                        self.model
                            .selection(start_x, start_y, event.x, event.y, false),
                        sender,
                    );
                }
            }
            MouseEventKind::Release => {
                if let Some((start_x, start_y)) = self.selection_anchor.take() {
                    output::result(
                        self.model
                            .selection(start_x, start_y, event.x, event.y, false),
                        sender,
                    );
                }
            }
            MouseEventKind::Scroll => output::optional(self.model.mouse(event), sender),
            MouseEventKind::Press => {}
        }
    }
}
