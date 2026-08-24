use rshell_ui::{SmokeActionKind, SmokeReport, SmokeStepState};

use crate::{
    p0_smoke_contract_binding::{step_binding_matches_surface, surface_binding_is_verified},
    p0_smoke_evidence::{QaEvidence, QaObservation, SmokeSurface},
    p0_smoke_status::SurfaceStatus,
};

pub(crate) fn terminal_evidence_is_exact(report: Option<&SmokeReport>) -> bool {
    report.is_some_and(|report| {
        let aggregate = report.steps.iter().any(|step| {
            if step.state != SmokeStepState::Passed
                || step.surface.as_deref() != Some(SmokeSurface::LocalTerminal.as_str())
            {
                return false;
            }
            let evidence = &step.evidence.terminal;
            evidence.resize.is_some_and(|resize| {
                resize.sequence > 0 && resize.exact && resize.observed.is_some()
            }) && evidence.search.is_some_and(|search| {
                search.sequence > 0
                    && search.completed
                    && search.match_count > 0
                    && search.current.is_some()
            }) && evidence.selection.is_some_and(|selection| {
                selection.sequence > 0 && selection.frame_confirmed && selection.wide_midpoint
            }) && evidence.clipboard.is_some_and(|clipboard| {
                clipboard.sequence > 0
                    && clipboard.actor_exact
                    && clipboard.gtk_written
                    && clipboard.actual_bytes == clipboard.expected_bytes
            }) && evidence.tui_entered
                && evidence.tui_exited
        });
        let paste = report.steps.iter().any(|step| {
            step.state == SmokeStepState::Passed
                && step.surface.as_deref() == Some(SmokeSurface::LocalTerminal.as_str())
                && step.action == SmokeActionKind::PasteTextFromEnv
                && step.evidence.terminal.paste.is_some_and(|evidence| {
                    evidence.sequence > 0
                        && evidence.command_exact
                        && evidence.frame_effect
                        && evidence.expected_bytes > 0
                        && evidence.actual_bytes == evidence.expected_bytes
                })
        });
        let color = report.steps.iter().any(|step| {
            step.state == SmokeStepState::Passed
                && step.surface.as_deref() == Some(SmokeSurface::LocalTerminal.as_str())
                && step.action == SmokeActionKind::SendTerminalText
                && step.evidence.terminal.color.is_some_and(|evidence| {
                    evidence.sequence > 0
                        && evidence.marker_bytes > 0
                        && evidence.marker_cells > 0
                        && evidence.non_default_foreground
                        && evidence.red_foreground
                })
        });
        aggregate && paste && color
    })
}

pub(crate) fn import_evidence_is_exact(report: Option<&SmokeReport>) -> bool {
    report.is_some_and(|report| {
        let legacy_commit = report.steps.iter().any(|step| {
            let evidence = &step.evidence.imports;
            import_step(step, SmokeActionKind::CommitImport)
                && evidence.sequence > 0
                && evidence.completed
                && evidence.commit_source == Some(rshell_core::ImportSourceKind::LegacyRshellJson)
                && evidence.imported_groups == evidence.expected_groups
                && evidence.imported_connections == evidence.expected_connections
                && evidence.exact_group
                && evidence.exact_connection
                && evidence.authentication_matches
                && evidence.credential_reference_matches
                && evidence.terminal_override_matches
        });
        let openssh_preview = report.steps.iter().any(|step| {
            import_step(step, SmokeActionKind::PreviewImport)
                && step
                    .evidence
                    .imports
                    .preview
                    .as_ref()
                    .is_some_and(openssh_preview_is_exact)
        });
        let openssh_cancel = report.steps.iter().any(|step| {
            let evidence = &step.evidence.imports;
            import_step(step, SmokeActionKind::CancelImport)
                && evidence.cancel_sequence > 0
                && evidence.cancelled_preview_matches
                && evidence.cancel_pending_zero
                && evidence.pending_preview_count == 0
                && evidence
                    .preview
                    .as_ref()
                    .is_some_and(openssh_preview_is_exact)
        });
        legacy_commit && openssh_preview && openssh_cancel
    })
}

