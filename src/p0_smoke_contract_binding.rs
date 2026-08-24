use rshell_core::ConnectionId;
use rshell_ui::{SmokeReport, SmokeStepReport, SmokeStepState};

use crate::p0_smoke_evidence::{QaEvidence, SmokeSurface};

pub(crate) fn step_binding_matches_surface(
    report: &SmokeReport,
    step: &SmokeStepReport,
    surface: SmokeSurface,
    evidence: &QaEvidence,
) -> bool {
    if step.state != SmokeStepState::Passed || step.surface.as_deref() != Some(surface.as_str()) {
        return false;
    }
    let Some(binding) = step
        .binding
        .as_ref()
        .filter(|binding| binding.verified && binding.component_verified)
    else {
        return false;
    };
    match surface {
        SmokeSurface::NativePassword
        | SmokeSurface::NativeKey
        | SmokeSurface::NativeKeyboardInteractive
        | SmokeSurface::SystemAgent
        | SmokeSurface::HostKey
        | SmokeSurface::Vault => surface_connection_id(report, surface, evidence)
            .is_some_and(|connection| binding.connection_id == Some(connection)),
        SmokeSurface::LocalTerminal => {
            binding.local && binding.actual_label.as_deref() == Some("local")
        }
        SmokeSurface::TabsSplits => binding.actual_label.as_deref() == Some("workspace"),
        SmokeSurface::Imports => matches!(
            binding.actual_label.as_deref(),
            Some("import_preview" | "import_catalog")
        ),
        SmokeSurface::Gtk => binding.actual_label.as_deref() == Some("main_window"),
        SmokeSurface::Cleanup => binding.actual_label.as_deref() == Some("shutdown"),
    }
}

pub(crate) fn surface_binding_is_verified(
    report: &SmokeReport,
    surface: SmokeSurface,
    evidence: &QaEvidence,
) -> bool {
    match surface {
        SmokeSurface::NativePassword
        | SmokeSurface::NativeKey
        | SmokeSurface::NativeKeyboardInteractive
        | SmokeSurface::SystemAgent
        | SmokeSurface::HostKey
        | SmokeSurface::Vault => surface_connection_id(report, surface, evidence).is_some(),
        _ => report
            .steps
            .iter()
            .any(|step| step_binding_matches_surface(report, step, surface, evidence)),
    }
}

fn surface_connection_id(
    report: &SmokeReport,
    surface: SmokeSurface,
    evidence: &QaEvidence,
) -> Option<ConnectionId> {
    report.steps.iter().find_map(|step| {
        if step.state != SmokeStepState::Passed || step.surface.as_deref() != Some(surface.as_str())
        {
            return None;
        }
        let binding = step.binding.as_ref().filter(|binding| {
            binding.verified
                && binding.component_verified
                && binding.actual_label.as_deref() == Some(surface.as_str())
                && binding.profile_name.as_deref() == Some(surface.as_str())
        })?;
        if let Some(qa) = evidence.binding(surface) {
            let qa_matches = binding.pane_id.is_some()
                && binding.session_id.is_some()
                && qa.connection == binding.profile_name.as_deref().unwrap_or_default()
                && binding.endpoint.as_deref() == Some(qa.endpoint.as_str())
                && qa.run_nonce == report.run_nonce
                && !qa.fixture.is_empty();
            if !qa_matches {
                return None;
            }
        }
        binding.connection_id
    })
}
