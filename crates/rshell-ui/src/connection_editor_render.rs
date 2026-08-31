use gtk::prelude::*;
use relm4::gtk;
use rshell_core::{AuthenticationKind, TransportKind};

use crate::{
    AuthenticationCapabilities, ConnectionEditor, ConnectionEditorDraft, ConnectionEditorViewModel,
    connection_editor_override_render::render_terminal_overrides,
    connection_editor_widgets::ConnectionEditorWidgets,
};

impl ConnectionEditor {
    pub(crate) fn render(&self, root: &gtk::Box, widgets: &mut ConnectionEditorWidgets) {
        widgets.syncing.set(true);
        let Some(view) = self.draft.as_ref().map(ConnectionEditorDraft::view) else {
            root.set_visible(false);
            widgets.secret.set_text("");
            widgets.syncing.set(false);
            return;
        };
        widgets.title.set_focusable(false);
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.secret_is_empty())
            && !widgets.secret.text().is_empty()
        {
            widgets.secret.set_text("");
        }
        root.set_visible(true);
        widgets.title.set_label(if view.is_new {
            "New connection"
        } else {
            "Edit connection"
        });
        set_entry(&widgets.name, &view.name);
        set_entry(&widgets.host, &view.host);
        set_spin(&widgets.port, view.port.parse::<f64>().unwrap_or(22.0));
        set_entry(&widgets.username, &view.username);
        set_selected(
            &widgets.transport,
            match view.transport {
                TransportKind::SystemOpenSsh => 0,
                TransportKind::NativeSsh => 1,
            },
        );
        render_authentication(view, widgets);
        set_entry(&widgets.identity, &view.identity_file);
        set_entry(&widgets.remote_command, &view.remote_command);
        set_buffer(&widgets.note.buffer(), &view.note);
        set_entry(
            &widgets.tags,
            &view.tags.iter().cloned().collect::<Vec<_>>().join(", "),
        );
        self.render_terminal(view, widgets);
        widgets.error.set_label(self.error.as_deref().unwrap_or(""));
        widgets.error.set_visible(self.error.is_some());
        widgets.save.set_sensitive(!self.pending);
        widgets.syncing.set(false);
    }

    fn render_terminal(
        &self,
        view: &ConnectionEditorViewModel,
        widgets: &mut ConnectionEditorWidgets,
    ) {
        let mut labels = vec!["Inherit default".to_string()];
        labels.extend(
            self.terminal_profiles
                .iter()
                .map(|profile| profile.name.clone()),
        );
        if widgets.profile_labels != labels {
            let references = labels.iter().map(String::as_str).collect::<Vec<_>>();
            widgets
                .terminal_profile
                .set_model(Some(&gtk::StringList::new(&references)));
            widgets.profile_labels = labels;
        }
        let selected = view
            .terminal_profile_id
            .and_then(|id| {
                self.terminal_profiles
                    .iter()
                    .position(|profile| profile.id == id)
            })
            .map_or(0, |index| index as u32 + 1);
        set_selected(&widgets.terminal_profile, selected);
        let base = self.selected_base().cloned().unwrap_or_default();
        render_terminal_overrides(
            &widgets.overrides,
            &view.terminal_overrides,
            &base,
            self.pending,
        );
    }
}

fn render_authentication(view: &ConnectionEditorViewModel, widgets: &ConnectionEditorWidgets) {
    for (button, authentication) in [
        (&widgets.password_auth, AuthenticationKind::Password),
        (&widgets.public_key_auth, AuthenticationKind::PublicKey),
        (&widgets.agent_auth, AuthenticationKind::Agent),
        (
            &widgets.keyboard_auth,
            AuthenticationKind::KeyboardInteractive,
        ),
    ] {
        button.set_sensitive(
            AuthenticationCapabilities::for_transport(view.transport).allows(authentication),
        );
        let active = view.authentication == authentication;
        if button.is_active() != active {
            button.set_active(active);
        }
    }
    widgets
        .identity
        .set_sensitive(view.authentication == AuthenticationKind::PublicKey);
    let managed_secret = view.transport == TransportKind::NativeSsh
        && matches!(
            view.authentication,
            AuthenticationKind::Password | AuthenticationKind::PublicKey
        );
    widgets.secret.set_sensitive(managed_secret);
}

fn set_entry(entry: &gtk::Entry, value: &str) {
    if entry.text().as_str() != value {
        entry.set_text(value);
    }
}

fn set_buffer(buffer: &gtk::TextBuffer, value: &str) {
    if buffer.text(&buffer.start_iter(), &buffer.end_iter(), false) != value {
        buffer.set_text(value);
    }
}

fn set_spin(spin: &gtk::SpinButton, value: f64) {
    if (spin.value() - value).abs() > f64::EPSILON {
        spin.set_value(value);
    }
}

fn set_selected(dropdown: &gtk::DropDown, selected: u32) {
    if dropdown.selected() != selected {
        dropdown.set_selected(selected);
    }
}
