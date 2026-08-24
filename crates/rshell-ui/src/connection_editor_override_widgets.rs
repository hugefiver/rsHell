use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use std::{cell::Cell, rc::Rc};

use crate::{ConnectionEditor, connection_editor_override_bindings::connect_override_widgets};

pub(crate) struct OverrideControl<T> {
    pub(crate) inherit: gtk::CheckButton,
    pub(crate) value: T,
    pub(crate) syncing: Rc<Cell<bool>>,
}

pub(crate) struct TerminalOverrideWidgets {
    pub(crate) terminal_type: OverrideControl<gtk::Entry>,
    pub(crate) initial_cols: OverrideControl<gtk::SpinButton>,
    pub(crate) initial_rows: OverrideControl<gtk::SpinButton>,
    pub(crate) scrollback: OverrideControl<gtk::SpinButton>,
    pub(crate) font_family: OverrideControl<gtk::Entry>,
    pub(crate) font_size: OverrideControl<gtk::SpinButton>,
    pub(crate) color_scheme: OverrideControl<gtk::DropDown>,
    pub(crate) key_bindings: OverrideControl<gtk::Entry>,
    pub(crate) left_alt: OverrideControl<gtk::CheckButton>,
    pub(crate) right_alt: OverrideControl<gtk::CheckButton>,
    pub(crate) csi_u: OverrideControl<gtk::CheckButton>,
    pub(crate) kitty: OverrideControl<gtk::CheckButton>,
    pub(crate) mouse: OverrideControl<gtk::CheckButton>,
    pub(crate) scroll_output: OverrideControl<gtk::CheckButton>,
    pub(crate) scroll_key: OverrideControl<gtk::CheckButton>,
    pub(crate) answerback: OverrideControl<gtk::Entry>,
    pub(crate) clear: gtk::Button,
}

impl TerminalOverrideWidgets {
    pub(crate) fn build(
        grid: &gtk::Grid,
        row: &mut i32,
        sender: &ComponentSender<ConnectionEditor>,
    ) -> Self {
        let terminal_type = add_row(grid, row, "Terminal type", entry());
        let initial_cols = add_row(grid, row, "Columns", spin(1.0, 999.0, 1.0));
        let initial_rows = add_row(grid, row, "Rows", spin(1.0, 999.0, 1.0));
        let scrollback = add_row(grid, row, "Scrollback", spin(100.0, 1_000_000.0, 100.0));
        let font_family = add_row(grid, row, "Font family", entry());
        let font_size = add_row(grid, row, "Font size", spin(6.0, 72.0, 0.5));
        let color_scheme = add_row(
            grid,
            row,
            "Color scheme",
            gtk::DropDown::from_strings(&[
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
            ]),
        );
        let key_bindings = add_row(grid, row, "Key bindings", entry());
        key_bindings
            .value
            .set_placeholder_text(Some("Ctrl+K=clear_scrollback; F2=new_tab"));
        let left_alt = add_row(grid, row, "Left Alt as Meta", toggle());
        let right_alt = add_row(grid, row, "Right Alt as Meta", toggle());
        let csi_u = add_row(grid, row, "CSI-u keyboard", toggle());
        let kitty = add_row(grid, row, "Kitty keyboard", toggle());
        let mouse = add_row(grid, row, "Mouse reporting", toggle());
        let scroll_output = add_row(grid, row, "Scroll on output", toggle());
        let scroll_key = add_row(grid, row, "Scroll on keypress", toggle());
        let answerback = add_row(grid, row, "Answerback", entry());
        let clear = gtk::Button::with_label("Inherit all terminal settings");
        clear.update_property(&[gtk::accessible::Property::Label(
            "Clear all terminal overrides",
        )]);
        clear.set_halign(gtk::Align::Start);
        grid.attach(&clear, 1, *row, 2, 1);
        *row += 1;
        let widgets = Self {
            terminal_type,
            initial_cols,
            initial_rows,
            scrollback,
            font_family,
            font_size,
            color_scheme,
            key_bindings,
            left_alt,
            right_alt,
            csi_u,
            kitty,
            mouse,
            scroll_output,
            scroll_key,
            answerback,
            clear,
        };
        connect_override_widgets(&widgets, sender);
        widgets
    }
}

fn add_row<T>(grid: &gtk::Grid, row: &mut i32, text: &str, value: T) -> OverrideControl<T>
where
    T: IsA<gtk::Widget> + IsA<gtk::Accessible>,
{
    let label = gtk::Label::new(Some(text));
    label.set_halign(gtk::Align::End);
    label.set_valign(gtk::Align::Center);
    label.set_mnemonic_widget(Some(&value));
    let inherit = gtk::CheckButton::with_label("Inherit");
    inherit.update_property(&[gtk::accessible::Property::Label(&format!("Inherit {text}"))]);
    value.update_property(&[gtk::accessible::Property::Label(text)]);
    value.upcast_ref::<gtk::Widget>().set_hexpand(true);
    grid.attach(&label, 0, *row, 1, 1);
    grid.attach(&inherit, 1, *row, 1, 1);
    grid.attach(&value, 2, *row, 1, 1);
    *row += 1;
    OverrideControl {
        inherit,
        value,
        syncing: Rc::new(Cell::new(false)),
    }
}

fn entry() -> gtk::Entry {
    gtk::Entry::new()
}

fn spin(min: f64, max: f64, step: f64) -> gtk::SpinButton {
    gtk::SpinButton::with_range(min, max, step)
}

fn toggle() -> gtk::CheckButton {
    gtk::CheckButton::with_label("Enabled")
}
