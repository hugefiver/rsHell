use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use crate::config::TerminalSettings;
use crate::credentials::{CredentialStore, SystemCredentialStore};

pub const DEFAULT_SSH_PORT: u16 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionBackend {
    #[default]
    SystemOpenSsh,
    WezTermSsh,
}

impl ConnectionBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::SystemOpenSsh => "System OpenSSH",
            Self::WezTermSsh => "WezTerm SSH",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionFolder {
    pub id: Uuid,
    pub name: String,
}

impl ConnectionFolder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default, skip_serializing)]
    pub password: String,
    #[serde(skip)]
    pub password_dirty: bool,
    #[serde(default)]
    pub identity_file: String,
    #[serde(default)]
    pub remote_command: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub backend: ConnectionBackend,
    #[serde(default = "default_accept_new_host")]
    pub accept_new_host: bool,
    #[serde(default, skip_serializing_if = "TerminalSettings::is_empty")]
    pub terminal: TerminalSettings,
}

impl ConnectionProfile {
    pub fn new(name: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            folder_id: None,
            host: host.into(),
            port: DEFAULT_SSH_PORT,
            user: String::new(),
            password: String::new(),
            password_dirty: false,
            identity_file: String::new(),
            remote_command: String::new(),
            note: String::new(),
            backend: ConnectionBackend::SystemOpenSsh,
            accept_new_host: true,
            terminal: TerminalSettings::default(),
        }
    }

    pub fn destination(&self) -> String {
        if self.user.trim().is_empty() {
            self.host.trim().to_string()
        } else {
            format!("{}@{}", self.user.trim(), self.host.trim())
        }
    }

    pub fn host_label(&self) -> String {
        format!("{}:{}", self.host.trim(), self.port)
    }

    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.host = self.host.trim().to_string();
        self.user = self.user.trim().to_string();
        self.identity_file = self.identity_file.trim().to_string();
        self.remote_command = self.remote_command.trim().to_string();
        self.note = self.note.trim().to_string();
        if self.port == 0 {
            self.port = DEFAULT_SSH_PORT;
        }
    }
}

impl Default for ConnectionProfile {
    fn default() -> Self {
        Self::new("New connection", "")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionStore {
    #[serde(default)]
    pub folders: Vec<ConnectionFolder>,
    #[serde(default)]
    pub connections: Vec<ConnectionProfile>,
}

impl ConnectionStore {
    pub fn normalize(&mut self) {
        for folder in &mut self.folders {
            folder.name = folder.name.trim().to_string();
        }
        self.folders.retain(|folder| !folder.name.is_empty());
        self.folders
            .sort_by_key(|folder| folder.name.to_lowercase());

        for connection in &mut self.connections {
            connection.normalize();
        }
        self.connections
            .retain(|connection| !connection.name.is_empty() || !connection.host.is_empty());
        let folder_names = self.folder_name_map();
        self.connections.sort_by_key(|connection| {
            (
                connection
                    .folder_id
                    .and_then(|folder_id| folder_names.get(&folder_id))
                    .cloned()
                    .unwrap_or_default()
                    .to_lowercase(),
                connection.name.to_lowercase(),
                connection.host.to_lowercase(),
            )
        });

        self.cleanup_unused_folders();
    }

    pub fn cleanup_unused_folders(&mut self) {
        self.folders.retain(|folder| {
            self.connections
                .iter()
                .any(|connection| connection.folder_id == Some(folder.id))
        });
    }

    pub fn folder_name(&self, folder_id: Option<Uuid>) -> Option<&str> {
        let folder_id = folder_id?;
        self.folders
            .iter()
            .find(|folder| folder.id == folder_id)
            .map(|folder| folder.name.as_str())
    }

    pub fn ensure_folder_named(&mut self, name: &str) -> Option<Uuid> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }

        if let Some(existing) = self
            .folders
            .iter()
            .find(|folder| folder.name.eq_ignore_ascii_case(name))
        {
            return Some(existing.id);
        }

        let folder = ConnectionFolder::new(name);
        let id = folder.id;
        self.folders.push(folder);
        self.folders
            .sort_by_key(|folder| folder.name.to_lowercase());
        Some(id)
    }

    pub fn connection(&self, id: Uuid) -> Option<&ConnectionProfile> {
        self.connections
            .iter()
            .find(|connection| connection.id == id)
    }

