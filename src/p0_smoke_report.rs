use std::path::Path;

use rshell_ui::{SmokeReport, SmokeScenarioState, SmokeStepState};
use serde::Serialize;

use crate::{
    RootError,
    p0_smoke_cleanup::P0CleanupEvidence,
    p0_smoke_report_steps::{P0StepReport, convert_steps},
    p0_smoke_report_visual::{P0VisualEvidence, visual_evidence},
    p0_smoke_status::{SurfaceStatus, SurfaceStatuses},
};

#[derive(Serialize)]
pub(crate) struct P0SmokeReport {
    version: u16,
    run_nonce: Option<String>,
    state: &'static str,
    ui_state: Option<&'static str>,
    runner_error: Option<&'static str>,
    elapsed_ms: u128,
    requested_png_path: Option<String>,
    png_path: Option<String>,
    png_error: Option<&'static str>,
    cleanup_evidence: Option<P0CleanupEvidence>,
    visual: Option<P0VisualEvidence>,
    steps: Vec<P0StepReport>,
    gtk: SurfaceStatus,
    local_terminal: SurfaceStatus,
    native_password: SurfaceStatus,
    native_key: SurfaceStatus,
    native_keyboard_interactive: SurfaceStatus,
    system_agent: SurfaceStatus,
    host_key: SurfaceStatus,
    vault: SurfaceStatus,
    imports: SurfaceStatus,
    tabs_splits: SurfaceStatus,
    cleanup: SurfaceStatus,
}

impl P0SmokeReport {
    pub(crate) fn from_run(
        report: Option<&SmokeReport>,
        runner_error: Option<RootError>,
        cleanup_evidence: Option<&P0CleanupEvidence>,
        statuses: SurfaceStatuses,
    ) -> Self {
        let all_steps_passed = report.is_some_and(|value| {
            value.state == SmokeScenarioState::Passed
                && !value.steps.is_empty()
                && value
                    .steps
                    .iter()
                    .all(|step| step.state == SmokeStepState::Passed)
        });
        let mut state = if statuses.all_passed() && runner_error.is_none() && all_steps_passed {
            "passed"
        } else {
            "failed"
        };
        let (requested_png_path, png_path, png_error) =
            report.map_or((None, None, None), |value| {
                match (
                    value
                        .requested_png_path
                        .as_deref()
                        .map(artifact_name)
                        .transpose(),
                    value.png_path.as_deref().map(artifact_name).transpose(),
                ) {
                    (Ok(requested_png_path), Ok(png_path)) => {
                        (requested_png_path, png_path, value.png_error)
                    }
                    _ => (None, None, Some("artifact_path_invalid")),
                }
            });
        if png_error == Some("artifact_path_invalid") {
            state = "failed";
        }
        Self {
            version: report.map_or(1, |value| value.version),
            run_nonce: report.map(|value| value.run_nonce.clone()),
            state,
            ui_state: report.map(|value| scenario_state(value.state)),
            runner_error: runner_error.map(RootError::code),
            elapsed_ms: report.map_or(0, |value| value.elapsed.as_millis()),
            requested_png_path,
            png_path,
            png_error,
            cleanup_evidence: cleanup_evidence.cloned(),
            visual: report
                .and_then(|value| value.counters.visual.as_ref())
                .map(visual_evidence),
            steps: report.map_or_else(Vec::new, convert_steps),
            gtk: statuses.gtk,
            local_terminal: statuses.local_terminal,
            native_password: statuses.native_password,
            native_key: statuses.native_key,
            native_keyboard_interactive: statuses.native_keyboard_interactive,
            system_agent: statuses.system_agent,
            host_key: statuses.host_key,
            vault: statuses.vault,
            imports: statuses.imports,
            tabs_splits: statuses.tabs_splits,
            cleanup: statuses.cleanup,
        }
    }

