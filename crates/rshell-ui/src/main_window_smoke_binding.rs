use rshell_core::{AuthenticationKind, PaneLaunchTarget, TransportKind};

use crate::{
    ConnectionEditorDraftState, EditorTextField, MainWindow, SmokeAction, SmokeActionKind,
    SmokeBindingEvidence, SmokeConnectionField,
    main_window_smoke_visual::{visual_checkpoint_binding, visual_checkpoint_component_verified},
    smoke_driver_observation::SmokeBindingRequest,
};

impl MainWindow {
    pub(crate) fn smoke_binding(
        &self,
        request: Option<&SmokeBindingRequest>,
    ) -> Option<SmokeBindingEvidence> {
        let request = request?;
        if matches!(request.action, SmokeAction::VisualCheckpoint) {
            let component_verified = visual_checkpoint_component_verified(
                self.smoke_state.visual_checkpoint,
                self.smoke_state.visual.as_ref(),
            );
            return Some(visual_checkpoint_binding(
                request.surface.as_deref(),
                request.connection.as_deref(),
                component_verified,
            ));
        }
        let active_tab = self
            .smoke_state
            .active_tab
            .or(self.view_model.workspace.active_tab)
            .and_then(|id| {
                self.view_model
                    .workspace
                    .tabs
                    .iter()
                    .find(|tab| tab.id == id)
            })
            .or_else(|| self.view_model.workspace.active_tab());
        let pane_id = active_tab.map(|tab| tab.active_pane);
        let session_id =
            active_tab.and_then(|tab| tab.pane_tree.session_id(tab.active_pane).ok().flatten());
        let launch = pane_id.and_then(|pane| self.view_model.pane_launches.get(&pane));
        let profile = request.connection.as_deref().and_then(|name| {
            self.view_model
                .catalog
                .connections
                .iter()
                .find(|(_, profile)| profile.name == name)
        });
        let draft = self.smoke_state.editor_draft.as_ref();
        let editor_action = matches!(
            &request.action,
            SmokeAction::OpenConnectionEditor | SmokeAction::SetConnectionField(_)
        );
        let connection_id = profile
            .map(|(id, _)| *id)
            .or_else(|| editor_action.then(|| draft.map(|value| value.id)).flatten());
        let profile = profile.map(|(_, profile)| profile);
        let local = matches!(launch, Some(PaneLaunchTarget::Local));
        let launch_matches = match (launch, connection_id) {
            (Some(PaneLaunchTarget::Connection { id, .. }), Some(expected)) => *id == expected,
            _ => false,
        };
        let component_verified = component_verified(
            self,
            request.action.kind(),
            profile.is_some(),
            connection_id,
            session_id.is_some(),
            local,
            launch_matches,
        );
        let surface = request.surface.as_deref()?;
        let verified = match surface {
            "native_password"
            | "native_key"
            | "native_keyboard_interactive"
            | "system_agent"
            | "host_key"
            | "vault" => {
                if editor_action {
                    component_verified
                        && request.connection.as_deref() == Some(surface)
                        && draft.is_some_and(|draft| editor_action_matches(&request.action, draft))
                } else {
                    component_verified
                        && profile.is_some_and(|profile| profile_matches_surface(profile, surface))
                        && request.connection.as_deref() == Some(surface)
                        && (!requires_active_launch(request.action.kind()) || launch_matches)
                }
            }
            "local_terminal" => {
                component_verified && local && request.connection.as_deref() == Some("local")
            }
            "tabs_splits" | "imports" | "gtk" | "cleanup" => component_verified,
            _ => false,
        };
        let draft_name = draft
            .filter(|draft| !draft.name.is_empty())
            .map(|draft| draft.name.as_str());
        let profile_name = profile.map(|profile| profile.name.as_str()).or(draft_name);
        let endpoint = profile
            .map(|profile| format!("{}:{}", profile.host, profile.port))
            .or_else(|| draft_endpoint(draft));
        Some(SmokeBindingEvidence {
            verified,
            component_verified,
            actual_label: actual_label(self, surface, profile_name)
                .or_else(|| editor_action.then(|| "connection_editor".to_owned())),
            connection_id,
            profile_name: profile_name.map(str::to_owned),
            endpoint,
            pane_id,
            session_id,
            local,
        })
    }
}

