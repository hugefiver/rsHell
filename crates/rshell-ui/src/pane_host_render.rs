use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use gtk::prelude::*;
use relm4::{ComponentController, Controller, gtk};
use rshell_core::{PaneId, SessionId, SplitAxis};

use crate::{
    PaneAction, PaneHostMsg, PanePageKind, PaneProjection, ProductIcon, TerminalView,
    pane_host::PaneHost,
};

pub(crate) fn render_projection(
    projection: &PaneProjection,
    active: Option<PaneId>,
    terminals: &BTreeMap<SessionId, Controller<TerminalView>>,
    sender: &relm4::ComponentSender<PaneHost>,
) -> gtk::Widget {
    match projection {
        PaneProjection::Leaf(pane) => render_leaf(pane, active, terminals, sender).upcast(),
        PaneProjection::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let orientation = match axis {
                SplitAxis::Horizontal => gtk::Orientation::Horizontal,
                SplitAxis::Vertical => gtk::Orientation::Vertical,
            };
            let split = gtk::Paned::new(orientation);
            split.set_start_child(Some(&render_projection(first, active, terminals, sender)));
            split.set_end_child(Some(&render_projection(second, active, terminals, sender)));
            split.set_resize_start_child(true);
            split.set_resize_end_child(true);
            project_ratio(&split, *ratio);
            split.upcast()
        }
    }
}

fn render_leaf(
    pane: &crate::SessionPaneViewModel,
    active: Option<PaneId>,
    terminals: &BTreeMap<SessionId, Controller<TerminalView>>,
    sender: &relm4::ComponentSender<PaneHost>,
) -> gtk::Box {
    let surface = gtk::Box::new(gtk::Orientation::Vertical, 0);
    surface.add_css_class("pane-surface");
    surface.set_hexpand(true);
    surface.set_vexpand(true);
    if active == Some(pane.pane()) {
        surface.add_css_class("active-pane");
    }
    let focus = gtk::EventControllerFocus::new();
    let pane_id = pane.pane();
    let input = sender.input_sender().clone();
    focus.connect_enter(move |_| {
        let _ = input.send(PaneHostMsg::ActivatePane(pane_id));
    });
    surface.add_controller(focus);
    let click = gtk::GestureClick::new();
    let input = sender.input_sender().clone();
    click.connect_pressed(move |_, _, _, _| {
        let _ = input.send(PaneHostMsg::ActivatePane(pane_id));
    });
    surface.add_controller(click);

    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    toolbar.add_css_class("pane-command-row");
    let status = gtk::Label::new(Some(pane.status_label()));
    status.add_css_class("pane-state-label");
    status.set_halign(gtk::Align::Start);
    status.set_hexpand(true);
    toolbar.append(&status);
    if matches!(pane.page(), PanePageKind::Terminal | PanePageKind::Pending) {
        for action in pane.actions() {
            toolbar.append(&action_button(pane_id, action, sender, true));
        }
    }
    surface.append(&toolbar);

    match pane.page() {
        PanePageKind::Terminal => {
            if let Some(terminal) = pane.session().and_then(|id| terminals.get(&id)) {
                surface.append(terminal.widget());
            } else {
                surface.append(&status_page("Terminal unavailable"));
            }
        }
        PanePageKind::Pending => surface.append(&status_page(pane.status_label())),
        PanePageKind::Status | PanePageKind::Error | PanePageKind::Unavailable => {
            let page = status_page(pane.status_label());
            let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            actions.add_css_class("pane-error-actions");
            actions.set_halign(gtk::Align::Center);
            for action in pane.actions() {
                actions.append(&action_button(pane_id, action, sender, false));
            }
            page.append(&actions);
            surface.append(&page);
        }
    }
    surface
}

fn status_page(label: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.add_css_class("pane-status-page");
    page.set_halign(gtk::Align::Fill);
    page.set_valign(gtk::Align::Fill);
    page.set_hexpand(true);
    page.set_vexpand(true);
    let label = gtk::Label::new(Some(label));
    label.add_css_class("pane-state-label");
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);
    label.set_vexpand(true);
    page.append(&label);
    page
}

fn action_button(
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
    if icon_only {
        button.set_child(Some(&icon.image().expect("embedded pane action icon")));
    } else {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        content.append(&icon.image().expect("embedded pane action icon"));
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
        PaneAction::SplitHorizontal => (ProductIcon::SplitHorizontal, "Split horizontally"),
        PaneAction::SplitVertical => (ProductIcon::SplitVertical, "Split vertically"),
        PaneAction::Reconnect => (ProductIcon::Retry, "Reconnect session"),
        PaneAction::Retry => (ProductIcon::Retry, "Retry"),
        PaneAction::EditConnection => (ProductIcon::Edit, "Edit Connection"),
        PaneAction::CopyDiagnostics => (ProductIcon::CopyDiagnostics, "Copy Diagnostics"),
        PaneAction::Close => (ProductIcon::CloseTab, "Close"),
    }
}

fn project_ratio(split: &gtk::Paned, ratio: f32) {
    let positioned = Rc::new(Cell::new(false));
    split.connect_notify_local(Some("max-position"), move |split, _| {
        let max = split.max_position();
        if !positioned.get() && max > 0 {
            split.set_position((max as f32 * ratio).round() as i32);
            positioned.set(true);
        }
    });
}
