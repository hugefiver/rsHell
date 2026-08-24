use gtk::prelude::*;
use relm4::ComponentSender;
use std::{cell::Cell, rc::Rc};

use crate::{
    ConnectionEditor, ConnectionEditorMsg, TerminalOverrideKey,
    connection_editor_override_widgets::{OverrideControl, TerminalOverrideWidgets},
};

pub(crate) fn connect_override_widgets(
    widgets: &TerminalOverrideWidgets,
    sender: &ComponentSender<ConnectionEditor>,
) {
    connect_entry(
        &widgets.terminal_type,
        TerminalOverrideKey::TerminalType,
        sender,
    );
    connect_spin(
        &widgets.initial_cols,
        TerminalOverrideKey::InitialCols,
        sender,
    );
    connect_spin(
        &widgets.initial_rows,
        TerminalOverrideKey::InitialRows,
        sender,
    );
    connect_spin(
        &widgets.scrollback,
        TerminalOverrideKey::ScrollbackLines,
        sender,
    );
    connect_entry(
        &widgets.font_family,
        TerminalOverrideKey::FontFamily,
        sender,
    );
    connect_spin(&widgets.font_size, TerminalOverrideKey::FontSize, sender);
    connect_inherit(
        &widgets.color_scheme.inherit,
        TerminalOverrideKey::ColorScheme,
        widgets.color_scheme.syncing.clone(),
        sender,
    );
    widgets.color_scheme.value.connect_selected_notify({
        let sender = sender.clone();
        let syncing = widgets.color_scheme.syncing.clone();
        move |dropdown| {
            if dropdown.is_sensitive() && !syncing.get() {
                sender.input(ConnectionEditorMsg::OverrideScheme(dropdown.selected()));
            }
        }
    });
    connect_inherit(
        &widgets.key_bindings.inherit,
        TerminalOverrideKey::KeyBindings,
        widgets.key_bindings.syncing.clone(),
        sender,
    );
    widgets.key_bindings.value.connect_changed({
        let sender = sender.clone();
        let syncing = widgets.key_bindings.syncing.clone();
        move |entry| {
            if entry.is_sensitive() && !syncing.get() {
                sender.input(ConnectionEditorMsg::OverrideBindings(entry.text().into()));
            }
        }
    });
    for (control, key) in [
        (&widgets.left_alt, TerminalOverrideKey::LeftAltAsMeta),
        (&widgets.right_alt, TerminalOverrideKey::RightAltAsMeta),
        (&widgets.csi_u, TerminalOverrideKey::EnableCsiU),
        (&widgets.kitty, TerminalOverrideKey::EnableKittyKeyboard),
        (&widgets.mouse, TerminalOverrideKey::MouseReporting),
        (&widgets.scroll_output, TerminalOverrideKey::ScrollOnOutput),
        (&widgets.scroll_key, TerminalOverrideKey::ScrollOnKeypress),
    ] {
        connect_bool(control, key, sender);
    }
    connect_entry(&widgets.answerback, TerminalOverrideKey::Answerback, sender);
    widgets.clear.connect_clicked({
        let sender = sender.clone();
        move |_| sender.input(ConnectionEditorMsg::ClearOverrides)
    });
}

fn connect_entry(
    control: &OverrideControl<gtk::Entry>,
    key: TerminalOverrideKey,
    sender: &ComponentSender<ConnectionEditor>,
) {
    connect_inherit(&control.inherit, key, control.syncing.clone(), sender);
    control.value.connect_changed({
        let sender = sender.clone();
        let syncing = control.syncing.clone();
        move |entry| {
            if entry.is_sensitive() && !syncing.get() {
                sender.input(ConnectionEditorMsg::OverrideText(key, entry.text().into()));
            }
        }
    });
}

fn connect_spin(
    control: &OverrideControl<gtk::SpinButton>,
    key: TerminalOverrideKey,
    sender: &ComponentSender<ConnectionEditor>,
) {
    connect_inherit(&control.inherit, key, control.syncing.clone(), sender);
    control.value.connect_value_changed({
        let sender = sender.clone();
        let syncing = control.syncing.clone();
        move |spin| {
            if spin.is_sensitive() && !syncing.get() {
                sender.input(ConnectionEditorMsg::OverrideNumber(key, spin.value()));
            }
        }
    });
}

fn connect_bool(
    control: &OverrideControl<gtk::CheckButton>,
    key: TerminalOverrideKey,
    sender: &ComponentSender<ConnectionEditor>,
) {
    connect_inherit(&control.inherit, key, control.syncing.clone(), sender);
    control.value.connect_toggled({
        let sender = sender.clone();
        let syncing = control.syncing.clone();
        move |button| {
            if button.is_sensitive() && !syncing.get() {
                sender.input(ConnectionEditorMsg::OverrideBool(key, button.is_active()));
            }
        }
    });
}

fn connect_inherit(
    button: &gtk::CheckButton,
    key: TerminalOverrideKey,
    syncing: Rc<Cell<bool>>,
    sender: &ComponentSender<ConnectionEditor>,
) {
    let sender = sender.clone();
    button.connect_toggled(move |button| {
        if !syncing.get() {
            sender.input(ConnectionEditorMsg::OverrideInheritance(
                key,
                button.is_active(),
            ));
        }
    });
}
