mod bootstrap;
mod cleanup;
mod p0_smoke;
mod p0_smoke_action_fields;
mod p0_smoke_actions;
mod p0_smoke_cleanup;
mod p0_smoke_contract;
mod p0_smoke_contract_binding;
mod p0_smoke_contract_evidence;
mod p0_smoke_contract_visual;
mod p0_smoke_evidence;
mod p0_smoke_report;
mod p0_smoke_report_steps;
mod p0_smoke_report_terminal;
mod p0_smoke_report_visual;
mod p0_smoke_runtime;
mod p0_smoke_scenario;
mod p0_smoke_status;

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use rshell_platform::PlatformPaths;
use rshell_ui::{StartupProbe, StartupReport};

use crate::bootstrap::BootstrapError;

pub(crate) const APPLICATION_ID: &str = "io.github.hugefiver.rshell";
const STARTUP_SMOKE_TIMEOUT: Duration = Duration::from_secs(15);

enum LaunchMode {
    Normal,
    SmokeStartup(PathBuf),
    SmokeP0 { scenario: PathBuf, report: PathBuf },
}

#[derive(Clone, Copy)]
pub(crate) enum RootError {
    Arguments,
    PlatformPaths,
    Bootstrap(BootstrapError),
    Gtk,
    SmokeTemp,
    SmokeCleanup,
    SmokeReport,
    SmokeIncomplete,
    P0Scenario,
    P0Timeout,
    P0Incomplete,
}

impl RootError {
    pub(crate) const fn category(self) -> &'static str {
        match self {
            Self::Arguments => "arguments",
            Self::PlatformPaths => "platform",
            Self::Bootstrap(error) => error.category(),
            Self::Gtk | Self::P0Timeout => "gtk",
            Self::SmokeTemp | Self::SmokeCleanup => "smoke-cleanup",
            Self::SmokeReport | Self::SmokeIncomplete | Self::P0Incomplete => "smoke-report",
            Self::P0Scenario => "smoke-scenario",
        }
    }

    pub(crate) const fn context(self) -> &'static str {
        match self {
            Self::Arguments => "parsing launch arguments",
            Self::PlatformPaths => "discovering platform paths",
            Self::Bootstrap(error) => error.context(),
            Self::Gtk => "running GTK application",
            Self::P0Timeout => "reaching the P0 GTK fail-safe timeout",
            Self::SmokeTemp => "creating smoke state",
            Self::SmokeCleanup => "removing smoke state",
            Self::SmokeReport => "writing smoke report",
            Self::SmokeIncomplete => "validating startup report",
            Self::P0Scenario => "parsing P0 scenario",
            Self::P0Incomplete => "validating P0 smoke evidence",
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Arguments => "arguments",
            Self::PlatformPaths => "platform_paths",
            Self::Bootstrap(error) => error.category(),
            Self::Gtk => "gtk",
            Self::SmokeTemp => "smoke_temp",
            Self::SmokeCleanup => "smoke_cleanup",
            Self::SmokeReport => "smoke_report",
            Self::SmokeIncomplete => "smoke_incomplete",
            Self::P0Scenario => "p0_scenario",
            Self::P0Timeout => "p0_timeout",
            Self::P0Incomplete => "p0_incomplete",
        }
    }
}

