use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use gtk::prelude::*;
use relm4::{ComponentController, Controller, gtk};
use rshell_core::{PaneId, SessionId, SplitAxis};

use crate::{
    PaneAction, PaneHostMsg, PanePageKind, PaneProjection, TerminalView,
    pane_action_widgets::{action_button, action_region},
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
    surface.set_size_request(1, 1);
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
        let actions = pane
            .actions()
            .into_iter()
            .filter(|action| *action != PaneAction::ResetDisplay)
            .collect::<Vec<_>>();
        toolbar.append(&action_region(&surface, pane_id, actions, sender, true));
    }
    surface.append(&toolbar);

    match pane.page() {
        PanePageKind::Terminal => {
            if let Some(terminal) = pane.session().and_then(|id| terminals.get(&id)) {
                surface.append(terminal.widget());
            } else {
                surface.append(&status_page("Terminal unavailable"));
            }
            if pane.recovery_notice().is_some() {
                surface.append(&recovery_notice(pane_id, sender));
            }
        }
        PanePageKind::Pending => surface.append(&status_page(pane.status_label())),
        PanePageKind::Status | PanePageKind::Error | PanePageKind::Unavailable => {
            let page = status_page(pane.status_label());
            let actions = action_region(&surface, pane_id, pane.actions(), sender, false);
            actions.add_css_class("pane-error-actions");
            actions.set_halign(gtk::Align::Center);
            page.append(&actions);
            surface.append(&page);
        }
    }
    surface
}

fn recovery_notice(pane: PaneId, sender: &relm4::ComponentSender<PaneHost>) -> gtk::Box {
    let notice = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    notice.add_css_class("display-recovery-notice");
    let label = gtk::Label::new(Some("Display mode not restored"));
    label.set_halign(gtk::Align::Start);
    label.set_hexpand(true);
    notice.append(&label);
    notice.append(&action_button(pane, PaneAction::ResetDisplay, sender, true));
    notice
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
