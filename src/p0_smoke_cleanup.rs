use std::{collections::BTreeMap, fs, path::Path};

use rshell_core::{CatalogMutation, CredentialRef, SecretUpdate};
use rshell_storage::{CredentialCoordinator, SqliteRepository};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct P0CleanupEvidence {
    pub(crate) application_shutdown_clean: Option<bool>,
    pub(crate) repository_shutdown_clean: Option<bool>,
    pub(crate) actor_count: Option<usize>,
    pub(crate) direct_session_child_count: Option<usize>,
    pub(crate) credential_profiles_deleted: Option<usize>,
    pub(crate) credential_profiles_remaining: Option<usize>,
    pub(crate) temporary_vault_references_checked: Option<usize>,
    pub(crate) temporary_vault_references_remaining: Option<usize>,
    pub(crate) journal_failed: Option<usize>,
    pub(crate) journal_remaining: Option<usize>,
    pub(crate) platform_files_scanned: Option<usize>,
    pub(crate) scenario_secret_values_scanned: Option<usize>,
    pub(crate) scenario_secret_values_found: Option<usize>,
    pub(crate) state_scan_complete: Option<bool>,
}

impl P0CleanupEvidence {
    pub(crate) const fn new() -> Self {
        Self {
            application_shutdown_clean: None,
            repository_shutdown_clean: None,
            actor_count: None,
            direct_session_child_count: None,
            credential_profiles_deleted: None,
            credential_profiles_remaining: None,
            temporary_vault_references_checked: None,
            temporary_vault_references_remaining: None,
            journal_failed: None,
            journal_remaining: None,
            platform_files_scanned: None,
            scenario_secret_values_scanned: None,
            scenario_secret_values_found: None,
            state_scan_complete: None,
        }
    }

    pub(crate) fn actors_are_stopped(&self) -> bool {
        self.actor_count == Some(0)
    }

    pub(crate) fn application_is_stopped(&self) -> bool {
        self.application_shutdown_clean == Some(true)
    }

    pub(crate) fn repository_is_stopped(&self) -> bool {
        self.repository_shutdown_clean == Some(true)
    }

    pub(crate) fn direct_session_children_are_stopped(&self) -> bool {
        self.direct_session_child_count == Some(0)
    }

    pub(crate) fn vault_references_are_absent(&self) -> bool {
        self.temporary_vault_references_remaining == Some(0)
    }

    pub(crate) fn credential_profiles_are_deleted(&self) -> bool {
        self.credential_profiles_remaining == Some(0)
    }

    pub(crate) fn journal_is_empty(&self) -> bool {
        self.journal_failed == Some(0) && self.journal_remaining == Some(0)
    }

    pub(crate) fn state_files_are_secret_free(&self) -> bool {
        self.state_scan_complete == Some(true) && self.scenario_secret_values_found == Some(0)
    }
}

pub(crate) fn delete_temporary_credentials(
    coordinator: &CredentialCoordinator,
    repository: &SqliteRepository,
    evidence: &mut P0CleanupEvidence,
) -> bool {
    let Ok(catalog) = repository.load_catalog() else {
        return false;
    };
    let references = catalog
        .connections
        .values()
        .filter_map(|profile| profile.credential_ref.clone())
        .map(|reference| (reference.0.clone(), reference))
        .collect::<BTreeMap<_, _>>();
    evidence.temporary_vault_references_checked = Some(references.len());
    let profile_ids = catalog
        .connections
        .values()
        .filter(|profile| profile.credential_ref.is_some())
        .map(|profile| profile.id)
        .collect::<Vec<_>>();
    evidence.credential_profiles_deleted = Some(0);
    let mut clean = true;
    for profile_id in profile_ids {
        if coordinator
            .apply_catalog(CatalogMutation::Delete(profile_id), SecretUpdate::Unchanged)
            .is_ok()
        {
            evidence.credential_profiles_deleted = evidence
                .credential_profiles_deleted
                .map(|count| count.saturating_add(1));
        } else {
            clean = false;
        }
    }
    match repository.load_catalog() {
        Ok(catalog) => {
            evidence.credential_profiles_remaining = Some(
                catalog
                    .connections
                    .values()
                    .filter(|profile| profile.credential_ref.is_some())
                    .count(),
            );
        }
        Err(_) => clean = false,
    }
    match coordinator.reconcile() {
        Ok(report) => {
            evidence.journal_failed = Some(report.failed);
            evidence.journal_remaining = Some(report.remaining);
        }
        Err(_) => clean = false,
    }
    match count_remaining_references(coordinator, &references) {
        Some(remaining) => evidence.temporary_vault_references_remaining = Some(remaining),
        None => clean = false,
    }
    clean
        && evidence.credential_profiles_are_deleted()
        && evidence.journal_is_empty()
        && evidence.vault_references_are_absent()
}

