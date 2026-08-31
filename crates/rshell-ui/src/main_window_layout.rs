use std::cell::Cell;

use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{
    IconRenderRequest, MainWindow, MainWindowMsg, MainWindowShell, ModalHost, ModalKind,
    ModalRequest, ProductIcon,
};

pub struct MainWindowWidgets;

pub fn build_command_bar(sender: &ComponentSender<MainWindow>) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("command-bar");
    let icon_request = IconRenderRequest::for_widget(16, &bar);
    let new_button = global_action(
        ProductIcon::NewTab,
        "New session",
        "New local terminal tab",
        icon_request,
    );
    let input = sender.input_sender().clone();
    new_button.connect_clicked(move |_| {
        let _ = input.send(MainWindowMsg::NewLocalTab);
    });
    bar.append(&new_button);
    let import_button = global_action(
        ProductIcon::Import,
        "Import",
        "Import connections",
        icon_request,
    );
    let input = sender.input_sender().clone();
    let trigger: gtk::Widget = import_button.clone().upcast();
    import_button.connect_clicked(move |_| {
        let _ = input.send(MainWindowMsg::Modal(ModalRequest::Open {
            kind: ModalKind::Import,
            trigger: trigger.clone(),
        }));
    });
    bar.append(&import_button);
    let settings_button = global_action(
        ProductIcon::Settings,
        "Settings",
        "Terminal settings",
        icon_request,
    );
    let input = sender.input_sender().clone();
    let trigger: gtk::Widget = settings_button.clone().upcast();
    settings_button.connect_clicked(move |_| {
        let _ = input.send(MainWindowMsg::Modal(ModalRequest::Open {
            kind: ModalKind::Settings,
            trigger: trigger.clone(),
        }));
    });
    bar.append(&settings_button);
    bar
}

fn global_action(
    icon: ProductIcon,
    text: &str,
    accessible_name: &str,
    request: IconRenderRequest,
) -> gtk::Button {
    let button = gtk::Button::builder().tooltip_text(accessible_name).build();
    button.update_property(&[gtk::accessible::Property::Label(accessible_name)]);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    content.append(&icon.image(request).expect("embedded command icon"));
    let label = gtk::Label::new(Some(text));
    label.add_css_class("global-action-label");
    content.append(&label);
    button.set_child(Some(&content));
    button
}

pub struct MainWindowContent<'a> {
    pub command_bar: &'a gtk::Box,
    pub sidebar: &'a gtk::Widget,
    pub editor: &'a gtk::Widget,
    pub tab_bar: &'a gtk::Widget,
    pub pane_host: &'a gtk::Widget,
    pub settings: &'a gtk::Widget,
    pub import: &'a gtk::Widget,
    pub interaction: &'a gtk::Widget,
}

pub fn install_content(
    root: &gtk::ApplicationWindow,
    parts: MainWindowContent<'_>,
    sender: &ComponentSender<MainWindow>,
) -> (MainWindowShell, ModalHost) {
    let shell = MainWindowShell::new(
        parts.command_bar,
        parts.sidebar,
        parts.tab_bar,
        parts.pane_host,
        sender,
    );
    let background = shell.background.clone().upcast();
    let modal = ModalHost::new(&shell.overlay, &background);
    shell.overlay.add_overlay(parts.editor);
    shell.overlay.add_overlay(parts.settings);
    shell.overlay.add_overlay(parts.import);
    shell.overlay.add_overlay(parts.interaction);
    root.set_child(Some(&shell.overlay));
    install_tab_shortcuts(root, sender);
    connect_allocation(&shell.background, sender);
    (shell, modal)
}

fn install_tab_shortcuts(root: &gtk::ApplicationWindow, sender: &ComponentSender<MainWindow>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let input = sender.input_sender().clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        if key == gtk::gdk::Key::Tab && state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            let delta = if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                -1
            } else {
                1
            };
            let _ = input.send(MainWindowMsg::CycleTabs(delta));
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    root.add_controller(keys);
}

fn connect_allocation(widget: &gtk::Box, sender: &ComponentSender<MainWindow>) {
    let previous = Cell::new(0);
    let input = sender.input_sender().clone();
    widget.add_tick_callback(move |widget, _| {
        let width = widget.width();
        if width > 0 && previous.replace(width) != width {
            let _ = input.send(MainWindowMsg::Allocated { width });
        }
        gtk::glib::ControlFlow::Continue
    });
}