    pub fn upsert(&mut self, mut profile: ConnectionProfile) {
        profile.normalize();

        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|connection| connection.id == profile.id)
        {
            *existing = profile;
        } else {
            self.connections.push(profile);
        }

        self.normalize();
    }

    pub fn remove(&mut self, id: Uuid) -> Option<ConnectionProfile> {
        let index = self
            .connections
            .iter()
            .position(|connection| connection.id == id)?;
        let removed = self.connections.remove(index);
        self.cleanup_unused_folders();
        Some(removed)
    }

    pub fn sorted_connections(&self) -> Vec<&ConnectionProfile> {
        let mut items = self.connections.iter().collect::<Vec<_>>();
        let folder_names = self.folder_name_map();
        items.sort_by_key(|connection| {
            (
                connection
                    .folder_id
                    .and_then(|folder_id| folder_names.get(&folder_id))
                    .cloned()
                    .unwrap_or_default()
                    .to_lowercase(),
                connection.name.to_lowercase(),
                connection.host.to_lowercase(),
            )
        });
        items
    }

    fn folder_name_map(&self) -> HashMap<Uuid, String> {
        self.folders
            .iter()
            .map(|folder| (folder.id, folder.name.clone()))
            .collect()
    }
}

#[derive(Clone)]
pub struct ConnectionRepository {
    path: PathBuf,
    credentials: Arc<dyn CredentialStore>,
}

impl Default for ConnectionRepository {
    fn default() -> Self {
        Self::new(default_repository_path())
    }
}

impl ConnectionRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            credentials: Arc::new(SystemCredentialStore),
        }
    }

    pub fn new_with_credentials(
        path: impl Into<PathBuf>,
        credentials: Arc<dyn CredentialStore>,
    ) -> Self {
        Self {
            path: path.into(),
            credentials,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<ConnectionStore> {
        if !self.path.exists() {
            crate::storage::recover_backup_if_missing(&self.path)?;
        }

        if !self.path.exists() {
            let store = ConnectionStore::default();
            self.save(&store)?;
            return Ok(store);
        }

        let mut data = std::fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read {}", self.path.display()))?;
        let mut store: ConnectionStore = match serde_json::from_str(&data) {
            Ok(store) => store,
            Err(error) => {
                if crate::storage::recover_backup(&self.path)? {
                    data = std::fs::read_to_string(&self.path).with_context(|| {
                        format!("failed to read recovered {}", self.path.display())
                    })?;
                    serde_json::from_str(&data).with_context(|| {
                        format!("failed to parse recovered {}", self.path.display())
                    })?
                } else {
                    return Err(error)
                        .with_context(|| format!("failed to parse {}", self.path.display()));
                }
            }
        };
        self.hydrate_passwords(&mut store);
        store.normalize();
        Ok(store)
    }

    pub fn save(&self, store: &ConnectionStore) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create repository directory {}", parent.display())
            })?;
        }

        let mut store = store.clone();
        store.normalize();
        self.save_passwords(&mut store)?;
        let data = serde_json::to_string_pretty(&store).context("failed to encode JSON")?;
        crate::storage::write_file_durable(&self.path, &data)
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(())
    }

    pub fn delete_password(&self, profile_id: Uuid) -> Result<()> {
        self.credentials.delete_password(profile_id)
    }

    fn hydrate_passwords(&self, store: &mut ConnectionStore) {
        let mut should_scrub_plaintext = false;
        for profile in &mut store.connections {
            if !profile.password.is_empty() {
                if self
                    .credentials
                    .save_password(profile.id, &profile.password)
                    .is_ok()
                {
                    should_scrub_plaintext = true;
                }
                continue;
            }

            if let Ok(Some(password)) = self.credentials.load_password(profile.id) {
                profile.password = password;
            }
        }

        if should_scrub_plaintext {
            let mut scrubbed = store.clone();
            for profile in &mut scrubbed.connections {
                profile.password.clear();
            }
            if let Ok(data) = serde_json::to_string_pretty(&scrubbed) {
                let _ = crate::storage::write_file_durable_without_backup(&self.path, &data);
                let backup = backup_path(&self.path);
                if backup.exists() {
                    let _ = crate::storage::write_file_durable_without_backup(&backup, &data);
                }
            }
        }
    }

    fn save_passwords(&self, store: &mut ConnectionStore) -> Result<()> {
        for profile in &store.connections {
            if profile.password.is_empty() {
                if profile.password_dirty {
                    self.credentials
                        .delete_password(profile.id)
                        .with_context(|| {
                            format!("failed to clear password for {}", profile.name)
                        })?;
                }
            } else {
                self.credentials
                    .save_password(profile.id, &profile.password)
                    .with_context(|| format!("failed to save password for {}", profile.name))?;
            }
        }

        for profile in &mut store.connections {
            profile.password.clear();
        }
        Ok(())
    }
}