pub(crate) fn editor_action_matches(
    action: &SmokeAction,
    draft: &ConnectionEditorDraftState,
) -> bool {
    match action {
        SmokeAction::OpenConnectionEditor => draft.is_new,
        SmokeAction::SetConnectionField(field) => match field {
            SmokeConnectionField::Text { field, value } => match field {
                EditorTextField::Name => draft.name == *value,
                EditorTextField::Host => draft.host == *value,
                EditorTextField::Username => draft.username == *value,
                EditorTextField::IdentityFile => draft.identity_file == *value,
                EditorTextField::RemoteCommand => draft.remote_command == *value,
                EditorTextField::Note => draft.note == *value,
                EditorTextField::Tags => {
                    draft.tags
                        == value
                            .split(',')
                            .map(str::trim)
                            .filter(|tag| !tag.is_empty())
                            .map(str::to_owned)
                            .collect()
                }
            },
            SmokeConnectionField::Port(port) => draft.port.parse::<u16>() == Ok(*port),
            SmokeConnectionField::Transport(transport) => draft.transport == *transport,
            SmokeConnectionField::Authentication(authentication) => {
                draft.authentication == *authentication
            }
            SmokeConnectionField::SecretFromEnv { .. } => {
                draft.secret_changed && draft.secret_present
            }
        },
        _ => false,
    }
}

fn draft_endpoint(draft: Option<&ConnectionEditorDraftState>) -> Option<String> {
    let draft = draft?;
    let port = draft.port.parse::<u16>().ok()?;
    (!draft.host.is_empty()).then(|| format!("{}:{port}", draft.host))
}

fn component_verified(
    window: &MainWindow,
    action: SmokeActionKind,
    profile_exists: bool,
    connection: Option<rshell_core::ConnectionId>,
    session_exists: bool,
    local: bool,
    launch_matches: bool,
) -> bool {
    match action {
        SmokeActionKind::WaitWindowRealized => window.smoke_state.window_realized,
        SmokeActionKind::OpenConnectionEditor | SmokeActionKind::SetConnectionField => {
            window.smoke_state.editor_open
        }
        SmokeActionKind::SubmitConnection => profile_exists,
        SmokeActionKind::SelectConnection => window.smoke_state.sidebar_selection == connection,
        SmokeActionKind::Connect
        | SmokeActionKind::RespondHostKey
        | SmokeActionKind::RespondAuth
        | SmokeActionKind::SendTerminalText
        | SmokeActionKind::PasteTextFromEnv
        | SmokeActionKind::ResizeTerminal
        | SmokeActionKind::WaitFrameContains
        | SmokeActionKind::SearchTerminal
        | SmokeActionKind::SelectRange
        | SmokeActionKind::CopySelection
        | SmokeActionKind::Reconnect => session_exists && (local || launch_matches),
        SmokeActionKind::NewTab
        | SmokeActionKind::SplitHorizontal
        | SmokeActionKind::SplitVertical
        | SmokeActionKind::SwitchTab => !window.view_model.workspace.tabs.is_empty(),
        SmokeActionKind::VisualCheckpoint => visual_checkpoint_component_verified(
            window.smoke_state.visual_checkpoint,
            window.smoke_state.visual.as_ref(),
        ),
        SmokeActionKind::PreviewImport => window.smoke_state.import_preview_ready,
        SmokeActionKind::CommitImport => window.smoke_state.imports.completed,
        SmokeActionKind::CancelImport => window.smoke_state.imports.cancel_pending_zero,
        SmokeActionKind::CloseAll => {
            window.smoke_state.shutdown_complete && window.view_model.workspace.tabs.is_empty()
        }
    }
}

fn requires_active_launch(action: SmokeActionKind) -> bool {
    matches!(
        action,
        SmokeActionKind::Connect
            | SmokeActionKind::RespondHostKey
            | SmokeActionKind::RespondAuth
            | SmokeActionKind::SendTerminalText
            | SmokeActionKind::WaitFrameContains
            | SmokeActionKind::Reconnect
    )
}

fn profile_matches_surface(profile: &rshell_core::ConnectionProfile, surface: &str) -> bool {
    if profile.name != surface {
        return false;
    }
    matches!(
        (surface, profile.transport, profile.authentication),
        (
            "native_password" | "host_key" | "vault",
            TransportKind::NativeSsh,
            AuthenticationKind::Password
        ) | (
            "native_key",
            TransportKind::NativeSsh,
            AuthenticationKind::PublicKey
        ) | (
            "native_keyboard_interactive",
            TransportKind::NativeSsh,
            AuthenticationKind::KeyboardInteractive
        ) | (
            "system_agent",
            TransportKind::SystemOpenSsh,
            AuthenticationKind::Agent
        )
    )
}

fn actual_label(window: &MainWindow, surface: &str, profile_name: Option<&str>) -> Option<String> {
    let component_label = match surface {
        "local_terminal" => Some("local"),
        "tabs_splits" => Some("workspace"),
        "imports" if window.smoke_state.imports.completed => Some("import_catalog"),
        "imports" => Some("import_preview"),
        "gtk" => Some("main_window"),
        "cleanup" => Some("shutdown"),
        _ => None,
    };
    component_label
        .map(str::to_owned)
        .or_else(|| profile_name.map(str::to_owned))
}
