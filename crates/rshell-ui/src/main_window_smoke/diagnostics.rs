use gtk::prelude::*;
use relm4::gtk;

use crate::{
    MainWindow, ShellLayoutMode, SmokeAction, SmokeStepState, smoke_driver_state::SmokeDriver,
};

pub(crate) struct ResizeDiagnostics {
    width: i32,
    height: i32,
    expected: ShellLayoutMode,
}

impl ResizeDiagnostics {
    fn for_route(action: &SmokeAction, index: Option<usize>, enabled: bool) -> Option<Self> {
        if !enabled || index != Some(1) {
            return None;
        }
        let SmokeAction::ResizeWindow {
            width,
            height,
            expected_mode,
        } = action
        else {
            return None;
        };
        Some(Self {
            width: *width,
            height: *height,
            expected: *expected_mode,
        })
    }

    fn terminal_snapshot(
        pending: &mut Option<Self>,
        driver: &SmokeDriver,
    ) -> Option<(Self, &'static str)> {
        if pending.is_none() {
            return None;
        }
        let report = driver.report.report();
        let phase = match report.steps.get(1).map(|step| step.state) {
            Some(SmokeStepState::Passed) => "terminal_passed",
            Some(SmokeStepState::Failed) => "terminal_failed",
            _ => return None,
        };
        pending.take().map(|diagnostic| (diagnostic, phase))
    }

    fn snapshot(&self, window: &MainWindow, phase: &str) {
        let layout = window.shell.layout().mode;
        let root = window
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok());
        let Some(root) = root else {
            eprintln!(
                "P0_RESIZE_DIAGNOSTIC phase={phase} requested={}x{} expected={:?} layout={layout:?} window=unavailable",
                self.width, self.height, self.expected
            );
            return;
        };
        let surface = root.surface();
        let surface_size = surface
            .as_ref()
            .map(|surface| (surface.width(), surface.height(), surface.scale_factor()));
        let monitor = surface.as_ref().and_then(|surface| {
            gtk::prelude::WidgetExt::display(&root)
                .monitor_at_surface(surface)
                .map(|monitor| {
                    let geometry = monitor.geometry();
                    (
                        geometry.x(),
                        geometry.y(),
                        geometry.width(),
                        geometry.height(),
                        monitor.scale_factor(),
                    )
                })
        });
        eprintln!(
            "P0_RESIZE_DIAGNOSTIC phase={phase} requested={}x{} maximized={} fullscreen={} default_size={:?} mapped={} realized={} widget={}x{} widget_scale={} surface={surface_size:?} layout={layout:?} expected={:?} monitor_geometry_xywh_scale={monitor:?} monitor_workarea=unavailable",
            self.width,
            self.height,
            root.is_maximized(),
            root.is_fullscreen(),
            root.default_size(),
            root.is_mapped(),
            root.is_realized(),
            root.width(),
            root.height(),
            root.scale_factor(),
            self.expected,
        );
    }
}