fn import_step(step: &rshell_ui::SmokeStepReport, action: SmokeActionKind) -> bool {
    step.state == SmokeStepState::Passed
        && step.surface.as_deref() == Some(SmokeSurface::Imports.as_str())
        && step.action == action
}

fn openssh_preview_is_exact(evidence: &rshell_ui::SmokeImportPreviewEvidence) -> bool {
    evidence.sequence > 0
        && evidence.source == rshell_core::ImportSourceKind::OpenSshConfig
        && evidence.actual_groups == evidence.expected_groups
        && evidence.actual_candidates == evidence.expected_candidates
        && evidence.actual_candidate_name.is_some()
        && evidence.actual_host.is_some()
        && evidence.authentication.is_some()
        && evidence.credential_reference_present.is_some()
        && evidence.terminal_override_present.is_some()
        && evidence.importable.is_some()
        && evidence.wildcard.is_some()
        && evidence.exact_group
        && evidence.exact_candidate
        && evidence.authentication_matches
        && evidence.credential_reference_matches
        && evidence.terminal_override_matches
        && evidence.importable_matches
        && evidence.wildcard_matches
}

pub(crate) fn reconnect_evidence_is_exact(report: Option<&SmokeReport>) -> bool {
    report.is_some_and(|report| {
        report.steps.iter().any(|step| {
            step.state == SmokeStepState::Passed
                && step.surface.as_deref() == Some(SmokeSurface::TabsSplits.as_str())
                && step.action == SmokeActionKind::Reconnect
                && step.evidence.terminal.reconnect.is_some_and(|evidence| {
                    evidence.sequence > 0
                        && evidence.new_session.is_some()
                        && evidence.old_session_absent
                })
        })
    })
}

