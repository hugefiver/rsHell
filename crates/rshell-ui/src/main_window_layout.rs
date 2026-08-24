use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{MainWindow, MainWindowMsg, ProductIcon};

pub struct MainWindowWidgets {
    pub(crate) status: gtk::Label,
}

pub fn build_command_bar(sender: &ComponentSender<MainWindow>) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("command-bar");
    let identity = gtk::Label::new(Some("rsHell"));
    identity.add_css_class("command-bar-identity");
    identity.set_halign(gtk::Align::Start);
    identity.set_hexpand(true);
    identity.set_tooltip_text(Some("Native SSH connection manager"));
    bar.append(&identity);
    let import_button = ProductIcon::Import
        .button(Some("Import connections"))
        .expect("embedded import icon");
    let input = sender.input_sender().clone();
    import_button.connect_clicked(move |_| {
        let _ = input.send(MainWindowMsg::OpenImport);
    });
    bar.append(&import_button);
    let settings_button = ProductIcon::Settings
        .button(Some("Terminal settings"))
        .expect("embedded settings icon");
    let input = sender.input_sender().clone();
    settings_button.connect_clicked(move |_| {
        let _ = input.send(MainWindowMsg::OpenSettings);
    });
    bar.append(&settings_button);
    bar
}

pub struct MainWindowContent<'a> {
    pub command_bar: &'a gtk::Widget,
    pub sidebar: &'a gtk::Widget,
    pub editor: &'a gtk::Widget,
    pub tab_bar: &'a gtk::Widget,
    pub pane_host: &'a gtk::Widget,
    pub settings: &'a gtk::Widget,
    pub import: &'a gtk::Widget,
    pub interaction: &'a gtk::Widget,
}

pub fn install_content(root: &gtk::ApplicationWindow, parts: MainWindowContent<'_>) -> gtk::Label {
    let workspace = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    let terminal_workspace = gtk::Box::new(gtk::Orientation::Vertical, 0);
    terminal_workspace.set_hexpand(true);
    terminal_workspace.set_vexpand(true);
    terminal_workspace.append(parts.tab_bar);
    terminal_workspace.append(parts.pane_host);
    workspace.append(&terminal_workspace);
    workspace.append(parts.editor);
    let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned.set_start_child(Some(parts.sidebar));
    paned.set_end_child(Some(&workspace));
    paned.set_position(232);
    paned.set_resize_start_child(false);
    paned.set_shrink_start_child(true);
    let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    status_bar.add_css_class("status-bar");
    let status = gtk::Label::new(Some("Ready"));
    status.set_halign(gtk::Align::Start);
    status.add_css_class("status-label");
    status_bar.append(&status);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(parts.command_bar);
    content.append(&paned);
    paned.set_vexpand(true);
    content.append(&status_bar);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&content));
    overlay.add_overlay(parts.settings);
    overlay.add_overlay(parts.import);
    overlay.add_overlay(parts.interaction);
    root.set_child(Some(&overlay));
    status
}
