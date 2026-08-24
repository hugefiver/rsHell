use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::ColorScheme;

pub use crate::settings_window_message::{
    SettingsBoolField, SettingsTextField, SettingsWindowInit, SettingsWindowMsg,
    SettingsWindowOutput,
};
use crate::{
    SettingsViewModel, key_binding_text::parse_bindings, settings_window_render::render_settings,
    settings_window_widgets::SettingsWindowWidgets,
};

pub struct SettingsWindow {
    pub(crate) view: SettingsViewModel,
    pub(crate) visible: bool,
}

impl SimpleComponent for SettingsWindow {
    type Init = SettingsWindowInit;
    type Input = SettingsWindowMsg;
    type Output = SettingsWindowOutput;
    type Root = gtk::Box;
    type Widgets = SettingsWindowWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("settings-window");
        root.add_css_class("content-dialog");
        root.set_width_request(620);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Fill);
        root.set_margin_top(40);
        root.set_margin_bottom(40);
        root.set_visible(false);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            view: SettingsViewModel::new(init.settings, init.profiles),
            visible: false,
        };
        let mut widgets = SettingsWindowWidgets::build(&root, &sender);
        connect_inputs(&widgets, &sender);
        connect_keys(&root, &sender);
        render_settings(&model, &root, &mut widgets);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SettingsWindowMsg::Open => self.visible = true,
            SettingsWindowMsg::Close => {
                self.visible = false;
                let _ = sender.output(SettingsWindowOutput::Closed);
            }
            SettingsWindowMsg::SelectProfile(index) => {
                self.view.select_profile(index as usize);
            }
            SettingsWindowMsg::Text(field, value) => self.set_text(field, value),
            SettingsWindowMsg::Columns(cols) => {
                if let Some(profile) = self.view.active_profile_mut() {
                    profile.settings.initial_cols = cols;
                }
            }
            SettingsWindowMsg::Rows(rows) => {
                if let Some(profile) = self.view.active_profile_mut() {
                    profile.settings.initial_rows = rows;
                }
            }
            SettingsWindowMsg::Scrollback(value) => {
                if let Some(profile) = self.view.active_profile_mut() {
                    profile.settings.scrollback_lines = value;
                }
            }
            SettingsWindowMsg::FontSize(value) => {
                if let Some(profile) = self.view.active_profile_mut() {
                    profile.settings.font_size = value;
                }
            }
            SettingsWindowMsg::Scheme(index) => {
                if let Some(profile) = self.view.active_profile_mut() {
                    profile.settings.color_scheme = scheme(index);
                }
            }
            SettingsWindowMsg::Bool(field, value) => self.set_bool(field, value),
            SettingsWindowMsg::DefaultProfile(index) => {
                if let Some(profile) = self.view.profiles().get(index as usize) {
                    self.view.app_settings_mut().default_terminal_profile = profile.id;
                }
            }
            SettingsWindowMsg::AppScheme(index) => {
                self.view.app_settings_mut().color_scheme = scheme(index);
            }
            SettingsWindowMsg::SaveProfile => match self.view.save_profile_command() {
                Ok(command) => {
                    let _ = sender.output(SettingsWindowOutput::Command(Box::new(command)));
                }
                Err(error) => self.view.rejected(error.to_string()),
            },
            SettingsWindowMsg::SaveApp => match self.view.save_settings_command() {
                Ok(command) => {
                    let _ = sender.output(SettingsWindowOutput::Command(Box::new(command)));
                }
                Err(error) => self.view.rejected(error.to_string()),
            },
            SettingsWindowMsg::ProfilesAccepted(profiles) => self.view.accept_profiles(profiles),
            SettingsWindowMsg::SettingsAccepted(settings) => self.view.accept_settings(settings),
            SettingsWindowMsg::OperationFailed(context) => self.view.failed(context),
            SettingsWindowMsg::CommandRejected(error) => self.view.rejected(error.to_string()),
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        if let Some(root) = widgets.error.ancestor(gtk::Box::static_type())
            && let Ok(root) = root.downcast::<gtk::Box>()
        {
            render_settings(self, &root, widgets);
        }
    }
}

impl SettingsWindow {
    fn set_text(&mut self, field: SettingsTextField, value: String) {
        if field == SettingsTextField::AppBindings {
            match parse_bindings(&value) {
                Ok(bindings) => self.view.app_settings_mut().key_bindings = bindings,
                Err(error) => self.view.rejected(error.into()),
            }
            return;
        }
        let Some(profile) = self.view.active_profile_mut() else {
            return;
        };
        match field {
            SettingsTextField::ProfileName => profile.name = value,
            SettingsTextField::TerminalType => profile.settings.terminal_type = value,
            SettingsTextField::FontFamily => profile.settings.font_family = value,
            SettingsTextField::Answerback => profile.settings.answerback = value,
            SettingsTextField::ProfileBindings => match parse_bindings(&value) {
                Ok(bindings) => profile.settings.key_bindings = bindings,
                Err(error) => self.view.rejected(error.into()),
            },
            SettingsTextField::AppBindings => unreachable!(),
        }
    }

    fn set_bool(&mut self, field: SettingsBoolField, value: bool) {
        let Some(profile) = self.view.active_profile_mut() else {
            return;
        };
        match field {
            SettingsBoolField::LeftAltMeta => profile.settings.left_alt_as_meta = value,
            SettingsBoolField::RightAltMeta => profile.settings.right_alt_as_meta = value,
            SettingsBoolField::CsiU => profile.settings.enable_csi_u = value,
            SettingsBoolField::KittyKeyboard => profile.settings.enable_kitty_keyboard = value,
            SettingsBoolField::MouseReporting => profile.settings.mouse_reporting = value,
            SettingsBoolField::ScrollOnOutput => profile.settings.scroll_on_output = value,
            SettingsBoolField::ScrollOnKeypress => profile.settings.scroll_on_keypress = value,
        }
    }
}

pub(crate) fn scheme(index: u32) -> ColorScheme {
    match index {
        1 => ColorScheme::OneDark,
        2 => ColorScheme::SolarizedDark,
        3 => ColorScheme::SolarizedLight,
        4 => ColorScheme::Dracula,
        5 => ColorScheme::Monokai,
        6 => ColorScheme::Nord,
        7 => ColorScheme::GruvboxDark,
        8 => ColorScheme::TokyoNight,
        9 => ColorScheme::CampbellPowershell,
        _ => ColorScheme::Default,
    }
}

fn connect_inputs(widgets: &SettingsWindowWidgets, sender: &ComponentSender<SettingsWindow>) {
    crate::settings_window_render::connect_settings_inputs(widgets, sender);
}

fn connect_keys(root: &gtk::Box, sender: &ComponentSender<SettingsWindow>) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed({
        let sender = sender.clone();
        move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                sender.input(SettingsWindowMsg::Close);
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        }
    });
    root.add_controller(keys);
}
