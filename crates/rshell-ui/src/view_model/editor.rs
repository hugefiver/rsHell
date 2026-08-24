use std::{collections::BTreeSet, fmt};

use rshell_core::{
    AuthenticationKind, CatalogMutation, ConnectionId, ConnectionProfile, GroupId,
    TerminalOverrides, TerminalProfileId, TerminalSettingsV1, TransportKind, UiCommand,
};

use super::{
    editor_overrides::TerminalOverrideKey,
    editor_validation::{EditorValidationError, validate_profile},
    secret::{OriginalSecretMetadata, SecretEditKind, SecretEditState, uses_managed_secret},
};

const SYSTEM_AUTH: &[AuthenticationKind] =
    &[AuthenticationKind::Agent, AuthenticationKind::PublicKey];
const NATIVE_AUTH: &[AuthenticationKind] = &[
    AuthenticationKind::Password,
    AuthenticationKind::PublicKey,
    AuthenticationKind::KeyboardInteractive,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticationCapabilities {
    supported: &'static [AuthenticationKind],
}

impl AuthenticationCapabilities {
    pub fn for_transport(transport: TransportKind) -> Self {
        let supported = match transport {
            TransportKind::SystemOpenSsh => SYSTEM_AUTH,
            TransportKind::NativeSsh => NATIVE_AUTH,
        };
        Self { supported }
    }

    pub fn supported(self) -> &'static [AuthenticationKind] {
        self.supported
    }

    pub fn allows(self, authentication: AuthenticationKind) -> bool {
        self.supported.contains(&authentication)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionEditorViewModel {
    pub id: ConnectionId,
    pub is_new: bool,
    pub group_id: Option<GroupId>,
    pub position: i64,
    pub name: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub transport: TransportKind,
    pub authentication: AuthenticationKind,
    pub identity_file: String,
    pub remote_command: String,
    pub note: String,
    pub tags: BTreeSet<String>,
    pub terminal_profile_id: Option<TerminalProfileId>,
    pub terminal_overrides: TerminalOverrides,
}

pub struct ConnectionEditorDraft {
    view: ConnectionEditorViewModel,
    secret: SecretEditState,
    original_secret: OriginalSecretMetadata,
}

impl ConnectionEditorDraft {
    pub fn create(group_id: Option<GroupId>) -> Self {
        let profile = ConnectionProfile {
            group_id,
            ..ConnectionProfile::default()
        };
        Self::from_profile(profile, true)
    }

    pub fn edit(profile: &ConnectionProfile) -> Self {
        Self::from_profile(profile.clone(), false)
    }

    fn from_profile(profile: ConnectionProfile, is_new: bool) -> Self {
        let original_secret = OriginalSecretMetadata::from_profile(&profile);
        let view = ConnectionEditorViewModel {
            id: profile.id,
            is_new,
            group_id: profile.group_id,
            position: profile.position,
            name: profile.name,
            host: profile.host,
            port: profile.port.to_string(),
            username: profile.username,
            transport: profile.transport,
            authentication: profile.authentication,
            identity_file: profile
                .identity_file
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            remote_command: profile.remote_command.unwrap_or_default(),
            note: profile.note,
            tags: profile.tags,
            terminal_profile_id: profile.terminal_profile_id,
            terminal_overrides: profile.terminal_overrides,
        };
        Self {
            view,
            secret: SecretEditState::new(),
            original_secret,
        }
    }

    pub fn view(&self) -> &ConnectionEditorViewModel {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut ConnectionEditorViewModel {
        &mut self.view
    }

    pub fn is_inherited(&self, key: TerminalOverrideKey) -> bool {
        key.is_inherited(&self.view.terminal_overrides)
    }

    pub fn set_inherited(&mut self, key: TerminalOverrideKey) {
        key.inherit(&mut self.view.terminal_overrides);
    }

    pub fn set_explicit_from_base(&mut self, key: TerminalOverrideKey, base: &TerminalSettingsV1) {
        key.explicit_from(&mut self.view.terminal_overrides, base);
    }

    pub fn clear_all_overrides(&mut self) {
        self.view.terminal_overrides = self.view.terminal_overrides.clear_all();
    }

    pub fn set_secret(&mut self, value: impl Into<String>) {
        self.secret.set(value.into());
    }

    pub fn clear_secret(&mut self) {
        self.secret.clear();
    }

    pub fn uses_managed_secret(&self) -> bool {
        uses_managed_secret(self.view.transport, self.view.authentication)
    }

    pub fn set_transport(&mut self, transport: TransportKind) {
        self.view.transport = transport;
        let capabilities = AuthenticationCapabilities::for_transport(transport);
        if !capabilities.allows(self.view.authentication) {
            self.view.authentication = capabilities.supported()[0];
        }
        self.clear_if_unmanaged();
    }

    pub fn set_authentication(&mut self, authentication: AuthenticationKind) {
        if AuthenticationCapabilities::for_transport(self.view.transport).allows(authentication) {
            self.view.authentication = authentication;
            self.clear_if_unmanaged();
        }
    }

    pub fn mark_secret_edited(&mut self) {
        self.secret.mark_edited();
    }

    pub fn secret_kind(&self) -> SecretEditKind {
        self.secret.kind()
    }

    pub fn secret_is_empty(&self) -> bool {
        self.secret.is_empty()
    }

    pub fn close(&mut self) {
        self.secret.clear();
    }

    pub fn save_command(&mut self) -> Result<UiCommand, EditorValidationError> {
        let profile = validate_profile(&self.view)?;
        let secret = self
            .secret
            .take_update(
                self.original_secret,
                self.view.transport,
                self.view.authentication,
            )
            .map_err(|()| EditorValidationError::SecretRequired)?;
        let mutation = if self.view.is_new {
            CatalogMutation::Create(profile)
        } else {
            CatalogMutation::Update(profile)
        };
        Ok(UiCommand::ApplyCatalog { mutation, secret })
    }

    fn clear_if_unmanaged(&mut self) {
        if !self.uses_managed_secret() {
            self.clear_secret();
        }
    }
}

impl fmt::Debug for ConnectionEditorDraft {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionEditorDraft")
            .field("view", &self.view)
            .field("secret", &self.secret)
            .finish()
    }
}

impl Drop for ConnectionEditorDraft {
    fn drop(&mut self) {
        self.close();
    }
}