pub(crate) fn scan_temporary_state(
    temporary_root: &Path,
    secret_environment: &[String],
    evidence: &mut P0CleanupEvidence,
) -> bool {
    let Some(secrets) = environment_values(secret_environment) else {
        return false;
    };
    evidence.scenario_secret_values_scanned = Some(secrets.len());
    let Some(scan) = scan_directory(temporary_root, &secrets) else {
        return false;
    };
    evidence.platform_files_scanned = Some(scan.files);
    evidence.scenario_secret_values_found = Some(scan.matches);
    evidence.state_scan_complete = Some(true);
    evidence.state_files_are_secret_free()
}

fn count_remaining_references(
    coordinator: &CredentialCoordinator,
    references: &BTreeMap<String, CredentialRef>,
) -> Option<usize> {
    let mut remaining = 0;
    for credential_ref in references.values() {
        match coordinator.get(credential_ref) {
            Ok(None) => {}
            Ok(Some(_)) => remaining += 1,
            Err(_) => return None,
        }
    }
    Some(remaining)
}

fn environment_values(names: &[String]) -> Option<Vec<Vec<u8>>> {
    names
        .iter()
        .map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .collect::<Option<Vec<_>>>()
        .map(|values| values.into_iter().map(String::into_bytes).collect())
}

#[derive(Default)]
struct StateScan {
    files: usize,
    matches: usize,
}

fn scan_directory(root: &Path, secrets: &[Vec<u8>]) -> Option<StateScan> {
    let mut scan = StateScan::default();
    scan_directory_into(root, secrets, &mut scan).then_some(scan)
}

fn scan_directory_into(root: &Path, secrets: &[Vec<u8>], scan: &mut StateScan) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let Ok(file_type) = entry.file_type() else {
            return false;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if !scan_directory_into(&entry.path(), secrets, scan) {
                return false;
            }
        } else if file_type.is_file() {
            let Ok(bytes) = fs::read(entry.path()) else {
                return false;
            };
            scan.files += 1;
            scan.matches += secrets
                .iter()
                .filter(|secret| {
                    bytes
                        .windows(secret.len())
                        .any(|window| window == secret.as_slice())
                })
                .count();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, time::SystemTime};

    use rshell_core::{CatalogMutation, ConnectionProfile, CredentialRef};
    use rshell_storage::{CredentialCoordinator, MemoryCredentialVault, SqliteRepository};

    use super::*;

    #[test]
    fn p0_cleanup_deletes_only_profiles_with_credential_references() {
        let repository = Arc::new(SqliteRepository::open_in_memory().unwrap());
        repository.migrate().unwrap();
        let coordinator = CredentialCoordinator::new(
            Arc::clone(&repository),
            Arc::new(MemoryCredentialVault::new()),
        );
        let mut credential_profile = ConnectionProfile::new("credential", "credential.test");
        credential_profile.credential_ref = Some(CredentialRef::new("rshell://credential/p0"));
        let plain_profile = ConnectionProfile::new("plain", "plain.test");
        let plain_profile_id = plain_profile.id;
        repository
            .apply(CatalogMutation::Create(credential_profile))
            .unwrap();
        repository
            .apply(CatalogMutation::Create(plain_profile))
            .unwrap();

        let mut evidence = P0CleanupEvidence::new();
        assert!(delete_temporary_credentials(
            &coordinator,
            repository.as_ref(),
            &mut evidence,
        ));
        assert_eq!(evidence.credential_profiles_deleted, Some(1));
        assert!(evidence.credential_profiles_are_deleted());
        assert!(evidence.vault_references_are_absent());
        assert!(evidence.journal_is_empty());
        let catalog = repository.load_catalog().unwrap();
        assert_eq!(catalog.connections.len(), 1);
        assert!(catalog.connections.contains_key(&plain_profile_id));
        repository.shutdown().unwrap();
    }

    #[test]
    fn state_scan_detects_secret_bytes_without_exposing_them() {
        let root = temporary_root();
        let name = "RSHELL_P0_CLEANUP_SCAN_SECRET";
        let value = "p0-root-scan-secret";
        unsafe { std::env::set_var(name, value) };
        fs::write(root.join("rshell.sqlite3-wal"), value).unwrap();

        let mut evidence = P0CleanupEvidence::new();
        assert!(!scan_temporary_state(&root, &[name.into()], &mut evidence));
        assert_eq!(evidence.platform_files_scanned, Some(1));
        assert_eq!(evidence.scenario_secret_values_scanned, Some(1));
        assert_eq!(evidence.scenario_secret_values_found, Some(1));

        unsafe { std::env::remove_var(name) };
        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rshell-p0-cleanup-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }
}