    pub(crate) fn scenario_failure() -> Self {
        let status = || SurfaceStatus::missing("scenario_parse_failed");
        Self {
            version: 1,
            run_nonce: None,
            state: "failed",
            ui_state: None,
            runner_error: Some("p0_scenario"),
            elapsed_ms: 0,
            requested_png_path: None,
            png_path: None,
            png_error: None,
            cleanup_evidence: None,
            visual: None,
            steps: Vec::new(),
            gtk: status(),
            local_terminal: status(),
            native_password: status(),
            native_key: status(),
            native_keyboard_interactive: status(),
            system_agent: status(),
            host_key: status(),
            vault: status(),
            imports: status(),
            tabs_splits: status(),
            cleanup: status(),
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.state == "passed"
    }
}

const fn scenario_state(value: SmokeScenarioState) -> &'static str {
    match value {
        SmokeScenarioState::Pending => "pending",
        SmokeScenarioState::Running => "running",
        SmokeScenarioState::Passed => "passed",
        SmokeScenarioState::Failed => "failed",
    }
}

fn artifact_name(path: &Path) -> Result<String, &'static str> {
    let text = path.to_str().ok_or("artifact_path_invalid")?;
    if text.is_empty() || text.ends_with('/') || text.ends_with('\\') {
        return Err("artifact_path_invalid");
    }
    let leaf = text
        .rsplit(['/', '\\'])
        .next()
        .ok_or("artifact_path_invalid")?;
    if leaf.is_empty()
        || matches!(leaf, "." | "..")
        || leaf
            .chars()
            .any(|character| matches!(character, '/' | '\\' | ':'))
        || Path::new(leaf).is_absolute()
        || Path::new(leaf).components().count() != 1
    {
        return Err("artifact_path_invalid");
    }
    Ok(leaf.to_owned())
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use rshell_ui::{
        SmokeActionKind, SmokeCounters, SmokeReport, SmokeScenarioState, SmokeStepReport,
        SmokeStepState,
    };

    fn passed_statuses() -> super::SurfaceStatuses {
        let status = || super::SurfaceStatus::from_evidence(Vec::new(), Vec::new());
        super::SurfaceStatuses {
            gtk: status(),
            local_terminal: status(),
            native_password: status(),
            native_key: status(),
            native_keyboard_interactive: status(),
            system_agent: status(),
            host_key: status(),
            vault: status(),
            imports: status(),
            tabs_splits: status(),
            cleanup: status(),
        }
    }

    fn report_with_png_paths(
        requested_png_path: PathBuf,
        png_path: PathBuf,
    ) -> super::P0SmokeReport {
        let ui = SmokeReport {
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
            requested_png_path: Some(requested_png_path),
            png_path: Some(png_path),
            png_error: None,
        };
        super::P0SmokeReport::from_run(Some(&ui), None, None, passed_statuses())
    }

    #[test]
    fn absolute_paths_serialize_as_stable_artifact_names() {
        for absolute_path in [
            r"C:\Users\alice\work\artifacts\private.png",
            "/home/alice/work/artifacts/private.png",
        ] {
            let report =
                report_with_png_paths(PathBuf::from(absolute_path), PathBuf::from(absolute_path));
            let serialized = serde_json::to_value(&report).unwrap();
            let json = serde_json::to_string(&report).unwrap();

            assert!(serialized["requested_png_path"] == "private.png");
            assert!(serialized["png_path"] == "private.png");
            for forbidden in [
                r"C:\\Users\\alice",
                "/home/alice",
                env!("CARGO_MANIFEST_DIR"),
                "..",
            ] {
                assert!(!json.contains(forbidden));
            }
            for path in ["requested_png_path", "png_path"] {
                let name = serialized[path].as_str().unwrap();
                assert!(!name.contains(['/', '\\', ':']));
            }
        }
    }

    #[test]
    fn invalid_artifact_paths_fail_closed() {
        for invalid_path in [
            PathBuf::new(),
            PathBuf::from("trailing/"),
            PathBuf::from("trailing\\"),
            PathBuf::from("/"),
            PathBuf::from("."),
            PathBuf::from(".."),
            PathBuf::from("C:"),
        ] {
            let report = report_with_png_paths(invalid_path, PathBuf::from("private.png"));
            let serialized = serde_json::to_value(&report).unwrap();

            assert!(serialized["requested_png_path"].is_null());
            assert!(serialized["png_path"].is_null());
            assert_eq!(serialized["png_error"], "artifact_path_invalid");
            assert!(!report.is_complete());
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_artifact_path_fails_closed() {
        use std::os::unix::ffi::OsStringExt;

        let invalid_path =
            PathBuf::from(std::ffi::OsString::from_vec(b"private-\xff.png".to_vec()));
        let report = report_with_png_paths(invalid_path, PathBuf::from("private.png"));
        let serialized = serde_json::to_value(&report).unwrap();

        assert!(serialized["requested_png_path"].is_null());
        assert!(serialized["png_path"].is_null());
        assert_eq!(serialized["png_error"], "artifact_path_invalid");
        assert!(!report.is_complete());
    }

    #[test]
    fn scenario_parse_failure_serializes_each_fixed_surface_as_failed() {
        let report = serde_json::to_value(super::P0SmokeReport::scenario_failure()).unwrap();
        for name in [
            "gtk",
            "local_terminal",
            "native_password",
            "native_key",
            "native_keyboard_interactive",
            "system_agent",
            "host_key",
            "vault",
            "imports",
            "tabs_splits",
            "cleanup",
        ] {
            assert_eq!(report[name]["status"], "failed");
        }
    }

    #[test]
    fn passed_surfaces_cannot_hide_skipped_or_pending_steps() {
        let status = || super::SurfaceStatus::from_evidence(Vec::new(), Vec::new());
        let statuses = super::SurfaceStatuses {
            gtk: status(),
            local_terminal: status(),
            native_password: status(),
            native_key: status(),
            native_keyboard_interactive: status(),
            system_agent: status(),
            host_key: status(),
            vault: status(),
            imports: status(),
            tabs_splits: status(),
            cleanup: status(),
        };
        let ui = SmokeReport {
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
                state: SmokeStepState::Skipped,
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
        let report = super::P0SmokeReport::from_run(Some(&ui), None, None, statuses);
        assert!(
            !report.is_complete(),
            "root report must fail when any action is not passed"
        );
    }

    #[test]
    fn visual_facts_are_serialized_once_at_the_report_root() {
        use rshell_ui::{
            SmokeCounters, SmokeReport, SmokeScenarioState, SmokeVisualEvidence, SmokeVisualFacts,
        };

        let status = || super::SurfaceStatus::from_evidence(Vec::new(), Vec::new());
        let statuses = super::SurfaceStatuses {
            gtk: status(),
            local_terminal: status(),
            native_password: status(),
            native_key: status(),
            native_keyboard_interactive: status(),
            system_agent: status(),
            host_key: status(),
            vault: status(),
            imports: status(),
            tabs_splits: status(),
            cleanup: status(),
        };
        let facts = SmokeVisualFacts {
            requested_width: 1_360,
            requested_height: 860,
            realized_width: 1_360,
            realized_height: 852,
            command_bar: true,
            dense_sidebar: true,
            tab_strip: true,
            pane_command_row: true,
            terminal_canvas: true,
            content_dialog: true,
            embedded_icon_count: 13,
            focus_or_selection_treatment: true,
        };
        let ui = SmokeReport {
            version: 1,
            run_nonce: "unit".into(),
            state: SmokeScenarioState::Passed,
            elapsed: Duration::ZERO,
            steps: Vec::new(),
            counters: SmokeCounters {
                visual: Some(SmokeVisualEvidence { facts, png: None }),
                ..Default::default()
            },
            failure: None,
            requested_png_path: None,
            png_path: None,
            png_error: None,
        };
        let value = serde_json::to_value(super::P0SmokeReport::from_run(
            Some(&ui),
            None,
            None,
            statuses,
        ))
        .unwrap();
        assert_eq!(value["visual"]["facts"]["requested_width"], 1_360);
        assert_eq!(value["visual"]["facts"]["embedded_icon_count"], 13);
        assert_eq!(value["visual"]["facts"]["content_dialog"], true);
        assert!(value["visual"]["png"].is_null());
        assert!(value["steps"].as_array().unwrap().is_empty());
    }
}
