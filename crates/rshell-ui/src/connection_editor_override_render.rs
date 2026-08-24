use gtk::prelude::*;
use rshell_core::{ColorScheme, TerminalOverrides, TerminalSettingsV1};

use crate::{
    connection_editor_override_widgets::{OverrideControl, TerminalOverrideWidgets},
    key_binding_text::display_bindings,
};

pub(crate) fn render_terminal_overrides(
    widgets: &TerminalOverrideWidgets,
    values: &TerminalOverrides,
    base: &TerminalSettingsV1,
    pending: bool,
) {
    render_entry(
        &widgets.terminal_type,
        values.terminal_type.as_deref(),
        &base.terminal_type,
        pending,
    );
    render_spin(
        &widgets.initial_cols,
        values.initial_cols.map(f64::from),
        f64::from(base.initial_cols),
        pending,
    );
    render_spin(
        &widgets.initial_rows,
        values.initial_rows.map(f64::from),
        f64::from(base.initial_rows),
        pending,
    );
    render_spin(
        &widgets.scrollback,
        values.scrollback_lines.map(|value| value as f64),
        base.scrollback_lines as f64,
        pending,
    );
    render_entry(
        &widgets.font_family,
        values.font_family.as_deref(),
        &base.font_family,
        pending,
    );
    render_spin(
        &widgets.font_size,
        values.font_size.map(f64::from),
        f64::from(base.font_size),
        pending,
    );
    widgets.color_scheme.syncing.set(true);
    set_state(
        &widgets.color_scheme,
        values.color_scheme.is_none(),
        pending,
    );
    set_selected(
        &widgets.color_scheme.value,
        scheme_index(values.color_scheme.unwrap_or(base.color_scheme)),
    );
    widgets.color_scheme.syncing.set(false);
    let displayed_bindings =
        display_bindings(values.key_bindings.as_deref().unwrap_or(&base.key_bindings));
    render_entry(
        &widgets.key_bindings,
        values
            .key_bindings
            .as_ref()
            .map(|_| displayed_bindings.as_str()),
        &display_bindings(&base.key_bindings),
        pending,
    );
    render_bool(
        &widgets.left_alt,
        values.left_alt_as_meta,
        base.left_alt_as_meta,
        pending,
    );
    render_bool(
        &widgets.right_alt,
        values.right_alt_as_meta,
        base.right_alt_as_meta,
        pending,
    );
    render_bool(
        &widgets.csi_u,
        values.enable_csi_u,
        base.enable_csi_u,
        pending,
    );
    render_bool(
        &widgets.kitty,
        values.enable_kitty_keyboard,
        base.enable_kitty_keyboard,
        pending,
    );
    render_bool(
        &widgets.mouse,
        values.mouse_reporting,
        base.mouse_reporting,
        pending,
    );
    render_bool(
        &widgets.scroll_output,
        values.scroll_on_output,
        base.scroll_on_output,
        pending,
    );
    render_bool(
        &widgets.scroll_key,
        values.scroll_on_keypress,
        base.scroll_on_keypress,
        pending,
    );
    render_entry(
        &widgets.answerback,
        values.answerback.as_deref(),
        &base.answerback,
        pending,
    );
    widgets
        .clear
        .set_sensitive(!pending && values.explicit_field_count() > 0);
}

fn render_entry(
    control: &OverrideControl<gtk::Entry>,
    explicit: Option<&str>,
    base: &str,
    pending: bool,
) {
    control.syncing.set(true);
    set_state(control, explicit.is_none(), pending);
    let value = explicit.unwrap_or(base);
    if control.value.text().as_str() != value {
        control.value.set_text(value);
    }
    control.syncing.set(false);
}

fn render_spin(
    control: &OverrideControl<gtk::SpinButton>,
    explicit: Option<f64>,
    base: f64,
    pending: bool,
) {
    control.syncing.set(true);
    set_state(control, explicit.is_none(), pending);
    let value = explicit.unwrap_or(base);
    if (control.value.value() - value).abs() > f64::EPSILON {
        control.value.set_value(value);
    }
    control.syncing.set(false);
}

fn render_bool(
    control: &OverrideControl<gtk::CheckButton>,
    explicit: Option<bool>,
    base: bool,
    pending: bool,
) {
    control.syncing.set(true);
    set_state(control, explicit.is_none(), pending);
    let value = explicit.unwrap_or(base);
    if control.value.is_active() != value {
        control.value.set_active(value);
    }
    control.syncing.set(false);
}

fn set_state<T: IsA<gtk::Widget>>(control: &OverrideControl<T>, inherited: bool, pending: bool) {
    control
        .value
        .upcast_ref::<gtk::Widget>()
        .set_sensitive(!inherited && !pending);
    control.inherit.set_sensitive(!pending);
    if control.inherit.is_active() != inherited {
        control.inherit.set_active(inherited);
    }
}

fn set_selected(dropdown: &gtk::DropDown, selected: u32) {
    if dropdown.selected() != selected {
        dropdown.set_selected(selected);
    }
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
