use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct SurfaceStatus {
    pub(crate) status: &'static str,
    observed_evidence: Vec<&'static str>,
    missing_evidence: Vec<&'static str>,
}

impl SurfaceStatus {
    pub(crate) fn missing(evidence: &'static str) -> Self {
        Self {
            status: "failed",
            observed_evidence: Vec::new(),
            missing_evidence: vec![evidence],
        }
    }

    pub(crate) fn from_evidence(
        observed_evidence: Vec<&'static str>,
        missing_evidence: Vec<&'static str>,
    ) -> Self {
        Self {
            status: if missing_evidence.is_empty() {
                "passed"
            } else {
                "failed"
            },
            observed_evidence,
            missing_evidence,
        }
    }
}

pub(crate) struct SurfaceStatuses {
    pub(crate) gtk: SurfaceStatus,
    pub(crate) local_terminal: SurfaceStatus,
    pub(crate) native_password: SurfaceStatus,
    pub(crate) native_key: SurfaceStatus,
    pub(crate) native_keyboard_interactive: SurfaceStatus,
    pub(crate) system_agent: SurfaceStatus,
    pub(crate) host_key: SurfaceStatus,
    pub(crate) vault: SurfaceStatus,
    pub(crate) imports: SurfaceStatus,
    pub(crate) tabs_splits: SurfaceStatus,
    pub(crate) cleanup: SurfaceStatus,
}

impl SurfaceStatuses {
    pub(crate) fn all_passed(&self) -> bool {
        [
            &self.gtk,
            &self.local_terminal,
            &self.native_password,
            &self.native_key,
            &self.native_keyboard_interactive,
            &self.system_agent,
            &self.host_key,
            &self.vault,
            &self.imports,
            &self.tabs_splits,
            &self.cleanup,
        ]
        .into_iter()
        .all(|status| status.status == "passed")
    }
}
