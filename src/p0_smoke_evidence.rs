use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SmokeSurface {
    Gtk,
    LocalTerminal,
    NativePassword,
    NativeKey,
    NativeKeyboardInteractive,
    SystemAgent,
    HostKey,
    Vault,
    Imports,
    TabsSplits,
    Cleanup,
}

#[derive(Debug, Clone)]
pub(crate) struct ExternalObservationRequest {
    pub(crate) surface: SmokeSurface,
    pub(crate) path: PathBuf,
    pub(crate) run_nonce: String,
    pub(crate) fixture: String,
    pub(crate) connection: String,
    pub(crate) endpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QaObservation {
    ServerAuthentication,
    ServerChannel,
    ServerHostKeyPrompt,
    VaultCredentialReference,
    VaultDatabaseSecretScan,
    ActorCountZero,
    DirectChildCountZero,
    VaultTemporaryReferenceZero,
    JournalCountZero,
}

impl QaObservation {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ServerAuthentication => "server_authentication",
            Self::ServerChannel => "server_channel",
            Self::ServerHostKeyPrompt => "server_host_key_prompt",
            Self::VaultCredentialReference => "vault_credential_reference",
            Self::VaultDatabaseSecretScan => "vault_database_secret_scan",
            Self::ActorCountZero => "actor_count_zero",
            Self::DirectChildCountZero => "direct_child_count_zero",
            Self::VaultTemporaryReferenceZero => "vault_temporary_reference_zero",
            Self::JournalCountZero => "journal_count_zero",
        }
    }
}

impl SmokeSurface {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Gtk => "gtk",
            Self::LocalTerminal => "local_terminal",
            Self::NativePassword => "native_password",
            Self::NativeKey => "native_key",
            Self::NativeKeyboardInteractive => "native_keyboard_interactive",
            Self::SystemAgent => "system_agent",
            Self::HostKey => "host_key",
            Self::Vault => "vault",
            Self::Imports => "imports",
            Self::TabsSplits => "tabs_splits",
            Self::Cleanup => "cleanup",
        }
    }
}

#[derive(Default)]
pub(crate) struct QaEvidence {
    observations: BTreeMap<SmokeSurface, BTreeSet<QaObservation>>,
    bindings: BTreeMap<SmokeSurface, QaBinding>,
    errors: BTreeMap<SmokeSurface, &'static str>,
}

#[derive(Debug, Clone)]
pub(crate) struct QaBinding {
    pub(crate) run_nonce: String,
    pub(crate) fixture: String,
    pub(crate) connection: String,
    pub(crate) endpoint: String,
}

impl QaEvidence {
    pub(crate) fn load(requests: &[ExternalObservationRequest]) -> Self {
        let mut evidence = Self::default();
        for request in requests {
            if evidence.observations.contains_key(&request.surface)
                || evidence.errors.contains_key(&request.surface)
            {
                evidence
                    .errors
                    .insert(request.surface, "qa_observation_duplicate");
                continue;
            }
            match read_document(request) {
                Ok(observations) => {
                    evidence.observations.insert(request.surface, observations);
                    evidence.bindings.insert(
                        request.surface,
                        QaBinding {
                            run_nonce: request.run_nonce.clone(),
                            fixture: request.fixture.clone(),
                            connection: request.connection.clone(),
                            endpoint: request.endpoint.clone(),
                        },
                    );
                }
                Err(error) => {
                    evidence.errors.insert(request.surface, error);
                }
            }
        }
        evidence
    }

    pub(crate) fn has(&self, surface: SmokeSurface, observation: QaObservation) -> bool {
        self.observations
            .get(&surface)
            .is_some_and(|observations| observations.contains(&observation))
    }

    pub(crate) fn error(&self, surface: SmokeSurface) -> Option<&'static str> {
        self.errors.get(&surface).copied().or_else(|| {
            (!self.observations.contains_key(&surface)).then_some("qa_observation_file_missing")
        })
    }

    pub(crate) fn binding(&self, surface: SmokeSurface) -> Option<&QaBinding> {
        self.bindings.get(&surface)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QaObservationDocument {
    version: u16,
    generated_by: String,
    surface: SmokeSurface,
    run_nonce: String,
    fixture: String,
    connection: String,
    endpoint: String,
    observations: BTreeSet<QaObservation>,
}

fn read_document(
    request: &ExternalObservationRequest,
) -> Result<BTreeSet<QaObservation>, &'static str> {
    let document = read_with_retry(&request.path)?;
    let document: QaObservationDocument =
        serde_json::from_str(&document).map_err(|_| "qa_observation_file_invalid")?;
    if document.version != 1 || document.generated_by != "p0_qa" {
        return Err("qa_observation_file_invalid");
    }
    if document.surface != request.surface {
        return Err("qa_observation_surface_mismatch");
    }
    if document.run_nonce != request.run_nonce
        || document.fixture != request.fixture
        || document.connection != request.connection
        || document.endpoint != request.endpoint
    {
        return Err("qa_observation_binding_mismatch");
    }
    Ok(document.observations)
}

fn read_with_retry(path: &std::path::Path) -> Result<String, &'static str> {
    for attempt in 0..80 {
        match fs::read_to_string(path) {
            Ok(document) => return Ok(document),
            Err(_) if attempt < 79 => {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(_) => return Err("qa_observation_file_unreadable"),
        }
    }
    Err("qa_observation_file_unreadable")
}
