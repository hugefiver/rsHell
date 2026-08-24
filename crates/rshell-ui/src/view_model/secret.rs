use std::fmt;

use rshell_core::{AuthenticationKind, ConnectionProfile, SecretUpdate, TransportKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretEditKind {
    Untouched,
    EditedEmpty,
    EditedValue,
}

pub(super) struct SecretEditState {
    bytes: Vec<u8>,
    edited: bool,
}

#[derive(Clone, Copy)]
pub(super) struct OriginalSecretMetadata {
    transport: TransportKind,
    authentication: AuthenticationKind,
    had_credential: bool,
}

impl OriginalSecretMetadata {
    pub(super) fn from_profile(profile: &ConnectionProfile) -> Self {
        Self {
            transport: profile.transport,
            authentication: profile.authentication,
            had_credential: profile.credential_ref.is_some(),
        }
    }
}

impl SecretEditState {
    pub(super) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            edited: false,
        }
    }

    pub(super) fn set(&mut self, value: String) {
        self.wipe();
        self.bytes = value.into_bytes();
        self.edited = true;
    }

    pub(super) fn mark_edited(&mut self) {
        self.edited = true;
    }

    pub(super) fn kind(&self) -> SecretEditKind {
        match (self.edited, self.bytes.is_empty()) {
            (false, _) => SecretEditKind::Untouched,
            (true, true) => SecretEditKind::EditedEmpty,
            (true, false) => SecretEditKind::EditedValue,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(super) fn take_update(
        &mut self,
        original: OriginalSecretMetadata,
        transport: TransportKind,
        authentication: AuthenticationKind,
    ) -> Result<SecretUpdate, ()> {
        if !uses_managed_secret(transport, authentication) {
            self.clear();
            return Ok(if original.had_credential {
                SecretUpdate::Clear
            } else {
                SecretUpdate::Unchanged
            });
        }
        if authentication == AuthenticationKind::Password {
            let usable = original.had_credential
                && original.transport == TransportKind::NativeSsh
                && original.authentication == AuthenticationKind::Password;
            return match self.kind() {
                SecretEditKind::EditedValue => Ok(self.take_set()),
                SecretEditKind::Untouched if usable => Ok(SecretUpdate::Unchanged),
                SecretEditKind::EditedEmpty if usable => Ok(SecretUpdate::Clear),
                SecretEditKind::Untouched | SecretEditKind::EditedEmpty => Err(()),
            };
        }
        match self.kind() {
            SecretEditKind::EditedValue => Ok(self.take_set()),
            SecretEditKind::EditedEmpty if original.had_credential => Ok(SecretUpdate::Clear),
            SecretEditKind::EditedEmpty => Ok(SecretUpdate::Unchanged),
            SecretEditKind::Untouched
                if original.transport == TransportKind::NativeSsh
                    && original.authentication == AuthenticationKind::PublicKey =>
            {
                Ok(SecretUpdate::Unchanged)
            }
            SecretEditKind::Untouched if original.had_credential => Ok(SecretUpdate::Clear),
            SecretEditKind::Untouched => Ok(SecretUpdate::Unchanged),
        }
    }

    pub(super) fn clear(&mut self) {
        self.wipe();
        self.edited = false;
    }

    fn wipe(&mut self) {
        self.bytes.fill(0);
        self.bytes.clear();
    }

    fn take_set(&mut self) -> SecretUpdate {
        let bytes = std::mem::take(&mut self.bytes);
        self.edited = false;
        let value = String::from_utf8(bytes).expect("secret originated as UTF-8");
        SecretUpdate::Set(value.into())
    }
}

pub(super) fn uses_managed_secret(
    transport: TransportKind,
    authentication: AuthenticationKind,
) -> bool {
    transport == TransportKind::NativeSsh
        && matches!(
            authentication,
            AuthenticationKind::Password | AuthenticationKind::PublicKey
        )
}

impl fmt::Debug for SecretEditState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretEditState")
            .field("kind", &self.kind())
            .field("value", &"[REDACTED]")
            .finish()
    }
}

impl Drop for SecretEditState {
    fn drop(&mut self) {
        self.clear();
    }
}
