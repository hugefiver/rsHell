use crate::{ConnectionEditor, ConnectionEditorDraftState, ConnectionEditorState, SecretEditKind};

impl ConnectionEditor {
    pub(crate) fn state_snapshot(&self) -> ConnectionEditorState {
        ConnectionEditorState {
            open: self.draft.is_some(),
            pending: self.pending,
            has_error: self.error.is_some(),
            revision: self.revision,
            draft: self.draft.as_ref().map(|draft| {
                let view = draft.view();
                ConnectionEditorDraftState {
                    id: view.id,
                    is_new: view.is_new,
                    name: view.name.clone(),
                    host: view.host.clone(),
                    port: view.port.clone(),
                    username: view.username.clone(),
                    transport: view.transport,
                    authentication: view.authentication,
                    identity_file: view.identity_file.clone(),
                    remote_command: view.remote_command.clone(),
                    note: view.note.clone(),
                    tags: view.tags.clone(),
                    secret_changed: draft.secret_kind() != SecretEditKind::Untouched,
                    secret_present: !draft.secret_is_empty(),
                }
            }),
        }
    }
}
