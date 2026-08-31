use std::cell::Cell;

use gtk::prelude::*;
use relm4::gtk;

use crate::modal_focus::ModalFocusSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalKind {
    ConnectionEditor,
    Settings,
    Import,
    Interaction,
}

#[derive(Debug)]
pub enum ModalRequest {
    Open {
        kind: ModalKind,
        trigger: gtk::Widget,
    },
    Close(ModalKind),
}

pub struct ModalHost {
    overlay: gtk::Overlay,
    scrim: gtk::Box,
    background: gtk::Widget,
    open: Option<ModalKind>,
    focus: Option<ModalFocusSession>,
    surface: gtk::glib::WeakRef<gtk::Widget>,
    keys: Option<gtk::EventControllerKey>,
    window_width: Cell<i32>,
}

impl ModalHost {
    pub fn new(overlay: &gtk::Overlay, background: &gtk::Widget) -> Self {
        let scrim = gtk::Box::new(gtk::Orientation::Vertical, 0);
        scrim.add_css_class("modal-scrim");
        scrim.set_hexpand(true);
        scrim.set_vexpand(true);
        scrim.set_halign(gtk::Align::Fill);
        scrim.set_valign(gtk::Align::Fill);
        scrim.set_can_target(true);
        scrim.set_visible(false);
        overlay.add_overlay(&scrim);
        Self {
            overlay: overlay.clone(),
            scrim,
            background: background.clone(),
            open: None,
            focus: None,
            surface: gtk::glib::WeakRef::new(),
            keys: None,
            window_width: Cell::new(1_360),
        }
    }

    pub fn open(&mut self, kind: ModalKind, surface: &gtk::Widget, trigger: &gtk::Widget) {
        if self.open == Some(kind) {
            return;
        }
        self.finish_close();
        self.hide_surfaces();
        surface.set_halign(gtk::Align::Center);
        surface.set_valign(gtk::Align::Fill);
        surface.set_margin_top(24);
        surface.set_margin_bottom(24);
        surface.set_width_request(modal_width(self.window_width.get()));
        surface.set_visible(true);
        self.scrim.add_css_class("modal-open");
        self.scrim.set_visible(true);
        self.background.set_sensitive(false);

        let fallback = find_css_descendant(&self.background, "terminal-canvas")
            .unwrap_or_else(|| self.background.clone());
        let focus = ModalFocusSession::new(trigger, &fallback, surface);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed({
            let focus = focus.clone();
            move |_, key, _, modifiers| {
                if key != gtk::gdk::Key::Tab {
                    return gtk::glib::Propagation::Proceed;
                }
                focus.contain_tab(modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK))
            }
        });
        surface.add_controller(keys.clone());
        focus.focus_first();
        gtk::glib::idle_add_local_once({
            let focus = focus.clone();
            move || focus.focus_first()
        });
        self.open = Some(kind);
        self.focus = Some(focus);
        self.surface.set(Some(surface));
        self.keys = Some(keys);
    }

    pub fn resize(&self, window_width: i32) {
        self.window_width.set(window_width);
        let width = modal_width(window_width);
        visit_children(self.overlay.upcast_ref(), &mut |widget| {
            if widget.has_css_class("content-dialog") {
                widget.set_width_request(width);
            }
        });
    }

    pub fn close(&mut self, kind: ModalKind) {
        if self.open == Some(kind) {
            self.finish_close();
        }
    }

    pub fn open_kind(&self) -> Option<ModalKind> {
        self.open
    }

    fn finish_close(&mut self) {
        if let Some(surface) = self.surface.upgrade()
            && let Some(keys) = self.keys.take()
        {
            surface.remove_controller(&keys);
        }
        self.background.set_sensitive(true);
        if let Some(focus) = self.focus.take() {
            focus.restore();
        }
        if let Some(surface) = self.surface.upgrade() {
            surface.set_visible(false);
        }
        self.scrim.remove_css_class("modal-open");
        self.scrim.set_visible(false);
        self.open = None;
        self.surface.set(gtk::Widget::NONE);
    }

    fn hide_surfaces(&self) {
        visit_children(self.overlay.upcast_ref(), &mut |widget| {
            if widget.has_css_class("content-dialog") {
                widget.set_visible(false);
            }
        });
    }
}

fn modal_width(window_width: i32) -> i32 {
    680.min((window_width - 48).max(1))
}

fn find_css_descendant(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(current) = child {
        if current.has_css_class(class) {
            return Some(current);
        }
        if let Some(found) = find_css_descendant(&current, class) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn visit_children(widget: &gtk::Widget, visit: &mut impl FnMut(&gtk::Widget)) {
    let mut child = widget.first_child();
    while let Some(current) = child {
        visit(&current);
        visit_children(&current, visit);
        child = current.next_sibling();
    }
}
