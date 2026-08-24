use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::{TerminalProfile, TransportKind};

pub use crate::connection_editor_message::{
    ConnectionEditorDraftState, ConnectionEditorInit, ConnectionEditorMsg, ConnectionEditorOutput,
    ConnectionEditorState, EditorTextField,
};
use crate::{
    ConnectionEditorDraft, ConnectionEditorViewModel, connection_editor_state::set_text,
    connection_editor_widgets::ConnectionEditorWidgets,
};

pub struct ConnectionEditor {
    pub(crate) draft: Option<ConnectionEditorDraft>,
    pub(crate) terminal_profiles: Vec<TerminalProfile>,
    pub(crate) error: Option<String>,
    pub(crate) override_input_error: Option<&'static str>,
    pub(crate) pending: bool,
    pub(crate) revision: u64,
}

impl SimpleComponent for ConnectionEditor {
    type Init = ConnectionEditorInit;
    type Input = ConnectionEditorMsg;
    type Output = ConnectionEditorOutput;
    type Root = gtk::Box;
    type Widgets = ConnectionEditorWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("editor-dialog");
        root.add_css_class("content-dialog");
        root.set_width_request(560);
        root.set_visible(false);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            draft: None,
            terminal_profiles: init.terminal_profiles,
            error: None,
            override_input_error: None,
            pending: false,
            revision: 0,
        };
        let mut widgets = ConnectionEditorWidgets::build(&root, &sender);
        model.render(&root, &mut widgets);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ConnectionEditorMsg::OpenCreate(group) => {
                self.close_draft();
                self.draft = Some(ConnectionEditorDraft::create(group));
                self.error = None;
                self.override_input_error = None;
            }
            ConnectionEditorMsg::OpenEdit(profile) => {
                self.close_draft();
                self.draft = Some(ConnectionEditorDraft::edit(profile.as_ref()));
                self.error = None;
                self.override_input_error = None;
            }
            ConnectionEditorMsg::SetTerminalProfiles(profiles) => self.terminal_profiles = profiles,
            ConnectionEditorMsg::TextChanged(field, value) => {
                if let Some(view) = self.view_mut() {
                    set_text(view, field, value);
                }
            }
            ConnectionEditorMsg::PortChanged(port) => {
                if let Some(view) = self.view_mut() {
                    view.port = port.to_string();
                }
            }
            ConnectionEditorMsg::TransportChanged(selected) => {
                if let Some(draft) = &mut self.draft {
                    let transport = if selected == 0 {
                        TransportKind::SystemOpenSsh
                    } else {
                        TransportKind::NativeSsh
                    };
                    draft.set_transport(transport);
                }
            }
            ConnectionEditorMsg::AuthenticationChanged(authentication) => {
                if let Some(draft) = &mut self.draft {
                    draft.set_authentication(authentication);
                }
            }
            ConnectionEditorMsg::SecretChanged(value) => {
                if let Some(draft) = &mut self.draft {
                    if draft.uses_managed_secret() {
                        draft.set_secret(value);
                    } else {
                        draft.clear_secret();
                    }
                }
            }
            ConnectionEditorMsg::ProfileChanged(index) => {
                let id = index
                    .checked_sub(1)
                    .and_then(|index| self.terminal_profiles.get(index as usize))
                    .map(|profile| profile.id);
                if let Some(view) = self.view_mut() {
                    view.terminal_profile_id = id;
                }
            }
            ConnectionEditorMsg::OverrideInheritance(key, inherited) => {
                self.set_override_inheritance(key, inherited);
            }
            ConnectionEditorMsg::OverrideText(key, value) => self.set_override_text(key, value),
            ConnectionEditorMsg::OverrideNumber(key, value) => self.set_override_number(key, value),
            ConnectionEditorMsg::OverrideScheme(index) => self.set_override_scheme(index),
            ConnectionEditorMsg::OverrideBool(key, value) => self.set_override_bool(key, value),
            ConnectionEditorMsg::OverrideBindings(text) => self.set_override_bindings(&text),
            ConnectionEditorMsg::ClearOverrides => {
                if let Some(draft) = &mut self.draft {
                    draft.clear_all_overrides();
                }
            }
            ConnectionEditorMsg::Save => {
                if let Some(error) = self.override_input_error {
                    self.error = Some(error.into());
                } else if let Some(result) = self.prepare_save() {
                    match result {
                        Ok(command) => {
                            self.error = None;
                            self.pending = true;
                            let _ =
                                sender.output(ConnectionEditorOutput::Command(Box::new(command)));
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            }
            ConnectionEditorMsg::Cancel => {
                self.close_draft();
                let _ = sender.output(ConnectionEditorOutput::Closed);
            }
            ConnectionEditorMsg::CommandAccepted => {
                self.close_draft();
                let _ = sender.output(ConnectionEditorOutput::Closed);
            }
            ConnectionEditorMsg::CommandRejected(error) => {
                self.pending = false;
                self.error = Some(error.to_string());
            }
            ConnectionEditorMsg::OperationFailed(context) => {
                self.pending = false;
                self.error = Some(context.into());
            }
        }
        self.revision = self.revision.saturating_add(1);
        let _ = sender.output(ConnectionEditorOutput::StateChanged(Box::new(
            self.state_snapshot(),
        )));
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        let root = widgets.root.clone();
        self.render(&root, widgets);
    }
}

impl ConnectionEditor {
    fn prepare_save(
        &mut self,
    ) -> Option<Result<rshell_core::UiCommand, crate::EditorValidationError>> {
        if self.pending {
            return None;
        }
        self.draft.as_mut().map(ConnectionEditorDraft::save_command)
    }

    fn view_mut(&mut self) -> Option<&mut ConnectionEditorViewModel> {
        self.draft.as_mut().map(ConnectionEditorDraft::view_mut)
    }

    fn close_draft(&mut self) {
        if let Some(draft) = &mut self.draft {
            draft.close();
        }
        self.draft = None;
        self.error = None;
        self.override_input_error = None;
        self.pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SecretEditKind;

    fn valid_password_draft() -> ConnectionEditorDraft {
        let mut draft = ConnectionEditorDraft::create(None);
        draft.view_mut().name = "Pending".into();
        draft.view_mut().host = "pending.example.test".into();
        draft.view_mut().transport = TransportKind::NativeSsh;
        draft.view_mut().authentication = rshell_core::AuthenticationKind::Password;
        draft.set_secret("pending-value");
        draft
    }

    #[test]
    fn save_gate_is_total_when_closed_and_idempotent_while_pending() {
        let mut editor = ConnectionEditor {
            draft: None,
            terminal_profiles: Vec::new(),
            error: None,
            override_input_error: None,
            pending: false,
            revision: 0,
        };
        assert!(
            editor.prepare_save().is_none(),
            "closed Save must be a no-op"
        );

        editor.draft = Some(valid_password_draft());
        editor.pending = true;
        assert_eq!(
            editor
                .draft
                .as_ref()
                .map(ConnectionEditorDraft::secret_kind),
            Some(SecretEditKind::EditedValue)
        );
        assert!(
            editor.prepare_save().is_none(),
            "pending Save must not emit another command"
        );
        assert_eq!(
            editor
                .draft
                .as_ref()
                .map(ConnectionEditorDraft::secret_kind),
            Some(SecretEditKind::EditedValue),
            "pending Save must not consume or alter secret state"
        );
    }
}
