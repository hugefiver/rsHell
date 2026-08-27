use rshell_ui::{SmokeActionKind, SmokeReport};

use crate::{
    p0_smoke_cleanup::P0CleanupEvidence,
    p0_smoke_contract_evidence as evidence,
    p0_smoke_evidence::{QaEvidence, QaObservation, SmokeSurface},
    p0_smoke_status::SurfaceStatuses,
};

pub(crate) fn assess_all(
    report: Option<&SmokeReport>,
    snapshot_exists: bool,
    state_removed: bool,
    cleanup: Option<&P0CleanupEvidence>,
    evidence: &QaEvidence,
) -> SurfaceStatuses {
    let native_auth = [
        (SmokeActionKind::OpenConnectionEditor, 1),
        (SmokeActionKind::SetConnectionField, 1),
        (SmokeActionKind::SubmitConnection, 1),
        (SmokeActionKind::SelectConnection, 1),
        (SmokeActionKind::Connect, 1),
        (SmokeActionKind::RespondHostKey, 1),
        (SmokeActionKind::WaitFrameContains, 1),
    ];
    SurfaceStatuses {
        gtk: evidence::assess(
            report,
            &[
                (SmokeActionKind::WaitWindowRealized, 1),
                (SmokeActionKind::VisualCheckpoint, 1),
            ],
            SmokeSurface::Gtk,
            &[
                (snapshot_exists, "window_png_snapshot"),
                (
                    report
                        .and_then(|value| value.counters.visual)
                        .is_some_and(|visual| visual.facts.contract_passes()),
                    "semantic_visual_contract",
                ),
                (
                    report
                        .and_then(|value| value.counters.visual)
                        .is_some_and(|visual| {
                            visual.png.is_some_and(|png| {
                                png.width == visual.facts.realized_width
                                    && png.height == visual.facts.realized_height
                                    && png.non_empty
                                    && png.dark_regions_required == 4
                                    && png.dark_regions_passed == 4
                                    && (2..=4).contains(&png.focus_or_selection_thickness_px)
                            })
                        }),
                    "range_png_visual_contract",
                ),
            ],
            evidence,
            &[],
        ),
        local_terminal: evidence::assess(
            report,
            &[
                (SmokeActionKind::NewTab, 1),
                (SmokeActionKind::SendTerminalText, 1),
                (SmokeActionKind::PasteTextFromEnv, 1),
                (SmokeActionKind::ResizeTerminal, 1),
                (SmokeActionKind::WaitFrameContains, 1),
                (SmokeActionKind::SearchTerminal, 1),
                (SmokeActionKind::SelectRange, 1),
                (SmokeActionKind::CopySelection, 1),
            ],
            SmokeSurface::LocalTerminal,
            &[(
                evidence::terminal_evidence_is_exact(report),
                "typed_terminal_outcome",
            )],
            evidence,
            &[],
        ),
        native_password: evidence::assess(
            report,
            &native_auth,
            SmokeSurface::NativePassword,
            &[],
            evidence,
            &[
                QaObservation::ServerAuthentication,
                QaObservation::ServerChannel,
            ],
        ),
        native_key: evidence::assess(
            report,
            &native_auth,
            SmokeSurface::NativeKey,
            &[],
            evidence,
            &[
                QaObservation::ServerAuthentication,
                QaObservation::ServerChannel,
            ],
        ),
        native_keyboard_interactive: evidence::assess(
            report,
            &[
                (SmokeActionKind::OpenConnectionEditor, 1),
                (SmokeActionKind::SetConnectionField, 1),
                (SmokeActionKind::SubmitConnection, 1),
                (SmokeActionKind::SelectConnection, 1),
                (SmokeActionKind::Connect, 1),
                (SmokeActionKind::RespondHostKey, 1),
                (SmokeActionKind::RespondAuth, 2),
                (SmokeActionKind::WaitFrameContains, 1),
            ],
            SmokeSurface::NativeKeyboardInteractive,
            &[],
            evidence,
            &[
                QaObservation::ServerAuthentication,
                QaObservation::ServerChannel,
            ],
        ),
        system_agent: evidence::assess(
            report,
            &[
                (SmokeActionKind::OpenConnectionEditor, 1),
                (SmokeActionKind::SetConnectionField, 1),
                (SmokeActionKind::SubmitConnection, 1),
                (SmokeActionKind::SelectConnection, 1),
                (SmokeActionKind::Connect, 1),
                (SmokeActionKind::SendTerminalText, 1),
                (SmokeActionKind::WaitFrameContains, 1),
            ],
            SmokeSurface::SystemAgent,
            &[],
            evidence,
            &[
                QaObservation::ServerAuthentication,
                QaObservation::ServerChannel,
            ],
        ),
        host_key: evidence::assess(
            report,
            &[
                (SmokeActionKind::Connect, 1),
                (SmokeActionKind::RespondHostKey, 1),
            ],
            SmokeSurface::HostKey,
            &[],
            evidence,
            &[QaObservation::ServerHostKeyPrompt],
        ),
        vault: evidence::assess(
            report,
            &[
                (SmokeActionKind::SetConnectionField, 1),
                (SmokeActionKind::SubmitConnection, 1),
            ],
            SmokeSurface::Vault,
            &[
                (
                    cleanup.is_some_and(P0CleanupEvidence::state_files_are_secret_free),
                    "temporary_state_secret_scan",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::vault_references_are_absent),
                    "temporary_vault_references_absent",
                ),
            ],
            evidence,
            &[],
        ),
        imports: evidence::assess(
            report,
            &[
                (SmokeActionKind::PreviewImport, 1),
                (SmokeActionKind::CommitImport, 1),
                (SmokeActionKind::CancelImport, 1),
            ],
            SmokeSurface::Imports,
            &[(
                evidence::import_evidence_is_exact(report),
                "typed_import_outcome",
            )],
            evidence,
            &[],
        ),
        tabs_splits: evidence::assess(
            report,
            &[
                (SmokeActionKind::NewTab, 2),
                (SmokeActionKind::SplitHorizontal, 1),
                (SmokeActionKind::SplitVertical, 1),
                (SmokeActionKind::SwitchTab, 1),
                (SmokeActionKind::Reconnect, 1),
            ],
            SmokeSurface::TabsSplits,
            &[(
                evidence::reconnect_evidence_is_exact(report),
                "typed_reconnect_outcome",
            )],
            evidence,
            &[],
        ),
        cleanup: evidence::assess(
            report,
            &[(SmokeActionKind::CloseAll, 1)],
            SmokeSurface::Cleanup,
            &[
                (state_removed, "temporary_platform_state_removed"),
                (
                    cleanup.is_some_and(P0CleanupEvidence::application_is_stopped),
                    "application_shutdown_clean",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::repository_is_stopped),
                    "repository_shutdown_clean",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::actors_are_stopped),
                    "actor_count_zero",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::direct_session_children_are_stopped),
                    "direct_child_count_zero",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::vault_references_are_absent),
                    "vault_temporary_reference_zero",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::credential_profiles_are_deleted),
                    "credential_profiles_zero",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::journal_is_empty),
                    "journal_count_zero",
                ),
                (
                    cleanup.is_some_and(P0CleanupEvidence::state_files_are_secret_free),
                    "temporary_state_secret_scan",
                ),
            ],
            evidence,
            &[],
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rshell_ui::{
        SmokeActionKind, SmokeCounters, SmokeReport, SmokeScenarioState, SmokeStepReport,
        SmokeStepState,
    };

    #[test]
    fn missing_ui_report_fails_instead_of_skipping() {
        let statuses = super::assess_all(None, false, false, None, &Default::default());
        assert_eq!(statuses.gtk.status, "failed");
        assert_eq!(statuses.cleanup.status, "failed");
    }

    #[test]
    fn cleanup_cannot_pass_from_external_observation_json() {
        let report = SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: vec![SmokeStepReport {
                index: 0,
                action: SmokeActionKind::CloseAll,
                surface: Some("cleanup".into()),
                connection: None,
                binding: None,
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
        let statuses = super::assess_all(Some(&report), false, false, None, &Default::default());
        let serialized = serde_json::to_value(statuses.cleanup).unwrap();
        assert!(
            serialized["missing_evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "actor_count_zero")
        );
    }

    fn report_with_passed_actions(actions: &[SmokeActionKind]) -> SmokeReport {
        SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: actions
                .iter()
                .enumerate()
                .map(|(index, action)| SmokeStepReport {
                    index,
                    action: *action,
                    surface: None,
                    connection: None,
                    binding: None,
                    state: SmokeStepState::Passed,
                    elapsed: Duration::ZERO,
                    evidence: SmokeCounters::default(),
                    field_status: None,
                })
                .collect(),
            counters: SmokeCounters::default(),
            failure: None,
            requested_png_path: None,
            png_path: None,
            png_error: None,
        }
    }

    #[test]
    fn terminal_surface_cannot_pass_from_dispatched_action_counters() {
        let report = report_with_passed_actions(&[
            SmokeActionKind::NewTab,
            SmokeActionKind::SendTerminalText,
            SmokeActionKind::PasteTextFromEnv,
            SmokeActionKind::ResizeTerminal,
            SmokeActionKind::WaitFrameContains,
            SmokeActionKind::SelectRange,
            SmokeActionKind::CopySelection,
        ]);
        let statuses = super::assess_all(Some(&report), false, false, None, &Default::default());
        assert_eq!(
            statuses.local_terminal.status, "failed",
            "terminal commands require exact geometry/search/selection/copy/TUI outcomes"
        );
    }

    #[test]
    fn import_surface_cannot_pass_from_revision_counters() {
        let report = report_with_passed_actions(&[
            SmokeActionKind::PreviewImport,
            SmokeActionKind::CommitImport,
            SmokeActionKind::PreviewImport,
            SmokeActionKind::CancelImport,
        ]);
        let statuses = super::assess_all(Some(&report), false, false, None, &Default::default());
        assert_eq!(
            statuses.imports.status, "failed",
            "import requires exact catalog and pending-preview outcomes"
        );
    }
}
