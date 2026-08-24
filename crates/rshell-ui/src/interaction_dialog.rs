use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use std::collections::VecDeque;

pub use crate::interaction_dialog_message::{
    InteractionDialogInit, InteractionDialogMsg, InteractionDialogOutput, InteractionDialogState,
};
use crate::{
    InteractionAction, InteractionViewModel, interaction_dialog_render::render_interaction,
    interaction_dialog_widgets::InteractionDialogWidgets,
};

pub struct InteractionDialog {
    pub(crate) view: Option<InteractionViewModel>,
    pub(crate) visible: bool,
    pub(crate) pending: bool,
    pub(crate) error: Option<String>,
    pub(crate) queued: VecDeque<InteractionViewModel>,
    pub(crate) revision: u64,
}

impl SimpleComponent for InteractionDialog {
    type Init = InteractionDialogInit;
    type Input = InteractionDialogMsg;
    type Output = InteractionDialogOutput;
    type Root = gtk::Box;
    type Widgets = InteractionDialogWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("interaction-dialog");
        root.add_css_class("content-dialog");
        root.set_width_request(520);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Center);
        root.set_visible(false);
        root
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            view: None,
            visible: false,
            pending: false,
            error: None,
            queued: VecDeque::new(),
            revision: 0,
        };
        let mut widgets = InteractionDialogWidgets::build(&root);
        attach_keys(&root, &sender);
        render_interaction(&model, &root, &mut widgets, &sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            InteractionDialogMsg::Open { session, request } => {
                self.open_or_queue(session, request, &sender);
            }
            InteractionDialogMsg::Answer(index, value) => {
                if !self.pending
                    && let Some(view) = &mut self.view
                    && let Err(error) = view.set_answer(index, value)
                {
                    self.error = Some(error.into());
                }
            }
            InteractionDialogMsg::Action(InteractionAction::CopyDiagnostics) => {
                if let Some(view) = &self.view {
                    let diagnostics = format!(
                        "Host key changed for {} ({})",
                        view.endpoint().unwrap_or("unknown endpoint"),
                        view.fingerprint().unwrap_or("unknown fingerprint")
                    );
                    let _ = sender.output(InteractionDialogOutput::CopyDiagnostics(diagnostics));
                }
            }
            InteractionDialogMsg::Action(action) => {
                if let Some(view) = &mut self.view {
                    let command = view.action_command(action);
                    if command.is_some() {
                        self.pending = true;
                    }
                    self.output(command, &sender);
                }
            }
            InteractionDialogMsg::ResponseAccepted(interaction)
                if self
                    .view
                    .as_ref()
                    .is_some_and(|view| view.interaction_id() == interaction) =>
            {
                self.response_accepted(&sender);
            }
            InteractionDialogMsg::DismissSession(session) => self.dismiss_session(session, &sender),
            InteractionDialogMsg::OperationFailed(interaction, context)
                if self
                    .view
                    .as_ref()
                    .is_some_and(|view| view.interaction_id() == interaction) =>
            {
                self.pending = false;
                if let Some(view) = &mut self.view {
                    view.submission_failed();
                }
                self.error = Some(context.into());
            }
            InteractionDialogMsg::CommandRejected(interaction, error)
                if self
                    .view
                    .as_ref()
                    .is_some_and(|view| view.interaction_id() == interaction) =>
            {
                self.pending = false;
                if let Some(view) = &mut self.view {
                    view.submission_failed();
                }
                self.error = Some(error.to_string());
            }
            InteractionDialogMsg::ResponseAccepted(_)
            | InteractionDialogMsg::OperationFailed(_, _)
            | InteractionDialogMsg::CommandRejected(_, _) => {}
        }
        self.revision = self.revision.saturating_add(1);
        let _ = sender.output(InteractionDialogOutput::StateChanged(
            crate::interaction_dialog_message::InteractionDialogState {
                interaction: self.view.as_ref().map(InteractionViewModel::interaction_id),
                pending: self.pending,
                has_error: self.error.is_some(),
                revision: self.revision,
                prompt_count: self
                    .view
                    .as_ref()
                    .map_or(0, InteractionViewModel::prompt_count),
                answered_prompts: self
                    .view
                    .as_ref()
                    .map_or_else(Vec::new, InteractionViewModel::answered_prompt_indices),
            },
        ));
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        if self.pending || !self.visible {
            widgets.wipe_inputs();
        }
        if let Some(root) = widgets.title.ancestor(gtk::Box::static_type())
            && let Ok(root) = root.downcast::<gtk::Box>()
        {
            render_interaction(self, &root, widgets, &sender);
        }
    }
}

impl InteractionDialog {
    pub(crate) fn output(
        &self,
        command: Option<rshell_core::UiCommand>,
        sender: &ComponentSender<Self>,
    ) {
        if let Some(command) = command {
            let _ = sender.output(InteractionDialogOutput::Command(Box::new(command)));
        }
    }
}

fn attach_keys(root: &gtk::Box, sender: &ComponentSender<InteractionDialog>) {
    let keys = gtk::EventControllerKey::new();
    let input = sender.input_sender().clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        let action = if key == gtk::gdk::Key::Escape {
            Some(InteractionAction::Cancel)
        } else if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
            Some(InteractionAction::Submit)
        } else {
            None
        };
        if let Some(action) = action {
            let _ = input.send(InteractionDialogMsg::Action(action));
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    root.add_controller(keys);
}
