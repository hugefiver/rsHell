use relm4::ComponentSender;
use rshell_core::{InteractionRequest, SessionId};

use crate::{InteractionDialog, InteractionDialogOutput, InteractionViewModel};

const REPLACEMENT_CAPACITY: usize = 32;

impl InteractionDialog {
    pub(crate) fn open_or_queue(
        &mut self,
        session: SessionId,
        request: InteractionRequest,
        sender: &ComponentSender<Self>,
    ) {
        let mut incoming = InteractionViewModel::new(session, request);
        let interaction = incoming.interaction_id();
        if self
            .view
            .as_ref()
            .is_some_and(|view| view.interaction_id() == interaction)
            || self
                .queued
                .iter()
                .any(|view| view.interaction_id() == interaction)
        {
            return;
        }
        if self.view.is_none() {
            self.view = Some(incoming);
            self.visible = true;
            self.pending = false;
            self.error = None;
            return;
        }
        if self.queued.len() == REPLACEMENT_CAPACITY {
            self.output(incoming.cancel_command(), sender);
            return;
        }
        self.queued.push_back(incoming);
        if !self.pending
            && let Some(current) = &mut self.view
            && !current.is_handed_off()
        {
            let command = current.cancel_command();
            self.pending = command.is_some();
            self.output(command, sender);
        }
    }

    pub(crate) fn response_accepted(&mut self, sender: &ComponentSender<Self>) {
        self.view = self.queued.pop_front();
        self.pending = false;
        self.error = None;
        if self.view.is_none() {
            self.visible = false;
            let _ = sender.output(InteractionDialogOutput::Closed);
            return;
        }
        self.visible = true;
        if !self.queued.is_empty()
            && let Some(current) = &mut self.view
        {
            let command = current.cancel_command();
            self.pending = command.is_some();
            self.output(command, sender);
        }
    }

    pub(crate) fn dismiss_session(&mut self, session: SessionId, sender: &ComponentSender<Self>) {
        self.queued.retain(|view| view.session_id() != session);
        if self
            .view
            .as_ref()
            .is_some_and(|view| view.session_id() == session)
        {
            self.response_accepted(sender);
        }
    }
}