impl MainWindow {
    pub(super) fn route_smoke_action_with_resize_diagnostics(
        &mut self,
        action: SmokeAction,
    ) -> Result<bool, &'static str> {
        let index = self
            .smoke
            .as_ref()
            .and_then(|driver| driver.current.as_ref().map(|step| step.index));
        let first = self.smoke_state.resize_diagnostics.is_none()
            && ResizeDiagnostics::for_route(
                &action,
                index,
                cfg!(target_os = "linux")
                    && std::env::var("RSHELL_P0_RESIZE_DIAGNOSTICS").as_deref() == Ok("1"),
            )
            .map(|diagnostic| {
                diagnostic.snapshot(self, "before");
                self.smoke_state.resize_diagnostics = Some(diagnostic);
            })
            .is_some();
        let result = self.route_smoke_action(action);
        if first && let Some(diagnostic) = &self.smoke_state.resize_diagnostics {
            diagnostic.snapshot(self, "after_route");
        }
        if result.is_err()
            && let Some(diagnostic) = self.smoke_state.resize_diagnostics.take()
        {
            diagnostic.snapshot(self, "terminal_route_error");
        }
        result
    }

    pub(super) fn snapshot_smoke_resize_terminal(&mut self) {
        let Some(driver) = self.smoke.as_ref() else {
            return;
        };
        if let Some((diagnostic, phase)) =
            ResizeDiagnostics::terminal_snapshot(&mut self.smoke_state.resize_diagnostics, driver)
        {
            diagnostic.snapshot(self, phase);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::{
        SmokeCounters, SmokeDriverInit, SmokeReportHandle, SmokeScenario, SmokeScenarioState,
        SmokeWindowResizeEvidence, smoke_driver_observation::SmokeObservation,
        smoke_driver_state::SmokeDecision,
    };

    fn observed() -> SmokeObservation {
        SmokeObservation {
            window_realized: true,
            editor_open: false,
            sidebar_selection: None,
            connection_panes: BTreeSet::new(),
            import_preview_ready: false,
            active_tab: None,
            tab_ids: Vec::new(),
            shutdown_complete: false,
            active_interaction: None,
            answered_prompts: Vec::new(),
            last_interaction_response: None,
            binding: None,
            counters: SmokeCounters::default(),
        }
    }

    fn resize_driver() -> (SmokeDriver, SmokeObservation, Option<ResizeDiagnostics>) {
        let resize = SmokeAction::ResizeWindow {
            width: 800,
            height: 600,
            expected_mode: ShellLayoutMode::Compact,
        };
        let init = SmokeDriverInit::new(SmokeScenario::new(vec![
            SmokeAction::WaitWindowRealized,
            resize.clone(),
            SmokeAction::CloseAll,
        ]));
        let report = SmokeReportHandle::new(&init);
        let mut driver = SmokeDriver::new(init, report.clone());
        let observed = observed();
        assert!(driver.tick(&observed, |_| false).is_none());
        assert!(matches!(
            driver.tick(&observed, |_| false),
            Some(SmokeDecision::Route(SmokeAction::ResizeWindow { .. }))
        ));
        let pending = ResizeDiagnostics::for_route(&resize, Some(1), true);
        (driver, observed, pending)
    }

    #[test]
    fn first_resize_timeout_keeps_the_failure_and_yields_one_terminal_snapshot() {
        let (mut driver, observed, mut pending) = resize_driver();
        let action = &driver.current.as_ref().unwrap().action;
        assert!(ResizeDiagnostics::for_route(action, Some(1), false).is_none());
        driver.current.as_mut().unwrap().started = Instant::now() - Duration::from_secs(11);
        assert!(matches!(
            driver.tick(&observed, |_| false),
            Some(SmokeDecision::Quit)
        ));
        let terminal = ResizeDiagnostics::terminal_snapshot(&mut pending, &driver)
            .expect("timeout must produce the terminal diagnostic before Quit");
        assert_eq!(terminal.1, "terminal_failed");
        assert_eq!((terminal.0.width, terminal.0.height), (800, 600));
        assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
        let report = driver.report.report();
        assert_eq!(
            report.failure.as_ref().map(|failure| failure.code),
            Some("step_timeout")
        );
        assert_eq!(
            report.failure.as_ref().and_then(|failure| failure.step),
            Some(1)
        );
    }

    #[test]
    fn asynchronous_failure_flushes_first_resize_terminal_snapshot_once() {
        let (mut driver, observed, mut pending) = resize_driver();
        assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
        driver.fail(&observed, "command_rejected");
        let (_, phase) = ResizeDiagnostics::terminal_snapshot(&mut pending, &driver)
            .expect("asynchronous failure must flush before Quit");
        assert_eq!(phase, "terminal_failed");
        assert!(ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).is_none());
        let report = driver.report.report();
        assert_eq!(report.state, SmokeScenarioState::Failed);
        assert_eq!(
            report.failure.as_ref().map(|failure| failure.code),
            Some("command_rejected")
        );
    }

    #[test]
    fn first_resize_success_is_reported_after_driver_advances() {
        let (mut driver, mut observed, mut pending) = resize_driver();
        observed.counters.window_resize = Some(SmokeWindowResizeEvidence {
            sequence: 1,
            requested_width: 800,
            requested_height: 600,
            realized_width: 800,
            realized_height: 600,
            expected_layout: ShellLayoutMode::Compact,
            layout: ShellLayoutMode::Compact,
        });
        assert!(matches!(
            driver.tick(&observed, |_| false),
            Some(SmokeDecision::Route(SmokeAction::CloseAll))
        ));
        assert_eq!(driver.current.as_ref().map(|step| step.index), Some(2));
        assert_eq!(
            ResizeDiagnostics::terminal_snapshot(&mut pending, &driver).map(|(_, phase)| phase),
            Some("terminal_passed")
        );
    }
}
