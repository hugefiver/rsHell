use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::SystemTime,
};

use rshell_platform::PlatformPaths;
use rshell_ui::SmokeDriverInit;

use crate::{
    RootError, p0_smoke_contract::assess_all, p0_smoke_evidence::QaEvidence,
    p0_smoke_report::P0SmokeReport, p0_smoke_runtime, p0_smoke_scenario,
};

const P0_GTK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub(crate) fn run(scenario_path: &Path, report_path: &Path) -> Result<(), RootError> {
    let parsed = match p0_smoke_scenario::read(scenario_path) {
        Ok(parsed) => parsed,
        Err(_) => return write_scenario_failure(report_path),
    };
    let (paths, temporary_root) = temporary_paths()?;
    let outcome = p0_smoke_runtime::run_p0(
        &paths,
        &temporary_root,
        &parsed.secret_environment,
        SmokeDriverInit::new(parsed.scenario).with_png_path(snapshot_path(report_path)),
        P0_GTK_TIMEOUT,
    );
    let cleanup = outcome.cleanup.unwrap_or_else(|| {
        let mut cleanup = crate::p0_smoke_cleanup::P0CleanupEvidence::new();
        let _ = crate::p0_smoke_cleanup::scan_temporary_state(
            &temporary_root,
            &parsed.secret_environment,
            &mut cleanup,
        );
        cleanup
    });
    let snapshot_exists = outcome.report.as_ref().is_some_and(|report| {
        report.png_path.as_ref().is_some_and(|path| path.is_file())
            && !report.png_paths.is_empty()
            && report.png_paths.iter().all(|path| path.is_file())
    });
    let state_removed = fs::remove_dir_all(&temporary_root).is_ok();
    let evidence = QaEvidence::load(&parsed.external_observations);
    let runner_error = outcome.result.err();
    let statuses = assess_all(
        outcome.report.as_ref(),
        snapshot_exists,
        state_removed,
        Some(&cleanup),
        &evidence,
    );
    let report = P0SmokeReport::from_run(
        outcome.report.as_ref(),
        runner_error,
        Some(&cleanup),
        statuses,
    );
    write_report(report_path, &report)?;
    match runner_error {
        Some(error) => Err(error),
        None if !state_removed => Err(RootError::SmokeCleanup),
        None if !report.is_complete() => Err(RootError::P0Incomplete),
        None => Ok(()),
    }
}

fn write_scenario_failure(path: &Path) -> Result<(), RootError> {
    write_report(path, &P0SmokeReport::scenario_failure())?;
    Err(RootError::P0Scenario)
}

fn temporary_paths() -> Result<(PlatformPaths, PathBuf), RootError> {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..32 {
        let root = std::env::temp_dir().join(format!(
            "rshell-p0-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&root) {
            Ok(()) => {
                return Ok((
                    PlatformPaths::from_roots(
                        root.join("config"),
                        root.join("state"),
                        root.join("cache"),
                    ),
                    root,
                ));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(RootError::SmokeTemp),
        }
    }
    Err(RootError::SmokeTemp)
}

fn snapshot_path(report_path: &Path) -> PathBuf {
    let parent = report_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = report_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("p0-smoke-report");
    parent.join(format!("{stem}.png"))
}

fn write_report(path: &Path, report: &P0SmokeReport) -> Result<(), RootError> {
    let mut json = serde_json::to_vec_pretty(report).map_err(|_| RootError::SmokeReport)?;
    json.push(b'\n');
    fs::write(path, json).map_err(|_| RootError::SmokeReport)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn snapshot_is_a_real_sibling_output_not_temporary_platform_state() {
        assert_eq!(
            super::snapshot_path(Path::new("artifacts/p0-report.json")),
            Path::new("artifacts/p0-report.png")
        );
    }
}
