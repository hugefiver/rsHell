use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::ColorScheme;

use crate::{
    SettingsBoolField, SettingsTextField, SettingsWindow, SettingsWindowMsg,
    key_binding_text::display_bindings, settings_window_widgets::SettingsWindowWidgets,
};

pub fn render_settings(
    model: &SettingsWindow,
    root: &gtk::Box,
    widgets: &mut SettingsWindowWidgets,
) {
    root.set_visible(model.visible);
    if !model.visible {
        return;
    }
    let profile_names = model
        .view
        .profiles()
        .iter()
        .map(|profile| profile.name.clone())
        .collect::<Vec<_>>();
    if widgets.profile_names != profile_names {
        let names = profile_names.iter().map(String::as_str);
        widgets.profile.set_model(Some(&string_list(names.clone())));
        widgets.default_profile.set_model(Some(&string_list(names)));
        widgets.profile_names = profile_names;
    }
    let Some(profile) = model.view.active_profile() else {
        return;
    };
    let index = model
        .view
        .profiles()
        .iter()
        .position(|item| item.id == profile.id)
        .unwrap_or_default() as u32;
    if widgets.profile.selected() != index {
        widgets.profile.set_selected(index);
    }
    set_entry(&widgets.name, &profile.name);
    set_entry(&widgets.terminal_type, &profile.settings.terminal_type);
    set_spin(&widgets.cols, profile.settings.initial_cols.into());
    set_spin(&widgets.rows, profile.settings.initial_rows.into());
    set_spin(
        &widgets.scrollback,
        profile.settings.scrollback_lines as f64,
    );
    set_entry(&widgets.font_family, &profile.settings.font_family);
    set_spin(&widgets.font_size, profile.settings.font_size.into());
    let profile_scheme = scheme_index(profile.settings.color_scheme);
    if widgets.scheme.selected() != profile_scheme {
        widgets.scheme.set_selected(profile_scheme);
    }
    set_entry(
        &widgets.bindings,
        &display_bindings(&profile.settings.key_bindings),
    );
    let values = [
        profile.settings.left_alt_as_meta,
        profile.settings.right_alt_as_meta,
        profile.settings.enable_csi_u,
        profile.settings.enable_kitty_keyboard,
        profile.settings.mouse_reporting,
        profile.settings.scroll_on_output,
        profile.settings.scroll_on_keypress,
    ];
    for (toggle, active) in widgets.toggles.iter().zip(values) {
        if toggle.is_active() != active {
            toggle.set_active(active);
        }
    }
    set_entry(&widgets.answerback, &profile.settings.answerback);
    let app = model.view.app_settings();
    let default_index = model
        .view
        .profiles()
        .iter()
        .position(|profile| profile.id == app.default_terminal_profile)
        .unwrap_or_default() as u32;
    if widgets.default_profile.selected() != default_index {
        widgets.default_profile.set_selected(default_index);
    }
    let app_scheme = scheme_index(app.color_scheme);
    if widgets.app_scheme.selected() != app_scheme {
        widgets.app_scheme.set_selected(app_scheme);
    }
    set_entry(&widgets.app_bindings, &display_bindings(&app.key_bindings));
    widgets
        .save_profile
        .set_sensitive(model.view.profile_dirty() && !model.view.pending());
    widgets
        .save_app
        .set_sensitive(model.view.app_dirty() && !model.view.pending());
    widgets.error.set_label(model.view.error().unwrap_or(""));
    widgets.error.set_visible(model.view.error().is_some());
}

