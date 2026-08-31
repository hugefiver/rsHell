use relm4::ComponentSender;
use rshell_core::{InteractionRequest, SessionId};

use crate::{InteractionDialog, InteractionViewModel};

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
            self.closing = false;
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
        if self.advance_response() {
            let input = sender.input_sender().clone();
            relm4::gtk::glib::idle_add_local_once(move || {
                let _ = input.send(crate::InteractionDialogMsg::FinalizeClose);
            });
            return;
        }
        if !self.queued.is_empty()
            && let Some(current) = &mut self.view
        {
            let command = current.cancel_command();
            self.pending = command.is_some();
            self.output(command, sender);
        }
    }

    fn advance_response(&mut self) -> bool {
        self.view = self.queued.pop_front();
        self.pending = false;
        self.error = None;
        if self.view.is_none() {
            self.closing = true;
            self.visible = true;
            return true;
        }
        self.closing = false;
        self.visible = true;
        false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_response_stays_visible_until_the_idle_finalize_message() {
        let mut dialog = InteractionDialog {
            view: None,
            visible: true,
            pending: true,
            closing: false,
            error: Some("redacted".into()),
            queued: Default::default(),
            revision: 0,
        };

        assert!(dialog.advance_response());
        assert!(dialog.visible);
        assert!(dialog.closing);
        assert!(!dialog.pending);
        assert!(dialog.error.is_none());
    }
}
