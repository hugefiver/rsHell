use gtk::{gdk, glib, prelude::*};
use relm4::ComponentSender;
use rshell_core::{KeyBinding, KeyCode, TerminalInput};

use crate::{
    TerminalView, TerminalViewMsg,
    terminal_input::{map_gdk_key, modifiers},
};

pub(crate) fn connect_keyboard(
    canvas: &gtk::DrawingArea,
    search: &gtk::SearchEntry,
    im_context: &gtk::IMMulticontext,
    bindings: &[KeyBinding],
    sender: &ComponentSender<TerminalView>,
) {
    let commit_sender = sender.clone();
    im_context.connect_commit(move |_, text| {
        commit_sender.input(TerminalViewMsg::CommittedText(text.to_owned()));
    });
    let key = gtk::EventControllerKey::new();
    key.set_im_context(Some(im_context));
    let key_sender = sender.clone();
    let bindings = bindings.to_vec();
    key.connect_key_pressed(move |_, key, _, state| {
        if should_handle_key(key, state, &bindings) {
            key_sender.input(TerminalViewMsg::Key { key, state });
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    let release_sender = sender.clone();
    key.connect_key_released(move |_, key, _, _| {
        release_sender.input(TerminalViewMsg::KeyReleased(key));
    });
    canvas.add_controller(key);

    let focus = gtk::EventControllerFocus::new();
    let focus_in = im_context.clone();
    focus.connect_enter(move |_| focus_in.focus_in());
    let focus_out = im_context.clone();
    let focus_sender = sender.clone();
    focus.connect_leave(move |_| {
        focus_out.focus_out();
        focus_sender.input(TerminalViewMsg::FocusLost);
    });
    canvas.add_controller(focus);

    let search_keys = gtk::EventControllerKey::new();
    let search_sender = sender.clone();
    search_keys.connect_key_pressed(move |_, key, _, state| {
        if matches!(
            key,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::Escape
        ) {
            search_sender.input(TerminalViewMsg::Key { key, state });
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    search.add_controller(search_keys);
}

fn should_handle_key(key: gdk::Key, state: gdk::ModifierType, bindings: &[KeyBinding]) -> bool {
    if matches!(key, gdk::Key::Alt_L | gdk::Key::Alt_R) {
        return true;
    }
    let terminal_modifiers = modifiers(state);
    if terminal_modifiers.control || terminal_modifiers.alt || terminal_modifiers.super_key {
        return true;
    }
    map_gdk_key(key, state).is_some_and(|input| match input {
        TerminalInput::Key {
            code: KeyCode::Character(character),
            modifiers,
        } => bindings.iter().any(|binding| {
            binding.code == KeyCode::Character(character) && binding.modifiers == modifiers
        }),
        TerminalInput::Key { .. } => true,
        TerminalInput::CommittedText(_) => false,
    })
}
