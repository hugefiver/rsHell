use rshell_core::{AuthenticationKind, ConnectionProfile, SessionId, TransportKind};

use crate::MainWindow;

#[derive(Clone, Copy)]
pub(crate) struct ComponentSessionState {
    pub exists: bool,
    pub local: bool,
    pub launch_matches: bool,
    pub rendered: Option<SessionId>,
    pub geometry: Option<SessionId>,
    pub expected: Option<SessionId>,
}

pub(crate) fn session_component_is_ready(
    rendered: Option<SessionId>,
    geometry: Option<SessionId>,
    expected: Option<SessionId>,
) -> bool {
    expected.is_some() && rendered == expected && geometry == expected
}

pub(crate) fn profile_matches_surface(profile: &ConnectionProfile, surface: &str) -> bool {
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

pub(crate) fn actual_label(
    window: &MainWindow,
    surface: &str,
    profile_name: Option<&str>,
) -> Option<String> {
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
