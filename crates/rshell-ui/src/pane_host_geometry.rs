use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use gtk::prelude::*;
use relm4::{ComponentSender, Controller, gtk};
use rshell_core::{SessionId, TerminalSize};

use crate::{PaneHost, PaneHostMsg, TerminalView, pane_host_terminals::send_terminal_message};

#[derive(Clone, Default)]
pub(crate) struct PaneHostGeometryAck {
    state: Rc<RefCell<GeometryAckState>>,
    retry_registered: Rc<Cell<bool>>,
}

#[derive(Default)]
struct GeometryAckState {
    current: BTreeSet<SessionId>,
    acknowledged: BTreeSet<SessionId>,
    probed: BTreeSet<SessionId>,
}

impl PaneHostGeometryAck {
    pub(crate) fn synchronize(
        &self,
        sessions: impl Iterator<Item = SessionId>,
        replaced: &BTreeSet<SessionId>,
    ) {
        let current = sessions.collect::<BTreeSet<_>>();
        let mut state = self.state.borrow_mut();
        state.current = current;
        let current = state.current.clone();
        state
            .acknowledged
            .retain(|session| current.contains(session) && !replaced.contains(session));
        state
            .probed
            .retain(|session| current.contains(session) && !replaced.contains(session));
    }

    pub(crate) fn acknowledge(
        &self,
        source: SessionId,
        session: SessionId,
        size: TerminalSize,
    ) -> bool {
        if source != session || !positive_terminal_geometry(size) {
            return false;
        }
        let mut state = self.state.borrow_mut();
        if !state.current.contains(&session) {
            return false;
        }
        state.probed.remove(&session);
        state.acknowledged.insert(session);
        true
    }

    pub(crate) fn refresh(&self, terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>) {
        for session in self.unacknowledged() {
            let message = if self.mark_probed(session) {
                crate::TerminalViewMsg::RefreshGeometry
            } else {
                crate::TerminalViewMsg::ReplayGeometry
            };
            let delivered = terminals
                .get(&session)
                .is_some_and(|terminal| send_terminal_message(terminal, message));
            if !delivered {
                terminals.remove(&session);
                self.forget(session);
            }
        }
    }

    pub(crate) fn schedule(&self, content: &gtk::Overlay, sender: &ComponentSender<PaneHost>) {
        if !self.has_unacknowledged() {
            content.remove_css_class("pane-geometry-pending");
            return;
        }
        content.add_css_class("pane-geometry-pending");
        if self.retry_registered.replace(true) {
            return;
        }
        let state = self.clone();
        let input = sender.input_sender().clone();
        let _ = content.add_tick_callback(move |content, _| {
            state.retry_registered.set(false);
            if !content.is_mapped() || !state.has_unacknowledged() {
                return gtk::glib::ControlFlow::Break;
            }
            let _ = input.send(PaneHostMsg::RefreshUnacknowledgedGeometry);
            gtk::glib::ControlFlow::Break
        });
    }

    fn has_unacknowledged(&self) -> bool {
        let state = self.state.borrow();
        state
            .current
            .iter()
            .any(|session| !state.acknowledged.contains(session))
    }

    fn unacknowledged(&self) -> Vec<SessionId> {
        let state = self.state.borrow();
        state
            .current
            .iter()
            .filter(|session| !state.acknowledged.contains(session))
            .copied()
            .collect()
    }

    fn forget(&self, session: SessionId) {
        let mut state = self.state.borrow_mut();
        state.current.remove(&session);
        state.acknowledged.remove(&session);
        state.probed.remove(&session);
    }

    fn mark_probed(&self, session: SessionId) -> bool {
        self.state.borrow_mut().probed.insert(session)
    }
}

pub(crate) fn positive_terminal_geometry(size: TerminalSize) -> bool {
    size.cols > 0 && size.rows > 0 && size.pixel_width > 0 && size.pixel_height > 0 && size.dpi > 0
}
