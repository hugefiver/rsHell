use std::collections::{BTreeMap, BTreeSet};

use gtk::prelude::*;
use relm4::{Component, ComponentController, ComponentSender, Controller, gtk};
use rshell_core::SessionId;

use crate::{
    FontMetricEnvironment, FontMetricsService, MetricsChange, PaneHostModel, PaneHostOutput,
    PanePageKind, TerminalView, TerminalViewInit, TerminalViewMsg,
};

pub(crate) fn send_active_terminal(
    model: &PaneHostModel,
    terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>,
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
    let Some(pane) = model.pane(pane) else {
        let _ = sender.output(PaneHostOutput::Error("active pane is unavailable"));
        return;
    };
    if pane.page() != PanePageKind::Terminal {
        let _ = sender.output(PaneHostOutput::Error("active terminal unavailable"));
        return;
    }
    let Some(session) = pane.session() else {
        let _ = sender.output(PaneHostOutput::Error("active pane has no session"));
        return;
    };
    let Some(terminal) = terminals.get(&session) else {
        let _ = sender.output(PaneHostOutput::Error("active terminal unavailable"));
        return;
    };
    if !send_terminal_message(terminal, message) {
        terminals.remove(&session);
        let _ = sender.output(PaneHostOutput::Error("active terminal unavailable"));
    }
}

pub(crate) fn send_terminal_message(
    terminal: &Controller<TerminalView>,
    message: TerminalViewMsg,
) -> bool {
    terminal.sender().send(message).is_ok()
}

pub(crate) fn sync_terminals(
    model: &mut PaneHostModel,
    terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>,
    metric_widget: &impl IsA<gtk::Widget>,
    sender: &ComponentSender<crate::PaneHost>,
) -> BTreeSet<SessionId> {
    let mut replaced = BTreeSet::new();
    let panes = model
        .active_tab()
        .and_then(|active| {
            model
                .view_model()
                .workspace
                .tabs
                .iter()
                .find(|tab| tab.id == active)
        })
        .map(|tab| tab.pane_tree.pane_ids())
        .unwrap_or_default();
    let desired = panes
        .iter()
        .filter_map(|pane_id| {
            let pane = model.pane(*pane_id)?;
            (pane.page() == PanePageKind::Terminal)
                .then_some(pane.session())
                .flatten()
        })
        .collect::<BTreeSet<_>>();
    terminals.retain(|session, _| desired.contains(session));
    for pane_id in panes {
        let Some(pane) = model.pane(pane_id) else {
            continue;
        };
        if pane.page() != PanePageKind::Terminal {
            continue;
        }
        let Some(session) = pane.session() else {
            continue;
        };
        let Some(profile) = pane.resolved_profile(model.view_model()) else {
            continue;
        };
        if let Some(terminal) = terminals.get(&session) {
            let profile_delivered =
                send_terminal_message(terminal, TerminalViewMsg::UpdateProfile(profile.clone()));
            let delivered = profile_delivered
                && pane.frame().is_none_or(|frame| {
                    let delivered =
                        send_terminal_message(terminal, TerminalViewMsg::ApplyFrame(frame.clone()));
                    if delivered {
                        model.observe_frame(frame);
                    }
                    delivered
                });
            if delivered {
                continue;
            }
            terminals.remove(&session);
        }
        let context = metric_widget.pango_context();
        let environment =
            FontMetricEnvironment::from_context(&context, f64::from(metric_widget.scale_factor()));
        let measured = environment.and_then(|environment| {
            FontMetricsService::default()
                .measure(&context, &profile, environment)
                .map(|change| match change {
                    MetricsChange::Unchanged(measured) | MetricsChange::Changed(measured) => {
                        measured
                    }
                })
        });
        let Ok(metrics) = measured else {
            let _ = sender.output(PaneHostOutput::Error("terminal metrics unavailable"));
            continue;
        };
        let controller = TerminalView::builder()
            .launch(TerminalViewInit {
                pane: pane_id,
                session,
                profile,
                metrics,
                startup_probe: model.startup_probe(),
            })
            .forward(sender.input_sender(), move |output| {
                crate::PaneHostMsg::Terminal(session, output)
            });
        if let Some(frame) = pane.frame()
            && send_terminal_message(&controller, TerminalViewMsg::ApplyFrame(frame.clone()))
        {
            model.observe_frame(frame);
        }
        terminals.insert(session, controller);
        replaced.insert(session);
    }
    replaced
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
