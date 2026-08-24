use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::AuthenticationKind;
use std::{cell::Cell, rc::Rc};

use crate::{
    ConnectionEditor, ConnectionEditorMsg, EditorTextField,
    connection_editor_widgets::ConnectionEditorWidgets,
};

pub(crate) fn connect_editor_widgets(
    widgets: &ConnectionEditorWidgets,
    root: &gtk::Box,
    close: &gtk::Button,
    cancel: &gtk::Button,
    sender: &ComponentSender<ConnectionEditor>,
) {
    close.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(ConnectionEditorMsg::Cancel)
    });
    cancel.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(ConnectionEditorMsg::Cancel)
    });
    widgets.save.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(ConnectionEditorMsg::Save)
    });
    for (entry, field) in [
        (&widgets.name, EditorTextField::Name),
        (&widgets.host, EditorTextField::Host),
        (&widgets.username, EditorTextField::Username),
        (&widgets.identity, EditorTextField::IdentityFile),
        (&widgets.remote_command, EditorTextField::RemoteCommand),
        (&widgets.tags, EditorTextField::Tags),
    ] {
        connect_entry(entry, widgets.syncing.clone(), sender, field);
    }
    widgets.port.connect_value_changed({
        let sender = sender.clone();
        let syncing = widgets.syncing.clone();
        move |spin| {
            if !syncing.get() {
                sender.input(ConnectionEditorMsg::PortChanged(spin.value_as_int() as u16));
            }
        }
    });
    widgets.transport.connect_selected_notify({
        let sender = sender.clone();
        let syncing = widgets.syncing.clone();
        move |dropdown| {
            if !syncing.get() {
                sender.input(ConnectionEditorMsg::TransportChanged(dropdown.selected()));
            }
        }
    });
    for (button, authentication) in [
        (&widgets.password_auth, AuthenticationKind::Password),
        (&widgets.public_key_auth, AuthenticationKind::PublicKey),
        (&widgets.agent_auth, AuthenticationKind::Agent),
        (
            &widgets.keyboard_auth,
            AuthenticationKind::KeyboardInteractive,
        ),
    ] {
        button.connect_toggled({
            let sender = sender.clone();
            let syncing = widgets.syncing.clone();
            move |button| {
                if button.is_active() && !syncing.get() {
                    sender.input(ConnectionEditorMsg::AuthenticationChanged(authentication));
                }
            }
        });
    }
    widgets.secret.connect_changed({
        let sender = sender.clone();
        let syncing = widgets.syncing.clone();
        move |entry| {
            if !syncing.get() {
                sender.input(ConnectionEditorMsg::SecretChanged(entry.text().into()));
            }
        }
    });
    widgets.note.buffer().connect_changed({
        let sender = sender.clone();
        let syncing = widgets.syncing.clone();
        move |buffer| {
            if syncing.get() {
                return;
            }
            let value = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            sender.input(ConnectionEditorMsg::TextChanged(
                EditorTextField::Note,
                value.into(),
            ));
        }
    });
    widgets.terminal_profile.connect_selected_notify({
        let sender = sender.clone();
        let syncing = widgets.syncing.clone();
        move |dropdown| {
            if !syncing.get() {
                sender.input(ConnectionEditorMsg::ProfileChanged(dropdown.selected()));
            }
        }
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let sender = sender.clone();
        move |_, key, _, modifiers| {
            if key == gtk::gdk::Key::Escape {
                sender.input(ConnectionEditorMsg::Cancel);
                return gtk::glib::Propagation::Stop;
            }
            if key == gtk::gdk::Key::Return
                && modifiers.contains(gtk::gdk::ModifierType::CONTROL_MASK)
            {
                sender.input(ConnectionEditorMsg::Save);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        }
    });
    root.add_controller(keys);
}

fn connect_entry(
    entry: &gtk::Entry,
    syncing: Rc<Cell<bool>>,
    sender: &ComponentSender<ConnectionEditor>,
    field: EditorTextField,
) {
    let sender = sender.clone();
    entry.connect_changed(move |entry| {
        if !syncing.get() {
            sender.input(ConnectionEditorMsg::TextChanged(field, entry.text().into()));
        }
    });
}