fn default_repository_path() -> PathBuf {
    let base = dirs::config_local_dir()
        .or_else(dirs::config_dir)
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    base.join("rshell").join("connections.json")
}

fn backup_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "rshell-config".into());
    parent.join(format!("{file_name}.bak"))
}

const fn default_ssh_port() -> u16 {
    DEFAULT_SSH_PORT
}

const fn default_accept_new_host() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryCredentialStore {
        passwords: Mutex<HashMap<Uuid, String>>,
        fail_save: bool,
        fail_delete: bool,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load_password(&self, profile_id: Uuid) -> Result<Option<String>> {
            Ok(self.passwords.lock().unwrap().get(&profile_id).cloned())
        }

        fn save_password(&self, profile_id: Uuid, password: &str) -> Result<()> {
            if self.fail_save {
                anyhow::bail!("credential store unavailable");
            }
            self.passwords
                .lock()
                .unwrap()
                .insert(profile_id, password.to_string());
            Ok(())
        }

        fn delete_password(&self, profile_id: Uuid) -> Result<()> {
            if self.fail_delete {
                anyhow::bail!("credential delete unavailable");
            }
            self.passwords.lock().unwrap().remove(&profile_id);
            Ok(())
        }
    }

    fn test_repository(path: &Path) -> ConnectionRepository {
        ConnectionRepository::new_with_credentials(path, Arc::new(MemoryCredentialStore::default()))
    }

    fn test_repository_with_credentials(
        path: &Path,
        credentials: Arc<MemoryCredentialStore>,
    ) -> ConnectionRepository {
        ConnectionRepository::new_with_credentials(path, credentials)
    }

    #[test]
    fn repository_roundtrip_preserves_connections() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let repository = test_repository(&path);

        let mut store = ConnectionStore::default();
        let folder_id = store.ensure_folder_named("Production");
        let mut profile = ConnectionProfile::new("Edge Node", "192.168.1.10");
        profile.folder_id = folder_id;
        profile.user = "deploy".into();
        profile.password = "  padded secret  ".into();
        profile.backend = ConnectionBackend::WezTermSsh;
        store.upsert(profile.clone());

        repository.save(&store).unwrap();
        let data = std::fs::read_to_string(&path).unwrap();
        assert!(!data.contains("padded secret"));
        assert!(!data.contains("\"password\""));

        let loaded = repository.load().unwrap();

        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].name, profile.name);
        assert_eq!(loaded.connections[0].backend, ConnectionBackend::WezTermSsh);
        assert_eq!(
            loaded.folder_name(loaded.connections[0].folder_id),
            Some("Production")
        );
        assert_eq!(loaded.connections[0].password, "  padded secret  ");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repository_save_deletes_credential_when_password_is_cleared() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let credentials = Arc::new(MemoryCredentialStore::default());
        let repository = test_repository_with_credentials(&path, Arc::clone(&credentials));

        let mut store = ConnectionStore::default();
        let mut profile = ConnectionProfile::new("Edge Node", "192.168.1.10");
        let id = profile.id;
        profile.password = "secret".into();
        store.upsert(profile.clone());
        repository.save(&store).unwrap();

        let mut loaded = repository.load().unwrap();
        loaded.connections[0].password.clear();
        loaded.connections[0].password_dirty = true;
        repository.save(&loaded).unwrap();
        let reloaded = repository.load().unwrap();

        assert_eq!(reloaded.connections[0].id, id);
        assert!(reloaded.connections[0].password.is_empty());
        assert!(credentials.passwords.lock().unwrap().get(&id).is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repository_save_preserves_credential_when_password_is_empty_but_unchanged() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let credentials = Arc::new(MemoryCredentialStore::default());
        let repository = test_repository_with_credentials(&path, Arc::clone(&credentials));

        let mut store = ConnectionStore::default();
        let mut profile = ConnectionProfile::new("Edge Node", "192.168.1.10");
        let id = profile.id;
        profile.password = "secret".into();
        store.upsert(profile);
        repository.save(&store).unwrap();

        let mut reloaded = repository.load().unwrap();
        reloaded.connections[0].password.clear();
        reloaded.connections[0].user = "deploy".into();
        repository.save(&reloaded).unwrap();

        assert_eq!(
            credentials
                .passwords
                .lock()
                .unwrap()
                .get(&id)
                .map(String::as_str),
            Some("secret")
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repository_load_migrates_legacy_plaintext_password() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let repository = test_repository(&path);
        let id = Uuid::new_v4();
        let data = format!(
            r#"{{
  "folders": [],
  "connections": [
    {{
      "id": "{id}",
      "name": "Legacy",
      "host": "10.0.0.7",
      "port": 22,
      "user": "deploy",
      "password": "legacy secret"
    }}
  ]
}}"#
        );
        std::fs::write(&path, data).unwrap();

        let loaded = repository.load().unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let backup = backup_path(&path);

        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].password, "legacy secret");
        assert!(!saved.contains("legacy secret"));
        assert!(!saved.contains("\"password\""));
        if backup.exists() {
            let backup_data = std::fs::read_to_string(&backup).unwrap();
            assert!(!backup_data.contains("legacy secret"));
            assert!(!backup_data.contains("\"password\""));
            let _ = std::fs::remove_file(backup);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repository_save_fails_without_scrubbing_when_credential_store_fails() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let credentials = Arc::new(MemoryCredentialStore {
            passwords: Mutex::new(HashMap::new()),
            fail_save: true,
            fail_delete: false,
        });
        let repository = test_repository_with_credentials(&path, credentials);

        let mut store = ConnectionStore::default();
        let mut profile = ConnectionProfile::new("Broken", "10.0.0.8");
        profile.password = "secret".into();
        store.upsert(profile);

        let error = repository.save(&store).unwrap_err().to_string();

        assert!(error.contains("failed to save password"));
        assert!(!path.exists());
    }

    #[test]
    fn repository_delete_password_reports_credential_store_failure() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let credentials = Arc::new(MemoryCredentialStore {
            passwords: Mutex::new(HashMap::new()),
            fail_save: false,
            fail_delete: true,
        });
        let repository = test_repository_with_credentials(&path, credentials);

        let error = repository
            .delete_password(Uuid::new_v4())
            .unwrap_err()
            .to_string();

        assert!(error.contains("credential delete unavailable"));
    }

    #[test]
    fn repository_load_recovers_backup_file() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let backup_path = backup_path(&path);
        let repository = test_repository(&path);

        let mut store = ConnectionStore::default();
        store.upsert(ConnectionProfile::new("Recovered", "10.0.0.5"));
        repository.save(&store).unwrap();
        std::fs::rename(&path, &backup_path).unwrap();

        let loaded = repository.load().unwrap();

        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].name, "Recovered");
        assert!(path.exists());
        assert!(!backup_path.exists());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn repository_load_recovers_backup_when_target_is_corrupt() {
        let path = std::env::temp_dir().join(format!("rshell-test-{}.json", Uuid::new_v4()));
        let backup_path = backup_path(&path);
        let repository = test_repository(&path);

        let mut previous = ConnectionStore::default();
        previous.upsert(ConnectionProfile::new("Recovered", "10.0.0.5"));
        repository.save(&previous).unwrap();

        let mut current = ConnectionStore::default();
        current.upsert(ConnectionProfile::new("Current", "10.0.0.6"));
        repository.save(&current).unwrap();
        std::fs::write(&path, "{not-json").unwrap();

        let loaded = repository.load().unwrap();

        assert_eq!(loaded.connections.len(), 1);
        assert_eq!(loaded.connections[0].name, "Recovered");
        assert!(path.exists());
        assert!(!backup_path.exists());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn destination_trims_user_and_host() {
        let mut profile = ConnectionProfile::new("Server", "  example.com  ");

        assert_eq!(profile.destination(), "example.com");

        profile.user = "  deploy  ".into();
        assert_eq!(profile.destination(), "deploy@example.com");
    }
}