pub(crate) fn assess(
    report: Option<&SmokeReport>,
    actions: &[(SmokeActionKind, usize)],
    surface: SmokeSurface,
    checks: &[(bool, &'static str)],
    evidence: &QaEvidence,
    facts: &[QaObservation],
) -> SurfaceStatus {
    let Some(report) = report else {
        return SurfaceStatus::missing("ui_smoke_report_missing");
    };
    let mut observed = Vec::new();
    let mut missing = Vec::new();
    for (action, count) in actions {
        let passed = report
            .steps
            .iter()
            .filter(|step| {
                step.action == *action
                    && step_binding_matches_surface(report, step, surface, evidence)
            })
            .count();
        if passed >= *count {
            observed.push(action.as_str());
        } else {
            missing.push(action.as_str());
        }
    }
    for (passed, label) in checks {
        if *passed {
            observed.push(label);
        } else {
            missing.push(label);
        }
    }
    if surface_binding_is_verified(report, surface, evidence) {
        observed.push("actual_component_binding");
    } else {
        missing.push("actual_component_binding");
    }
    if !facts.is_empty() {
        if let Some(error) = evidence.error(surface) {
            missing.push(error);
        } else {
            for fact in facts {
                if evidence.has(surface, *fact) {
                    observed.push(fact.as_str());
                } else {
                    missing.push(fact.as_str());
                }
            }
        }
    }
    SurfaceStatus::from_evidence(observed, missing)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rshell_ui::{
        SmokeActionKind, SmokeBindingEvidence, SmokeCounters, SmokeReport, SmokeScenarioState,
        SmokeStepReport, SmokeStepState,
    };

    #[test]
    fn swapped_surface_and_connection_identity_cannot_pass_component_binding() {
        let report = SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: vec![SmokeStepReport {
                index: 0,
                action: SmokeActionKind::Connect,
                surface: Some("native_password".into()),
                connection: Some("native_key".into()),
                binding: Some(SmokeBindingEvidence {
                    verified: true,
                    component_verified: true,
                    actual_label: Some("native_key".into()),
                    profile_name: Some("native_key".into()),
                    endpoint: Some("127.0.0.1:2222".into()),
                    ..Default::default()
                }),
                state: SmokeStepState::Passed,
                elapsed: Duration::ZERO,
                evidence: SmokeCounters::default(),
                field_status: None,
            }],
            counters: SmokeCounters::default(),
            failure: None,
            requested_png_path: None,
            png_path: None,
            png_error: None,
        };
        let status = super::assess(
            Some(&report),
            &[(SmokeActionKind::Connect, 1)],
            crate::p0_smoke_evidence::SmokeSurface::NativePassword,
            &[],
            &Default::default(),
            &[],
        );
        let value = serde_json::to_value(status).expect("serialize status");
        assert!(
            value["missing_evidence"]
                .as_array()
                .expect("missing")
                .iter()
                .any(|item| item == "actual_component_binding")
        );
    }

    #[test]
    fn editor_actions_from_another_actual_connection_cannot_satisfy_surface_counts() {
        let expected = rshell_core::ConnectionId::new();
        let other = rshell_core::ConnectionId::new();
        let mut report = SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: Vec::new(),
            counters: SmokeCounters::default(),
            failure: None,
            requested_png_path: None,
            png_path: None,
            png_error: None,
        };
        for (index, (action, connection_id, actual_label, profile_name)) in [
            (
                SmokeActionKind::OpenConnectionEditor,
                other,
                "connection_editor",
                None,
            ),
            (
                SmokeActionKind::SubmitConnection,
                expected,
                "native_password",
                Some("native_password"),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            report.steps.push(SmokeStepReport {
                index,
                action,
                surface: Some("native_password".into()),
                connection: Some("native_password".into()),
                binding: Some(SmokeBindingEvidence {
                    verified: true,
                    component_verified: true,
                    actual_label: Some(actual_label.into()),
                    connection_id: Some(connection_id),
                    profile_name: profile_name.map(str::to_owned),
                    ..Default::default()
                }),
                state: SmokeStepState::Passed,
                elapsed: Duration::ZERO,
                evidence: SmokeCounters::default(),
                field_status: None,
            });
        }
        let status = super::assess(
            Some(&report),
            &[
                (SmokeActionKind::OpenConnectionEditor, 1),
                (SmokeActionKind::SubmitConnection, 1),
            ],
            crate::p0_smoke_evidence::SmokeSurface::NativePassword,
            &[],
            &Default::default(),
            &[],
        );
        let value = serde_json::to_value(status).expect("serialize status");
        assert!(
            value["missing_evidence"]
                .as_array()
                .expect("missing")
                .iter()
                .any(|item| item == "open_connection_editor")
        );
    }

    #[test]
    fn legacy_commit_without_exact_openssh_preview_and_cancel_cannot_pass_imports() {
        let mut counters = SmokeCounters::default();
        counters.imports.sequence = 7;
        counters.imports.completed = true;
        counters.imports.commit_source = Some(rshell_core::ImportSourceKind::LegacyRshellJson);
        counters.imports.expected_groups = 1;
        counters.imports.expected_connections = 1;
        counters.imports.imported_groups = 1;
        counters.imports.imported_connections = 1;
        counters.imports.exact_group = true;
        counters.imports.exact_connection = true;
        counters.imports.authentication_matches = true;
        counters.imports.credential_reference_matches = true;
        counters.imports.terminal_override_matches = true;
        counters.imports.pending_preview_count = 0;
        counters.imports.cancel_pending_zero = true;
        counters.imports.cancel_sequence = 8;
        counters.imports.cancelled_preview_matches = true;
        let report = SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: vec![SmokeStepReport {
                index: 0,
                action: SmokeActionKind::CommitImport,
                surface: Some("imports".into()),
                connection: Some("legacy_import".into()),
                binding: Some(SmokeBindingEvidence {
                    verified: true,
                    component_verified: true,
                    actual_label: Some("imports".into()),
                    ..Default::default()
                }),
                state: SmokeStepState::Passed,
                elapsed: Duration::ZERO,
                evidence: counters,
                field_status: None,
            }],
            counters: SmokeCounters::default(),
            failure: None,
            requested_png_path: None,
            png_path: None,
            png_error: None,
        };
        assert!(!super::import_evidence_is_exact(Some(&report)));
    }
}
