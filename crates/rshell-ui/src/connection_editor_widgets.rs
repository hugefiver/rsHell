use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use std::{cell::Cell, rc::Rc};

use crate::{
    ConnectionEditor, ProductIcon, connection_editor_bindings::connect_editor_widgets,
    connection_editor_override_widgets::TerminalOverrideWidgets,
};

pub struct ConnectionEditorWidgets {
    pub(crate) root: gtk::Box,
    pub(crate) syncing: Rc<Cell<bool>>,
    pub(crate) title: gtk::Label,
    pub(crate) name: gtk::Entry,
    pub(crate) host: gtk::Entry,
    pub(crate) port: gtk::SpinButton,
    pub(crate) username: gtk::Entry,
    pub(crate) transport: gtk::DropDown,
    pub(crate) password_auth: gtk::CheckButton,
    pub(crate) public_key_auth: gtk::CheckButton,
    pub(crate) agent_auth: gtk::CheckButton,
    pub(crate) keyboard_auth: gtk::CheckButton,
    pub(crate) identity: gtk::Entry,
    pub(crate) secret: gtk::PasswordEntry,
    pub(crate) remote_command: gtk::Entry,
    pub(crate) note: gtk::TextView,
    pub(crate) tags: gtk::Entry,
    pub(crate) terminal_profile: gtk::DropDown,
    pub(crate) overrides: TerminalOverrideWidgets,
    pub(crate) error: gtk::Label,
    pub(crate) save: gtk::Button,
    pub(crate) profile_labels: Vec<String>,
    pub(crate) open: bool,
}

impl ConnectionEditorWidgets {
    pub(crate) fn build(root: &gtk::Box, sender: &ComponentSender<ConnectionEditor>) -> Self {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("dialog-header");
        header.set_margin_start(10);
        header.set_margin_end(10);
        header.set_margin_top(8);
        header.set_margin_bottom(4);
        let title = gtk::Label::new(Some("Connection editor"));
        title.set_hexpand(true);
        title.set_halign(gtk::Align::Start);
        title.add_css_class("connection-name");
        let close = icon_text_button(ProductIcon::CloseTab, "Close");
        header.append(&title);
        header.append(&close);
        root.append(&header);

        let grid = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(10)
            .margin_start(10)
            .margin_end(10)
            .margin_top(6)
            .margin_bottom(10)
            .build();
        grid.add_css_class("editor-group");
        grid.add_css_class("dialog-body");
        let name = entry("Connection name");
        let host = entry("Host name or address");
        let port = gtk::SpinButton::with_range(1.0, 65_535.0, 1.0);
        port.set_tooltip_text(Some("TCP port from 1 to 65535"));
        let username = entry("Remote user");
        let transport = gtk::DropDown::from_strings(&["System OpenSSH", "Native SSH"]);
        let password_auth = gtk::CheckButton::with_label("Password");
        let public_key_auth = gtk::CheckButton::with_label("Public key");
        let agent_auth = gtk::CheckButton::with_label("Agent");
        let keyboard_auth = gtk::CheckButton::with_label("Keyboard interactive");
        public_key_auth.set_group(Some(&password_auth));
        agent_auth.set_group(Some(&password_auth));
        keyboard_auth.set_group(Some(&password_auth));
        let auth = gtk::Box::new(gtk::Orientation::Vertical, 2);
        for button in [
            &password_auth,
            &public_key_auth,
            &agent_auth,
            &keyboard_auth,
        ] {
            auth.append(button);
        }
        let identity = entry("Path to private key");
        let secret = gtk::PasswordEntry::builder()
            .placeholder_text("New password or key passphrase")
            .show_peek_icon(true)
            .tooltip_text("Existing secrets are never loaded; leave untouched to preserve")
            .build();
        let remote_command = entry("Optional command after connect");
        let note = gtk::TextView::new();
        note.set_wrap_mode(gtk::WrapMode::WordChar);
        note.set_top_margin(4);
        note.set_bottom_margin(4);
        let note_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(72)
            .child(&note)
            .build();
        let tags = entry("Comma-separated tags");
        let terminal_profile = gtk::DropDown::from_strings(&["Inherit default"]);

        let mut row = 0;
        for (label, widget) in [
            ("Name", name.upcast_ref::<gtk::Widget>()),
            ("Host", host.upcast_ref()),
            ("Port", port.upcast_ref()),
            ("Username", username.upcast_ref()),
            ("Transport", transport.upcast_ref()),
            ("Authentication", auth.upcast_ref()),
            ("Identity file", identity.upcast_ref()),
            ("Secret", secret.upcast_ref()),
            ("Remote command", remote_command.upcast_ref()),
            ("Note", note_scroll.upcast_ref()),
            ("Tags", tags.upcast_ref()),
            ("Terminal profile", terminal_profile.upcast_ref()),
        ] {
            attach_row(&grid, row, label, widget);
            row += 1;
        }
        let overrides = TerminalOverrideWidgets::build(&grid, &mut row, sender);

        let error = gtk::Label::new(None);
        error.add_css_class("dialog-error");
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        error.set_selectable(true);
        grid.attach(&error, 0, row, 3, 1);
        row += 1;
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.add_css_class("dialog-footer");
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let save = gtk::Button::with_label("Save connection");
        save.add_css_class("suggested-action");
        save.add_css_class("connect-button");
        actions.append(&cancel);
        actions.append(&save);
        grid.attach(&actions, 0, row, 3, 1);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&grid)
            .build();
        root.append(&scroll);

        let widgets = Self {
            root: root.clone(),
            syncing: Rc::new(Cell::new(false)),
            title,
            name,
            host,
            port,
            username,
            transport,
            password_auth,
            public_key_auth,
            agent_auth,
            keyboard_auth,
            identity,
            secret,
            remote_command,
            note,
            tags,
            terminal_profile,
            overrides,
            error,
            save,
            profile_labels: Vec::new(),
            open: false,
        };
        connect_editor_widgets(&widgets, root, &close, &cancel, sender);
        widgets
    }
}

impl Drop for ConnectionEditorWidgets {
    fn drop(&mut self) {
        self.secret.set_text("");
    }
}

fn entry(placeholder: &str) -> gtk::Entry {
    gtk::Entry::builder().placeholder_text(placeholder).build()
}

fn attach_row(grid: &gtk::Grid, row: i32, text: &str, widget: &gtk::Widget) {
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::End);
    label.set_valign(gtk::Align::Center);
    label.set_mnemonic_widget(Some(widget));
    grid.attach(&label, 0, row, 1, 1);
    widget.set_hexpand(true);
    grid.attach(widget, 1, row, 2, 1);
}

fn icon_text_button(icon: ProductIcon, text: &str) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    content.append(&icon.image().expect("embedded dialog icon"));
    content.append(&gtk::Label::new(Some(text)));
    let button = gtk::Button::builder().child(&content).build();
    button.update_property(&[gtk::accessible::Property::Label(text)]);
    button
}
