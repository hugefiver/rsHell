use std::{cell::Cell, path::Path, rc::Rc, time::Duration};

use relm4::{RelmApp, gtk::prelude::ApplicationExt};
use rshell_platform::PlatformPaths;
use rshell_ui::{MainWindow, MainWindowInit, SmokeDriverInit, SmokeReport, StartupProbe};

use crate::{
    RootError,
    bootstrap::{BootstrapObserver, create_runtime, start},
    p0_smoke_cleanup::P0CleanupEvidence,
};

pub(crate) struct P0GuiOutcome {
    pub(crate) result: Result<(), RootError>,
    pub(crate) report: Option<SmokeReport>,
    pub(crate) cleanup: Option<P0CleanupEvidence>,
}

pub(crate) fn run_normal(paths: &PlatformPaths) -> Result<(), RootError> {
    run_application(paths, |init| init, None, false)
}

pub(crate) fn run_startup(
    paths: &PlatformPaths,
    probe: StartupProbe,
    timeout: Duration,
) -> Result<(), RootError> {
    run_application(
        paths,
        |init| init.with_startup_probe(probe),
        Some(timeout),
        false,
    )
}

pub(crate) fn run_p0(
    paths: &PlatformPaths,
    temporary_root: &Path,
    secret_environment: &[String],
    driver: SmokeDriverInit,
    timeout: Duration,
) -> P0GuiOutcome {
    eprintln!("P0_SMOKE bootstrap_start");
    let runtime = match create_runtime().map_err(RootError::Bootstrap) {
        Ok(runtime) => runtime,
        Err(error) => {
            return P0GuiOutcome {
                result: Err(error),
                report: None,
                cleanup: None,
            };
        }
    };
    eprintln!("P0_SMOKE runtime_ready");
    let application = match runtime
        .block_on(start(paths, &BootstrapObserver::default()))
        .map_err(RootError::Bootstrap)
    {
        Ok(application) => application,
        Err(error) => {
            return P0GuiOutcome {
                result: Err(error),
                report: None,
                cleanup: None,
            };
        }
    };
    eprintln!("P0_SMOKE bootstrap_complete");
    let (init, report) =
        MainWindowInit::from_application(&application.application).with_smoke_driver(driver);
    eprintln!("P0_SMOKE gtk_start");
    let gui_result = run_gtk_application(init, Some(timeout), true);
    eprintln!("P0_SMOKE gtk_complete");
    eprintln!("P0_SMOKE cleanup_start");
    let p0_shutdown = runtime.block_on(application.shutdown_p0(temporary_root, secret_environment));
    eprintln!("P0_SMOKE cleanup_complete");
    let result = p0_shutdown
        .error
        .map_or(gui_result, |error| Err(RootError::Bootstrap(error)));
    P0GuiOutcome {
        result,
        report: Some(report.report()),
        cleanup: Some(p0_shutdown.evidence),
    }
}

fn run_application(
    paths: &PlatformPaths,
    configure: impl FnOnce(MainWindowInit) -> MainWindowInit,
    timeout: Option<Duration>,
    timeout_is_error: bool,
) -> Result<(), RootError> {
    let runtime = create_runtime().map_err(RootError::Bootstrap)?;
    let application = runtime
        .block_on(start(paths, &BootstrapObserver::default()))
        .map_err(RootError::Bootstrap)?;
    let init = configure(MainWindowInit::from_application(&application.application));
    complete_gui_and_cleanup(
        || run_gtk_application(init, timeout, timeout_is_error),
        || {
            runtime
                .block_on(application.shutdown())
                .map_err(RootError::Bootstrap)
        },
    )
}

fn run_gtk_application(
    init: MainWindowInit,
    timeout: Option<Duration>,
    timeout_is_error: bool,
) -> Result<(), RootError> {
    let timed_out = Rc::new(Cell::new(false));
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let app = RelmApp::new(crate::APPLICATION_ID).with_args(gtk_arguments(
            &std::env::args().next().unwrap_or_else(|| "rshell".into()),
        ));
        rshell_ui::apply_global_css();
        if let Some(timeout) = timeout {
            let timed_out = Rc::clone(&timed_out);
            relm4::gtk::glib::timeout_add_local_once(timeout, move || {
                eprintln!("P0_SMOKE gtk_timeout");
                timed_out.set(true);
                relm4::main_application().quit();
            });
        }
        app.run::<MainWindow>(init);
    }))
    .map_err(|_| RootError::Gtk)?;
    if timed_out.get() && timeout_is_error {
        Err(RootError::P0Timeout)
    } else {
        Ok(())
    }
}

fn complete_gui_and_cleanup(
    run_gui: impl FnOnce() -> Result<(), RootError>,
    cleanup: impl FnOnce() -> Result<(), RootError>,
) -> Result<(), RootError> {
    let gui_result = run_gui();
    let cleanup_result = cleanup();
    match cleanup_result {
        Err(error) => Err(error),
        Ok(()) => gui_result,
    }
}

pub(crate) fn gtk_arguments(program: &str) -> Vec<String> {
    vec![program.into()]
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    #[test]
    fn gtk_does_not_receive_root_control_arguments() {
        assert_eq!(super::gtk_arguments("rshell.exe"), vec!["rshell.exe"]);
    }

    #[test]
    fn cleanup_has_priority_after_a_gui_failure() {
        let cleaned = Cell::new(false);
        let result = super::complete_gui_and_cleanup(
            || Err(crate::RootError::Gtk),
            || {
                cleaned.set(true);
                Err(crate::RootError::SmokeCleanup)
            },
        );
        assert!(cleaned.get());
        assert!(matches!(result, Err(crate::RootError::SmokeCleanup)));
    }
}
