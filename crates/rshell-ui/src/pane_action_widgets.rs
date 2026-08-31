use std::{cell::Cell, rc::Rc};

use gtk::prelude::*;
use relm4::gtk;
use rshell_core::PaneId;

use crate::{
    IconRenderRequest, PaneAction, PaneActionLayout, PaneHostMsg, ProductIcon, pane_host::PaneHost,
};

pub(crate) fn action_region(
    width_source: &gtk::Box,
    pane: PaneId,
    actions: Vec<PaneAction>,
    sender: &relm4::ComponentSender<PaneHost>,
    icon_only: bool,
) -> gtk::Box {
    let region = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    region.add_css_class("pane-action-region");
    render_region(
        &region,
        PaneActionLayout::for_width(&actions, 600),
        pane,
        sender,
        icon_only,
    );
    let previous_band = Rc::new(Cell::new(width_band(600)));
    let region_clone = region.clone();
    let sender = sender.clone();
    width_source.add_tick_callback(move |source, _| {
        let width = source.width();
        let band = width_band(width);
        if previous_band.replace(band) != band {
            render_region(
                &region_clone,
                PaneActionLayout::for_width(&actions, width),
                pane,
                &sender,
                icon_only,
            );
        }
        gtk::glib::ControlFlow::Continue
    });
    region
}

fn render_region(
    region: &gtk::Box,
    layout: PaneActionLayout,
    pane: PaneId,
    sender: &relm4::ComponentSender<PaneHost>,
    icon_only: bool,
) {
    clear_box(region);
    for action in layout.visible {
        region.append(&action_button(pane, action, sender, icon_only));
    }
    if !layout.overflow.is_empty() {
        region.append(&overflow_button(pane, layout.overflow, sender));
    }
}

fn overflow_button(
    pane: PaneId,
    actions: Vec<PaneAction>,
    sender: &relm4::ComponentSender<PaneHost>,
) -> gtk::MenuButton {
    let rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let popover = gtk::Popover::builder().child(&rows).build();
    popover.add_css_class("pane-action-overflow");
    for action in actions {
        let row = action_button(pane, action, sender, false);
        row.add_css_class("pane-action-overflow-row");
        let popover = popover.clone();
        row.connect_clicked(move |_| popover.popdown());
        rows.append(&row);
    }
    let button = gtk::MenuButton::new();
    button.add_css_class("pane-action-overflow");
    button.set_tooltip_text(Some("More pane actions"));
    button.update_property(&[gtk::accessible::Property::Label("More pane actions")]);
    button.set_child(Some(
        &ProductIcon::More
            .image(IconRenderRequest::for_widget(16, &button))
            .expect("embedded pane-overflow icon"),
    ));
    button.set_popover(Some(&popover));
    button
}

pub(crate) fn action_button(
    pane: PaneId,
    action: PaneAction,
    sender: &relm4::ComponentSender<PaneHost>,
    icon_only: bool,
) -> gtk::Button {
    let (icon, label) = action_metadata(action);
    let button = gtk::Button::new();
    button.add_css_class("pane-action-btn");
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    let request = IconRenderRequest::for_widget(16, &button);
    if icon_only {
        button.set_child(Some(
            &icon.image(request).expect("embedded pane action icon"),
        ));
    } else {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        content.append(&icon.image(request).expect("embedded pane action icon"));
        content.append(&gtk::Label::new(Some(label)));
        button.set_child(Some(&content));
    }
    let input = sender.input_sender().clone();
    button.connect_clicked(move |_| {
        let _ = input.send(PaneHostMsg::Action { pane, action });
    });
    button
}

fn action_metadata(action: PaneAction) -> (ProductIcon, &'static str) {
    match action {
        PaneAction::ResetDisplay => (ProductIcon::Retry, "Reset display"),
        PaneAction::SplitHorizontal => (ProductIcon::SplitHorizontal, "Split horizontally"),
        PaneAction::SplitVertical => (ProductIcon::SplitVertical, "Split vertically"),
        PaneAction::Reconnect => (ProductIcon::Retry, "Reconnect session"),
        PaneAction::Retry => (ProductIcon::Retry, "Retry"),
        PaneAction::EditConnection => (ProductIcon::Edit, "Edit Connection"),
        PaneAction::CopyDiagnostics => (ProductIcon::CopyDiagnostics, "Copy Diagnostics"),
        PaneAction::Close => (ProductIcon::CloseTab, "Close"),
    }
}

const fn width_band(width: i32) -> u8 {
    match width {
        ..=159 => 0,
        160..=239 => 1,
        240..=479 => 2,
        _ => 3,
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
