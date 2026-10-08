use super::completion_bridge_ports::Ports;
use async_trait::async_trait;
use rshell_core::*;
use secrecy::SecretString;
use std::{collections::BTreeSet, path::Path};

#[async_trait]
impl ConnectionRepository for Ports {
    async fn load_catalog(&self) -> Result<ConnectionCatalog, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn apply(&self, _: CatalogMutation) -> Result<ConnectionCatalog, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn load_terminal_profiles(&self) -> Result<Vec<TerminalProfile>, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn save_terminal_profile(&self, _: TerminalProfile) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn load_settings(&self) -> Result<AppSettings, RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
    async fn save_settings(&self, _: AppSettings) -> Result<(), RepositoryError> {
        Err(RepositoryError::Unavailable)
    }
}
#[async_trait]
impl CredentialPort for Ports {
    async fn apply_catalog(
        &self,
        _: CatalogMutation,
        _: SecretUpdate,
    ) -> Result<ConnectionCatalog, CredentialOperationError> {
        panic!("no credential operation")
    }
    async fn get(
        &self,
        _: &CredentialRef,
    ) -> Result<Option<SecretString>, CredentialOperationError> {
        panic!("no credential access")
    }
}
#[async_trait]
impl ImportPort for Ports {
    async fn preview(
        &self,
        _: ImportSourceKind,
        _: &Path,
    ) -> Result<ImportPreviewView, ImportError> {
        Err(ImportError::Validation)
    }
    async fn commit(
        &self,
        _: ImportPreviewId,
        _: &BTreeSet<ImportCandidateId>,
    ) -> Result<ImportCommitResult, ImportError> {
        Err(ImportError::Validation)
    }
    async fn cancel(&self, _: ImportPreviewId) -> Result<(), ImportError> {
        Err(ImportError::Validation)
    }
}
