use std::{collections::BTreeSet, sync::Arc, time::Duration};

use rshell_core::{
    ConnectionRepository, CredentialOperationError, CredentialPort, ImportCandidateId, ImportError,
    ImportPort, ImportSourceKind, VaultFailure,
};
use rshell_storage::{
    CrashPoint, CredentialCoordinator, MemoryCredentialVault, MemoryVaultFault, SqliteRepository,
    VaultError, VaultOperation,
    ports::{CredentialPortAdapter, ImportPortAdapter, RepositoryPortAdapter},
};
use tempfile::NamedTempFile;

fn storage() -> (
    Arc<SqliteRepository>,
    Arc<MemoryCredentialVault>,
    Arc<CredentialCoordinator>,
) {
    let repository = Arc::new(SqliteRepository::open_in_memory().unwrap());
    repository.migrate().unwrap();
    let vault = Arc::new(MemoryCredentialVault::new());
    let coordinator = Arc::new(CredentialCoordinator::new(
        Arc::clone(&repository),
        vault.clone(),
    ));
    (repository, vault, coordinator)
}

#[tokio::test]
async fn repository_and_credential_adapters_map_real_in_memory_storage() {
    let (repository, vault, coordinator) = storage();
    let repository_port = RepositoryPortAdapter::new(Arc::clone(&repository));
    let credential_port = CredentialPortAdapter::new(coordinator);
    let mut profile = rshell_core::ConnectionProfile::new("managed", "example.test");
    profile.username = "alice".into();

    let catalog = credential_port
        .apply_catalog(
            rshell_core::CatalogMutation::Create(profile.clone()),
            rshell_core::SecretUpdate::Set(secrecy::SecretString::from("adapter-secret")),
        )
        .await
        .unwrap();
    let stored = catalog.connections.get(&profile.id).unwrap();
    let reference = stored.credential_ref.as_ref().unwrap();
    assert!(vault.contains(reference));
    assert!(credential_port.get(reference).await.unwrap().is_some());
    assert_eq!(vault.call_counts().get, 1);
    assert_eq!(repository_port.load_catalog().await.unwrap(), catalog);

    vault.inject_fault(MemoryVaultFault::before(
        VaultOperation::Get,
        2,
        VaultError::Denied,
    ));
    assert!(matches!(
        credential_port.get(reference).await,
        Err(CredentialOperationError::Vault(VaultFailure::Denied))
    ));
}

#[tokio::test]
async fn legacy_import_adapter_keeps_secret_preview_private_and_commits_once() {
    let (repository, vault, coordinator) = storage();
    let adapter = ImportPortAdapter::new(Arc::clone(&repository), coordinator);
    let file = NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{
            "folders": [],
            "connections": [{
                "id": "11111111-1111-4111-8111-111111111111",
                "name": "Imported",
                "host": "import.example",
                "port": 22,
                "user": "alice",
                "password": "import-secret",
                "backend": "wez_term_ssh"
            }]
        }"#,
    )
    .unwrap();

    let preview = adapter
        .preview(ImportSourceKind::LegacyRshellJson, file.path())
        .await
        .unwrap();
    assert_eq!(preview.candidates.len(), 1);
    assert!(preview.candidates[0].has_secret);
    assert!(!format!("{preview:?}").contains("import-secret"));
    let selected = BTreeSet::from([preview.candidates[0].id]);
    let result = adapter.commit(preview.id, &selected).await.unwrap();

    assert_eq!(result.report.imported_connections, 1);
    assert_eq!(result.catalog.connections.len(), 1);
    assert_eq!(adapter.pending_count(), 0);
    assert_eq!(vault.call_counts().put, 1);
    assert_eq!(
        adapter.commit(preview.id, &selected).await,
        Err(ImportError::PreviewExpired)
    );
}

#[tokio::test]
async fn import_preview_expiry_is_deterministic_on_tick_and_every_call() {
    let (repository, _vault, coordinator) = storage();
    let adapter = ImportPortAdapter::with_ttl(repository, coordinator, Duration::ZERO);
    let file = NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "Host test\n  HostName example.test\n").unwrap();
    let preview = adapter
        .preview(ImportSourceKind::OpenSshConfig, file.path())
        .await
        .unwrap();
    assert_eq!(adapter.pending_count(), 1);
    assert_eq!(adapter.cleanup_expired(), 1);
    assert_eq!(
        adapter.cancel(preview.id).await,
        Err(ImportError::PreviewExpired)
    );
}

#[tokio::test]
async fn cancelling_secret_preview_drops_pending_value_without_vault_write() {
    let (repository, vault, coordinator) = storage();
    let adapter = ImportPortAdapter::new(repository, coordinator);
    let file = NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        r#"{"folders":[],"connections":[{"id":"22222222-2222-4222-8222-222222222222","name":"cancel","host":"cancel.example","password":"drop-secret","backend":"wez_term_ssh"}]}"#,
    )
    .unwrap();
    let preview = adapter
        .preview(ImportSourceKind::LegacyRshellJson, file.path())
        .await
        .unwrap();
    assert!(preview.candidates[0].has_secret);
    assert_eq!(adapter.pending_count(), 1);

    adapter.cancel(preview.id).await.unwrap();

    assert_eq!(adapter.pending_count(), 0);
    assert_eq!(vault.call_counts().put, 0);
}

