use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{SettingsWindow, SettingsWindowMsg};

pub struct SettingsWindowWidgets {
    pub profile: gtk::DropDown,
    pub name: gtk::Entry,
    pub terminal_type: gtk::Entry,
    pub cols: gtk::SpinButton,
    pub rows: gtk::SpinButton,
    pub scrollback: gtk::SpinButton,
    pub font_family: gtk::Entry,
    pub font_size: gtk::SpinButton,
    pub scheme: gtk::DropDown,
    pub bindings: gtk::Entry,
    pub toggles: Vec<gtk::CheckButton>,
    pub answerback: gtk::Entry,
    pub default_profile: gtk::DropDown,
    pub app_scheme: gtk::DropDown,
    pub app_bindings: gtk::Entry,
    pub save_profile: gtk::Button,
    pub save_app: gtk::Button,
    pub error: gtk::Label,
    pub profile_names: Vec<String>,
}

impl SettingsWindowWidgets {
    pub fn build(root: &gtk::Box, sender: &ComponentSender<SettingsWindow>) -> Self {
        let title = gtk::Label::new(Some("Terminal settings"));
        title.add_css_class("title-2");
        title.add_css_class("dialog-header");
        title.set_halign(gtk::Align::Start);
        root.append(&title);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("dialog-body");

        body.append(&section("Application"));
        let app_form = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(8)
            .hexpand(true)
            .build();
        let default_profile = dropdown(&app_form, 0, "Default profile");
        default_profile.add_css_class("modal-focus-first");
        let app_scheme = scheme_dropdown(&app_form, 1, "Default color scheme");
        let app_bindings = entry(&app_form, 2, "Default key bindings");
        body.append(&app_form);

        body.append(&section("Active terminal profile"));
        let form = gtk::Grid::builder()
            .column_spacing(12)
            .row_spacing(8)
            .hexpand(true)
            .build();
        let profile = dropdown(&form, 0, "Terminal profile");
        let name = entry(&form, 1, "Profile name");
        let terminal_type = entry(&form, 2, "Terminal type");
        let cols = spin(&form, 3, "Columns", 1.0, 999.0, 1.0);
        let rows = spin(&form, 4, "Rows", 1.0, 999.0, 1.0);
        let scrollback = spin(&form, 5, "Scrollback lines", 100.0, 1_000_000.0, 100.0);
        let font_family = entry(&form, 6, "Font family");
        let font_size = spin(&form, 7, "Font size", 6.0, 72.0, 0.5);
        let scheme = scheme_dropdown(&form, 8, "Color scheme");
        let bindings = entry(&form, 9, "Key bindings");
        bindings.set_placeholder_text(Some("Ctrl+Shift+T=new_tab; Ctrl+W=close"));

        let toggle_labels = [
            "Left Alt acts as Meta",
            "Right Alt acts as Meta",
            "Enable CSI-u",
            "Enable Kitty keyboard protocol",
            "Mouse reporting",
            "Scroll on output",
            "Scroll on keypress",
        ];
        let mut toggles = Vec::new();
        for (offset, label) in toggle_labels.into_iter().enumerate() {
            let toggle = gtk::CheckButton::with_label(label);
            toggle.set_halign(gtk::Align::Start);
            form.attach(&toggle, 1, 10 + offset as i32, 1, 1);
            toggles.push(toggle);
        }
        let answerback = entry(&form, 17, "Answerback");
        body.append(&form);

        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&body)
            .build();
        root.append(&scroll);

        let error = gtk::Label::new(None);
        error.add_css_class("settings-error");
        error.add_css_class("dialog-error");
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        root.append(&error);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.add_css_class("dialog-footer");
        actions.set_halign(gtk::Align::End);
        let close = gtk::Button::with_label("Close");
        close.add_css_class("modal-focus-last");
        let save_profile = gtk::Button::with_label("Save profile");
        let save_app = gtk::Button::with_label("Save defaults");
        save_profile.add_css_class("suggested-action");
        actions.append(&save_profile);
        actions.append(&save_app);
        actions.append(&close);
        root.append(&actions);
        close.connect_clicked({
            let input = sender.input_sender().clone();
            move |_| {
                let _ = input.send(SettingsWindowMsg::Close);
            }
        });
        save_profile.connect_clicked({
            let input = sender.input_sender().clone();
            move |_| {
                let _ = input.send(SettingsWindowMsg::SaveProfile);
            }
        });
        save_app.connect_clicked({
            let input = sender.input_sender().clone();
            move |_| {
                let _ = input.send(SettingsWindowMsg::SaveApp);
            }
        });
        Self {
            profile,
            name,
            terminal_type,
            cols,
            rows,
            scrollback,
            font_family,
            font_size,
            scheme,
            bindings,
            toggles,
            answerback,
            default_profile,
            app_scheme,
            app_bindings,
            save_profile,
            save_app,
            error,
            profile_names: Vec::new(),
        }
    }
}

fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dialog-section");
    label.set_halign(gtk::Align::Start);
    label
}

fn label(grid: &gtk::Grid, row: i32, text: &str) {
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::End);
    grid.attach(&label, 0, row, 1, 1);
}

fn entry(grid: &gtk::Grid, row: i32, text: &str) -> gtk::Entry {
    label(grid, row, text);
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.update_property(&[gtk::accessible::Property::Label(text)]);
    grid.attach(&entry, 1, row, 1, 1);
    entry
}

fn spin(grid: &gtk::Grid, row: i32, text: &str, min: f64, max: f64, step: f64) -> gtk::SpinButton {
    label(grid, row, text);
    let spin = gtk::SpinButton::with_range(min, max, step);
    spin.update_property(&[gtk::accessible::Property::Label(text)]);
    grid.attach(&spin, 1, row, 1, 1);
    spin
}

fn dropdown(grid: &gtk::Grid, row: i32, text: &str) -> gtk::DropDown {
    label(grid, row, text);
    let dropdown = gtk::DropDown::from_strings(&[]);
    dropdown.update_property(&[gtk::accessible::Property::Label(text)]);
    grid.attach(&dropdown, 1, row, 1, 1);
    dropdown
}

fn scheme_dropdown(grid: &gtk::Grid, row: i32, text: &str) -> gtk::DropDown {
    label(grid, row, text);
    let dropdown = gtk::DropDown::from_strings(&[
        "Default",
        "One Dark",
        "Solarized Dark",
        "Solarized Light",
        "Dracula",
        "Monokai",
        "Nord",
        "Gruvbox Dark",
        "Tokyo Night",
        "Campbell PowerShell",
    ]);
    dropdown.update_property(&[gtk::accessible::Property::Label(text)]);
    grid.attach(&dropdown, 1, row, 1, 1);
    dropdown
}