pub fn connect_settings_inputs(
    widgets: &SettingsWindowWidgets,
    sender: &ComponentSender<SettingsWindow>,
) {
    connect_dropdown(&widgets.profile, sender, SettingsWindowMsg::SelectProfile);
    connect_entry(&widgets.name, sender, SettingsTextField::ProfileName);
    connect_entry(
        &widgets.terminal_type,
        sender,
        SettingsTextField::TerminalType,
    );
    connect_entry(&widgets.font_family, sender, SettingsTextField::FontFamily);
    connect_entry(
        &widgets.bindings,
        sender,
        SettingsTextField::ProfileBindings,
    );
    connect_entry(&widgets.answerback, sender, SettingsTextField::Answerback);
    connect_entry(
        &widgets.app_bindings,
        sender,
        SettingsTextField::AppBindings,
    );
    connect_dropdown(&widgets.scheme, sender, SettingsWindowMsg::Scheme);
    connect_dropdown(
        &widgets.default_profile,
        sender,
        SettingsWindowMsg::DefaultProfile,
    );
    connect_dropdown(&widgets.app_scheme, sender, SettingsWindowMsg::AppScheme);
    let input = sender.input_sender().clone();
    widgets.cols.connect_value_changed(move |cols| {
        let _ = input.send(SettingsWindowMsg::Columns(cols.value() as u16));
    });
    let input = sender.input_sender().clone();
    widgets.rows.connect_value_changed(move |rows| {
        let _ = input.send(SettingsWindowMsg::Rows(rows.value() as u16));
    });
    connect_spin(&widgets.scrollback, sender, |value| {
        SettingsWindowMsg::Scrollback(value as usize)
    });
    connect_spin(&widgets.font_size, sender, |value| {
        SettingsWindowMsg::FontSize(value as f32)
    });
    let fields = [
        SettingsBoolField::LeftAltMeta,
        SettingsBoolField::RightAltMeta,
        SettingsBoolField::CsiU,
        SettingsBoolField::KittyKeyboard,
        SettingsBoolField::MouseReporting,
        SettingsBoolField::ScrollOnOutput,
        SettingsBoolField::ScrollOnKeypress,
    ];
    for (toggle, field) in widgets.toggles.iter().zip(fields) {
        let input = sender.input_sender().clone();
        toggle.connect_toggled(move |toggle| {
            let _ = input.send(SettingsWindowMsg::Bool(field, toggle.is_active()));
        });
    }
}

fn connect_entry(
    entry: &gtk::Entry,
    sender: &ComponentSender<SettingsWindow>,
    field: SettingsTextField,
) {
    let input = sender.input_sender().clone();
    entry.connect_changed(move |entry| {
        let _ = input.send(SettingsWindowMsg::Text(field, entry.text().into()));
    });
}

fn connect_dropdown(
    dropdown: &gtk::DropDown,
    sender: &ComponentSender<SettingsWindow>,
    message: fn(u32) -> SettingsWindowMsg,
) {
    let input = sender.input_sender().clone();
    dropdown.connect_selected_notify(move |dropdown| {
        let _ = input.send(message(dropdown.selected()));
    });
}

fn connect_spin(
    spin: &gtk::SpinButton,
    sender: &ComponentSender<SettingsWindow>,
    message: fn(f64) -> SettingsWindowMsg,
) {
    let input = sender.input_sender().clone();
    spin.connect_value_changed(move |spin| {
        let _ = input.send(message(spin.value()));
    });
}

fn string_list<'a>(values: impl IntoIterator<Item = &'a str>) -> gtk::StringList {
    let values = values.into_iter().collect::<Vec<_>>();
    gtk::StringList::new(&values)
}

fn scheme_index(scheme: ColorScheme) -> u32 {
    match scheme {
        ColorScheme::Default => 0,
        ColorScheme::OneDark => 1,
        ColorScheme::SolarizedDark => 2,
        ColorScheme::SolarizedLight => 3,
        ColorScheme::Dracula => 4,
        ColorScheme::Monokai => 5,
        ColorScheme::Nord => 6,
        ColorScheme::GruvboxDark => 7,
        ColorScheme::TokyoNight => 8,
        ColorScheme::CampbellPowershell => 9,
    }
}

fn set_entry(entry: &gtk::Entry, value: &str) {
    if entry.text().as_str() != value {
        entry.set_text(value);
    }
}

fn set_spin(spin: &gtk::SpinButton, value: f64) {
    if (spin.value() - value).abs() > f64::EPSILON {
        spin.set_value(value);
    }
}