#[tokio::test]
async fn invalid_candidate_selection_does_not_consume_pending_preview() {
    let (repository, _vault, coordinator) = storage();
    let adapter = ImportPortAdapter::new(repository, coordinator);
    let file = NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "Host valid\n  HostName valid.example\n").unwrap();
    let preview = adapter
        .preview(ImportSourceKind::OpenSshConfig, file.path())
        .await
        .unwrap();
    let invalid = BTreeSet::from([ImportCandidateId::new()]);

    assert_eq!(
        adapter.commit(preview.id, &invalid).await,
        Err(ImportError::Validation)
    );
    assert_eq!(adapter.pending_count(), 1);
    adapter.cancel(preview.id).await.unwrap();
    assert_eq!(adapter.pending_count(), 0);
}

#[tokio::test]
async fn preview_maps_read_parse_and_validation_to_exact_core_categories() {
    let (repository, _vault, coordinator) = storage();
    let adapter = ImportPortAdapter::new(repository, coordinator);
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.json");
    assert_eq!(
        adapter
            .preview(ImportSourceKind::LegacyRshellJson, &missing)
            .await,
        Err(ImportError::Read)
    );

    let malformed = directory.path().join("malformed.json");
    std::fs::write(&malformed, "{").unwrap();
    assert_eq!(
        adapter
            .preview(ImportSourceKind::LegacyRshellJson, &malformed)
            .await,
        Err(ImportError::Parse)
    );

    let invalid = directory.path().join("invalid.json");
    std::fs::write(
        &invalid,
        r#"{"folders":[],"connections":[{"id":"not-a-uuid","name":"bad","host":"example.test","backend":"wez_term_ssh"}]}"#,
    )
    .unwrap();
    assert_eq!(
        adapter
            .preview(ImportSourceKind::LegacyRshellJson, &invalid)
            .await,
        Err(ImportError::Validation)
    );
    assert!(!format!("{:?}", ImportError::Read).contains("missing.json"));
}

#[tokio::test]
async fn import_commit_maps_already_imported_and_id_conflict() {
    let (repository, _vault, coordinator) = storage();
    let adapter = ImportPortAdapter::new(repository, coordinator);
    let first = NamedTempFile::new().unwrap();
    std::fs::write(
        first.path(),
        r#"{"folders":[],"connections":[{"id":"33333333-3333-4333-8333-333333333333","name":"first","host":"first.example","backend":"wez_term_ssh"}]}"#,
    )
    .unwrap();
    let preview = adapter
        .preview(ImportSourceKind::LegacyRshellJson, first.path())
        .await
        .unwrap();
    adapter
        .commit(preview.id, &BTreeSet::from([preview.candidates[0].id]))
        .await
        .unwrap();

    let replay = adapter
        .preview(ImportSourceKind::LegacyRshellJson, first.path())
        .await
        .unwrap();
    assert_eq!(
        adapter
            .commit(replay.id, &BTreeSet::from([replay.candidates[0].id]))
            .await,
        Err(ImportError::AlreadyImported)
    );

    let conflicting = NamedTempFile::new().unwrap();
    std::fs::write(
        conflicting.path(),
        r#"{"folders":[],"connections":[{"id":"33333333-3333-4333-8333-333333333333","name":"conflict","host":"other.example","backend":"wez_term_ssh"}]}"#,
    )
    .unwrap();
    let conflict = adapter
        .preview(ImportSourceKind::LegacyRshellJson, conflicting.path())
        .await
        .unwrap();
    assert_eq!(
        adapter
            .commit(conflict.id, &BTreeSet::from([conflict.candidates[0].id]),)
            .await,
        Err(ImportError::Conflict)
    );
}

#[tokio::test]
async fn import_commit_preserves_vault_storage_and_reconciliation_categories() {
    for expected in [
        ImportError::Vault,
        ImportError::Storage,
        ImportError::ReconciliationRequired,
    ] {
        let (repository, vault, coordinator) = storage();
        let adapter = ImportPortAdapter::new(Arc::clone(&repository), Arc::clone(&coordinator));
        let file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"{"folders":[],"connections":[{"id":"44444444-4444-4444-8444-444444444444","name":"managed","host":"managed.example","password":"private-value","backend":"wez_term_ssh"}]}"#,
        )
        .unwrap();
        let preview = adapter
            .preview(ImportSourceKind::LegacyRshellJson, file.path())
            .await
            .unwrap();
        match expected {
            ImportError::Vault => vault.inject_fault(MemoryVaultFault::before(
                VaultOperation::Put,
                1,
                VaultError::Denied,
            )),
            ImportError::Storage => repository.shutdown().unwrap(),
            ImportError::ReconciliationRequired => {
                coordinator.inject_crash_once(CrashPoint::AfterPrepare)
            }
            _ => unreachable!(),
        }
        let result = adapter
            .commit(preview.id, &BTreeSet::from([preview.candidates[0].id]))
            .await;
        assert_eq!(result, Err(expected));
        assert!(!format!("{result:?}").contains("private-value"));
    }
}
