use anyhow::{Context, Result};
use uuid::Uuid;

const SERVICE_NAME: &str = "io.github.hugefiver.rshell";

pub trait CredentialStore: Send + Sync {
    fn load_password(&self, profile_id: Uuid) -> Result<Option<String>>;
    fn save_password(&self, profile_id: Uuid, password: &str) -> Result<()>;
    fn delete_password(&self, profile_id: Uuid) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct SystemCredentialStore;

impl SystemCredentialStore {
    fn entry(profile_id: Uuid) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE_NAME, &profile_id.to_string())
            .context("failed to create credential entry")
    }
}

impl CredentialStore for SystemCredentialStore {
    fn load_password(&self, profile_id: Uuid) -> Result<Option<String>> {
        let entry = Self::entry(profile_id)?;
        match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => {
                Err(error).context("failed to load password from system credential store")
            }
        }
    }

    fn save_password(&self, profile_id: Uuid, password: &str) -> Result<()> {
        Self::entry(profile_id)?
            .set_password(password)
            .context("failed to save password to system credential store")
    }

    fn delete_password(&self, profile_id: Uuid) -> Result<()> {
        let entry = Self::entry(profile_id)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => {
                Err(error).context("failed to delete password from system credential store")
            }
        }
    }
}
