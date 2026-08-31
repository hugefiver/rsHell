use std::{fmt, sync::Arc};

use rshell_core::{
    AppViewModel, ConnectionId, DisplayRecoveryNotice, ErrorPaneView, PaneId, PaneLaunchTarget,
    RenderFrame, ResolvedTerminalProfile, SessionFailure, SessionId, SessionState,
};

use crate::pane_status::{self, PaneAction, PanePageKind};

#[derive(Clone)]
pub struct SessionPaneViewModel {
    pane: PaneId,
    session: Option<SessionId>,
    state: SessionState,
    target: Option<PaneLaunchTarget>,
    frame: Option<Arc<RenderFrame>>,
    error: Option<ErrorPaneView>,
    recovery_notice: Option<DisplayRecoveryNotice>,
}

impl fmt::Debug for SessionPaneViewModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionPaneViewModel")
            .field("pane", &self.pane)
            .field("session", &self.session)
            .field("state", &self.state)
            .field(
                "target",
                &self
                    .target
                    .as_ref()
                    .and_then(PaneLaunchTarget::connection_id),
            )
            .field(
                "frame_generation",
                &self.frame.as_ref().map(|frame| frame.generation),
            )
            .field("failure", &self.error.as_ref().map(|error| error.failure))
            .field("recovery_notice", &self.recovery_notice)
            .finish()
    }
}

impl SessionPaneViewModel {
    pub fn from_app(app: &AppViewModel, pane: PaneId) -> Option<Self> {
        let session = app
            .workspace
            .tabs
            .iter()
            .find_map(|tab| {
                tab.pane_tree
                    .session_id(pane)
                    .ok()
                    .map(|session| (tab.id, session))
            })?
            .1;
        Some(Self::from_leaf(app, pane, session))
    }

    pub(crate) fn from_leaf(app: &AppViewModel, pane: PaneId, session: Option<SessionId>) -> Self {
        let target = app.pane_launches.get(&pane).cloned();
        let state = session
            .and_then(|session| app.session_states.get(&session).copied())
            .unwrap_or(SessionState::Created);
        Self {
            pane,
            session,
            state,
            target,
            frame: session.and_then(|session| app.latest_frames.get(&session).cloned()),
            error: session.and_then(|session| app.error_panes.get(&session).cloned()),
            recovery_notice: session
                .and_then(|session| app.display_recovery.get(&session).copied()),
        }
    }

    pub fn pane(&self) -> PaneId {
        self.pane
    }

    pub fn session(&self) -> Option<SessionId> {
        self.session
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn frame(&self) -> Option<&Arc<RenderFrame>> {
        self.frame.as_ref()
    }

    pub fn recovery_notice(&self) -> Option<DisplayRecoveryNotice> {
        self.recovery_notice
    }

    pub fn page(&self) -> PanePageKind {
        pane_status::page(self.target.is_some(), self.session.is_some(), self.state)
    }

    pub fn status_label(&self) -> &'static str {
        pane_status::status_label(self.target.is_some(), self.session.is_some(), self.state)
    }

    pub fn actions(&self) -> Vec<PaneAction> {
        pane_status::actions(
            self.page(),
            self.connection_id().is_some(),
            self.recovery_notice.is_some(),
        )
    }

    pub fn connection_id(&self) -> Option<ConnectionId> {
        self.target
            .as_ref()
            .and_then(PaneLaunchTarget::connection_id)
    }

    pub fn diagnostics(&self) -> Option<String> {
        let target = self.target.as_ref()?;
        let (category, timestamp, diagnostic, host) = match (&self.error, self.state) {
            (Some(error), _) => (
                failure_label(error.failure),
                error.timestamp_unix_seconds,
                error.diagnostic,
                error.host.as_deref(),
            ),
            (None, SessionState::Exited) => {
                ("exited", unix_timestamp(), "session exited", target.host())
            }
            (None, _) if self.session.is_none() => (
                "disconnected",
                unix_timestamp(),
                "session disconnected",
                target.host(),
            ),
            _ => return None,
        };
        let mut lines = vec![format!("category: {category}")];
        if let Some(host) = host {
            lines.push(format!("host: {}", sanitize_host(host)));
        }
        lines.push(format!("timestamp: {timestamp}"));
        lines.push(format!("error: {diagnostic}"));
        Some(lines.join("\n"))
    }

    pub fn resolved_profile(&self, app: &AppViewModel) -> Option<ResolvedTerminalProfile> {
        self.target.as_ref()?;
        let connection = self
            .connection_id()
            .and_then(|id| app.catalog.connections.get(&id));
        let requested = connection
            .and_then(|profile| profile.terminal_profile_id)
            .unwrap_or(app.settings.default_terminal_profile);
        let profile = app
            .terminal_profiles
            .iter()
            .find(|profile| profile.id == requested)?;
        let resolved = match connection {
            Some(connection) => profile.settings.resolve(&connection.terminal_overrides),
            None => profile.settings.resolve(&Default::default()),
        };
        Some(resolved.with_app_key_bindings(&app.settings.key_bindings))
    }
}

fn sanitize_host(host: &str) -> String {
    host.chars()
        .take(255)
        .map(|character| {
            if character.is_ascii_alphanumeric() || ".:-_[]".contains(character) {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(crate) fn failure_label(failure: SessionFailure) -> &'static str {
    match failure {
        SessionFailure::Validation => "validation",
        SessionFailure::Storage => "storage",
        SessionFailure::Vault => "vault",
        SessionFailure::HostKeyRejected => "host_key_rejected",
        SessionFailure::HostKeyChanged => "host_key_changed",
        SessionFailure::Authentication => "authentication",
        SessionFailure::Network => "network",
        SessionFailure::Pty => "pty",
        SessionFailure::SshChannel => "ssh_channel",
        SessionFailure::Subprocess => "subprocess",
        SessionFailure::Platform => "platform",
        SessionFailure::Backpressure => "backpressure",
        SessionFailure::Timeout => "timeout",
        SessionFailure::Crashed => "crashed",
    }
}
