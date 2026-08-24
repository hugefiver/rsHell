use std::collections::{BTreeMap, BTreeSet};

use gtk::prelude::*;
use relm4::{Component, ComponentController, ComponentSender, Controller, gtk};
use rshell_core::SessionId;

use crate::{
    PaneHostModel, PaneHostOutput, TerminalView, TerminalViewInit, TerminalViewMsg,
    terminal_geometry::terminal_font_metrics,
};

pub(crate) fn send_active_terminal(
    model: &PaneHostModel,
    terminals: &BTreeMap<SessionId, Controller<TerminalView>>,
    message: TerminalViewMsg,
    sender: &ComponentSender<crate::PaneHost>,
) {
    let Some(tab) = model.active_tab() else {
        let _ = sender.output(PaneHostOutput::Error("no active tab"));
        return;
    };
    let Some(pane) = model.active_pane(tab) else {
        let _ = sender.output(PaneHostOutput::Error("no active pane"));
        return;
    };
    let Some(session) = model.pane(pane).and_then(|pane| pane.session()) else {
        let _ = sender.output(PaneHostOutput::Error("active pane has no session"));
        return;
    };
    let Some(terminal) = terminals.get(&session) else {
        let _ = sender.output(PaneHostOutput::Error("active terminal unavailable"));
        return;
    };
    terminal.emit(message);
}

pub(crate) fn sync_terminals(
    model: &mut PaneHostModel,
    terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>,
    sender: &ComponentSender<crate::PaneHost>,
) {
    let panes = model
        .view_model()
        .workspace
        .tabs
        .iter()
        .flat_map(|tab| tab.pane_tree.pane_ids())
        .collect::<Vec<_>>();
    let desired = panes
        .iter()
        .filter_map(|pane| model.pane(*pane).and_then(|pane| pane.session()))
        .collect::<BTreeSet<_>>();
    terminals.retain(|session, _| desired.contains(session));
    for pane_id in panes {
        let Some(pane) = model.pane(pane_id) else {
            continue;
        };
        let Some(session) = pane.session() else {
            continue;
        };
        if terminals.contains_key(&session) {
            if let Some(frame) = pane.frame()
                && let Some(terminal) = terminals.get(&session)
            {
                terminal.emit(TerminalViewMsg::ApplyFrame(frame.clone()));
                model.observe_frame(frame);
            }
            continue;
        }
        let Some(profile) = pane.resolved_profile(model.view_model()) else {
            continue;
        };
        let controller = TerminalView::builder()
            .launch(TerminalViewInit {
                session,
                profile,
                metrics: terminal_font_metrics(),
            })
            .forward(sender.input_sender(), move |output| {
                crate::PaneHostMsg::Terminal(session, output)
            });
        if let Some(frame) = pane.frame() {
            controller.emit(TerminalViewMsg::ApplyFrame(frame.clone()));
            model.observe_frame(frame);
        }
        terminals.insert(session, controller);
    }
}

pub(crate) fn detach_terminals(terminals: &BTreeMap<SessionId, Controller<TerminalView>>) {
    for terminal in terminals.values() {
        let widget = terminal.widget();
        if let Some(parent) = widget.parent()
            && let Ok(container) = parent.downcast::<gtk::Box>()
        {
            container.remove(widget);
        }
    }
}