fn main() {
    init_tracing();
    let result = parse_arguments().and_then(|mode| match mode {
        LaunchMode::Normal => run_normal(),
        LaunchMode::SmokeStartup(report) => run_startup_smoke(&report),
        LaunchMode::SmokeP0 { scenario, report } => p0_smoke::run(&scenario, &report),
    });
    if let Err(error) = result {
        tracing::error!(
            category = error.category(),
            context = error.context(),
            "startup failed"
        );
        std::process::exit(1);
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .try_init();
}

fn parse_arguments() -> Result<LaunchMode, RootError> {
    let mut arguments = std::env::args_os().skip(1);
    match arguments.next() {
        None => Ok(LaunchMode::Normal),
        Some(flag) if flag == OsStr::new("--smoke-startup") => {
            single_path_argument(&mut arguments).map(LaunchMode::SmokeStartup)
        }
        Some(flag) if flag == OsStr::new("--smoke-p0") => {
            let scenario = required_path(&mut arguments)?;
            let report = required_path(&mut arguments)?;
            if arguments.next().is_some() {
                return Err(RootError::Arguments);
            }
            Ok(LaunchMode::SmokeP0 { scenario, report })
        }
        Some(_) => Err(RootError::Arguments),
    }
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, RootError> {
    arguments
        .next()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(RootError::Arguments)
}

fn single_path_argument(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<PathBuf, RootError> {
    let path = required_path(arguments)?;
    if arguments.next().is_some() {
        return Err(RootError::Arguments);
    }
    Ok(path)
}

fn run_normal() -> Result<(), RootError> {
    let paths = PlatformPaths::discover().map_err(|_| RootError::PlatformPaths)?;
    p0_smoke_runtime::run_normal(&paths)
}

fn run_startup_smoke(report_path: &Path) -> Result<(), RootError> {
    let (paths, temporary_root) = startup_smoke_paths()?;
    let probe = StartupProbe::for_gtk();
    let startup = p0_smoke_runtime::run_startup(&paths, probe.clone(), STARTUP_SMOKE_TIMEOUT);
    let state_removed = fs::remove_dir_all(&temporary_root).is_ok();
    let report = probe.report(startup.is_ok() && state_removed);
    let report_write = write_startup_report(report_path, report);

    match (startup, state_removed, report_write, report.is_complete()) {
        (Err(error), _, _, _) => Err(error),
        (_, false, _, _) => Err(RootError::SmokeCleanup),
        (_, _, Err(error), _) => Err(error),
        (_, _, _, false) => Err(RootError::SmokeIncomplete),
        (_, _, _, true) => Ok(()),
    }
}

fn startup_smoke_paths() -> Result<(PlatformPaths, PathBuf), RootError> {
    let suffix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let root = std::env::temp_dir().join(format!("rshell-startup-{}-{suffix}", std::process::id()));
    fs::create_dir(&root).map_err(|_| RootError::SmokeTemp)?;
    Ok((
        PlatformPaths::from_roots(root.join("config"), root.join("state"), root.join("cache")),
        root,
    ))
}

fn write_startup_report(path: &Path, report: StartupReport) -> Result<(), RootError> {
    let json = format!(
        concat!(
            "{{\"window_realized\":{},\"local_session_connected\":{},",
            "\"non_empty_render_frame\":{},\"shutdown_clean\":{},",
            "\"embedded_css_loaded\":{},\"embedded_icons_renderable\":{},",
            "\"embedded_icon_backend\":\"{}\",",
            "\"measured_terminal_geometry_ready\":{},\"scale_aware_icons_ready\":{},",
            "\"icon_backend\":\"{}\",\"icon_count\":{},",
            "\"adaptive_layout_modes\":{}}}\n"
        ),
        report.window_realized,
        report.local_session_connected,
        report.non_empty_render_frame,
        report.shutdown_clean,
        report.embedded_css_loaded,
        report.embedded_icons_renderable,
        report.embedded_icon_backend,
        report.measured_terminal_geometry_ready,
        report.scale_aware_icons_ready,
        report.icon_backend,
        report.icon_count,
        report.adaptive_layout_modes,
    );
    fs::write(path, json).map_err(|_| RootError::SmokeReport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_exact_p0_control_plane() {
        let arguments = ["--smoke-p0", "scenario.json", "report.json"]
            .into_iter()
            .map(std::ffi::OsString::from);
        let mode = match parse_from(arguments) {
            Ok(mode) => mode,
            Err(_) => panic!("P0 arguments must parse"),
        };
        assert!(matches!(mode, LaunchMode::SmokeP0 { .. }));
    }

    #[test]
    fn rejects_extra_p0_arguments() {
        let arguments = ["--smoke-p0", "scenario.json", "report.json", "extra"]
            .into_iter()
            .map(std::ffi::OsString::from);
        assert!(matches!(parse_from(arguments), Err(RootError::Arguments)));
    }

    fn parse_from(
        arguments: impl Iterator<Item = std::ffi::OsString>,
    ) -> Result<LaunchMode, RootError> {
        let mut arguments = arguments;
        match arguments.next() {
            Some(flag) if flag == "--smoke-p0" => {
                let scenario = required_path(&mut arguments)?;
                let report = required_path(&mut arguments)?;
                if arguments.next().is_some() {
                    return Err(RootError::Arguments);
                }
                Ok(LaunchMode::SmokeP0 { scenario, report })
            }
            _ => Err(RootError::Arguments),
        }
    }
}
