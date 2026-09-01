use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use gtk::prelude::*;
use relm4::{ComponentController, ComponentSender, Controller, gtk};
use rshell_core::{SessionId, SessionUiCommand, TerminalSize, UiCommand};

use crate::{
    PaneHost, PaneHostModel, PaneHostMsg, PaneHostOutput, TerminalView, TerminalViewMsg,
    pane_host_terminals::send_terminal_message,
};

#[derive(Clone, Default)]
pub(crate) struct PaneHostGeometryAck {
    state: Rc<RefCell<GeometryAckState>>,
    retry_registered: Rc<Cell<bool>>,
}

#[derive(Default)]
struct GeometryAckState {
    current: BTreeSet<SessionId>,
    acknowledged: BTreeMap<SessionId, TerminalSize>,
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
            .retain(|session, _| current.contains(session) && !replaced.contains(session));
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
        state.acknowledged.insert(session, size);
        true
    }

    fn current_positive_resize(
        &self,
        source: SessionId,
        command: &UiCommand,
    ) -> Option<(TerminalSize, bool)> {
        let UiCommand::Session {
            session,
            command: SessionUiCommand::Resize(size),
        } = command
        else {
            return None;
        };
        let state = self.state.borrow();
        (source == *session && positive_terminal_geometry(*size) && state.current.contains(session))
            .then_some((*size, state.acknowledged.get(session) == Some(size)))
    }

    pub(crate) fn refresh(&self, terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>) {
        for session in self.unacknowledged() {
            let message = terminals
                .get(&session)
                .and_then(|terminal| allocation_probe(terminal.widget()))
                .unwrap_or_else(|| self.next_probe(session));
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
            .any(|session| !state.acknowledged.contains_key(session))
    }

    fn unacknowledged(&self) -> Vec<SessionId> {
        let state = self.state.borrow();
        state
            .current
            .iter()
            .filter(|session| !state.acknowledged.contains_key(session))
            .copied()
            .collect()
    }

    fn forget(&self, session: SessionId) {
        let mut state = self.state.borrow_mut();
        state.current.remove(&session);
        state.acknowledged.remove(&session);
        state.probed.remove(&session);
    }

    fn next_probe(&self, session: SessionId) -> crate::TerminalViewMsg {
        let mut state = self.state.borrow_mut();
        if state.probed.remove(&session) {
            crate::TerminalViewMsg::ReplayGeometry
        } else {
            state.probed.insert(session);
            crate::TerminalViewMsg::RefreshGeometry
        }
    }
}

pub(crate) fn forward_terminal_command(
    source: SessionId,
    command: Box<UiCommand>,
    geometry: &PaneHostGeometryAck,
    terminals: &mut BTreeMap<SessionId, Controller<TerminalView>>,
    content: &gtk::Overlay,
    model: &PaneHostModel,
    sender: &ComponentSender<PaneHost>,
) -> bool {
    let resize = geometry.current_positive_resize(source, command.as_ref());
    if let Some((size, true)) = resize {
        let delivered = terminals.get(&source).is_some_and(|terminal| {
            send_terminal_message(terminal, TerminalViewMsg::GeometryAcknowledged(size))
        });
        if delivered {
            return false;
        }
        terminals.remove(&source);
        geometry.forget(source);
        geometry.schedule(content, sender);
        return true;
    }
    if sender.output(PaneHostOutput::Command(command)).is_err() {
        return false;
    }
    let Some((size, false)) = resize else {
        return false;
    };
    let delivered = terminals.get(&source).is_some_and(|terminal| {
        send_terminal_message(terminal, TerminalViewMsg::GeometryAcknowledged(size))
    });
    if delivered && geometry.acknowledge(source, source, size) {
        model.observe_terminal_geometry(size);
        let _ = sender.output(PaneHostOutput::GeometryReady(source));
        geometry.schedule(content, sender);
        return false;
    }
    terminals.remove(&source);
    geometry.forget(source);
    geometry.schedule(content, sender);
    true
}

pub(crate) fn positive_terminal_geometry(size: TerminalSize) -> bool {
    size.cols > 0 && size.rows > 0 && size.pixel_width > 0 && size.pixel_height > 0 && size.dpi > 0
}

fn allocation_probe(widget: &gtk::Overlay) -> Option<TerminalViewMsg> {
    allocation_message(
        widget.is_mapped(),
        widget.width(),
        widget.height(),
        widget.scale_factor(),
    )
}

fn allocation_message(
    mapped: bool,
    width: i32,
    height: i32,
    scale: i32,
) -> Option<TerminalViewMsg> {
    (mapped && width > 0 && height > 0 && scale > 0).then_some(TerminalViewMsg::Resize {
        width,
        height,
        scale: f64::from(scale),
    })
}

#[cfg(test)]
mod tests {
    use rshell_core::SessionId;

    use super::{PaneHostGeometryAck, allocation_message};

    #[test]
    fn unacknowledged_geometry_rechecks_allocation_after_an_empty_replay() {
        let geometry = PaneHostGeometryAck::default();
        let session = SessionId::new();
        assert!(matches!(
            geometry.next_probe(session),
            crate::TerminalViewMsg::RefreshGeometry
        ));
        assert!(matches!(
            geometry.next_probe(session),
            crate::TerminalViewMsg::ReplayGeometry
        ));
        assert!(matches!(
            geometry.next_probe(session),
            crate::TerminalViewMsg::RefreshGeometry
        ));
    }

    #[test]
    fn positive_mapped_host_allocation_is_a_real_resize_probe() {
        assert!(matches!(
            allocation_message(true, 640, 360, 2),
            Some(crate::TerminalViewMsg::Resize {
                width: 640,
                height: 360,
                scale,
            }) if scale.to_bits() == 2.0_f64.to_bits()
        ));
        assert!(allocation_message(false, 640, 360, 2).is_none());
        assert!(allocation_message(true, 0, 360, 2).is_none());
    }
}
