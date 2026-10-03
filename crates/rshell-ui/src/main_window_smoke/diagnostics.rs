use gtk::prelude::*;
use relm4::gtk;

use crate::{
    MainWindow, ShellLayoutMode, SmokeAction, SmokeStepState, smoke_driver_state::SmokeDriver,
};

#[path = "resize_trace.rs"]
pub(crate) mod resize_trace;
use resize_trace::{Event, ResizeTrace, Snapshot};

pub(crate) struct ResizeDiagnostics {
    width: i32,
    height: i32,
    expected: ShellLayoutMode,
    trace: ResizeTrace,
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
            trace: ResizeTrace::new((*width, *height), *expected_mode),
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
    pub(super) fn refresh_smoke_window_allocation_traced(&mut self) {
        self.observe_smoke_resize_trace(Event::TickEnter);
        self.observe_smoke_resize_trace(Event::RefreshEnter);
        self.refresh_smoke_window_allocation();
        self.observe_smoke_resize_trace(Event::RefreshExit);
    }

    pub(super) fn finish_smoke_resize_tick(&mut self) {
        self.observe_smoke_resize_trace(Event::TickExit);
        self.snapshot_smoke_resize_terminal();
    }

    pub(crate) fn observe_smoke_resize_trace(&self, event: Event) {
        let Some(diagnostic) = &self.smoke_state.resize_diagnostics else {
            return;
        };
        let root = self
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok());
        let snapshot = Snapshot::read(root.as_ref(), Some(self.shell.layout().mode));
        diagnostic.trace.record(event, snapshot);
    }

    pub(super) fn route_smoke_action_with_resize_diagnostics(
        &mut self,
        action: SmokeAction,
    ) -> Result<bool, &'static str> {
        let index = self
            .smoke
            .as_ref()
            .and_then(|driver| driver.current.as_ref().map(|step| step.index));
        let first = !self.smoke_state.resize_trace_armed
            && self.smoke_state.resize_diagnostics.is_none()
            && ResizeDiagnostics::for_route(
                &action,
                index,
                cfg!(target_os = "linux")
                    && std::env::var("RSHELL_P0_RESIZE_DIAGNOSTICS").as_deref() == Ok("1"),
            )
            .map(|diagnostic| {
                #[cfg(target_os = "linux")]
                let mut diagnostic = diagnostic;
                self.smoke_state.resize_trace_armed = true;
                #[cfg(target_os = "linux")]
                if let Some(root) = self
                    .shell
                    .overlay
                    .root()
                    .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
                {
                    diagnostic.trace.attach(&root);
                }
                diagnostic.snapshot(self, "before");
                self.smoke_state.resize_diagnostics = Some(diagnostic);
                self.observe_smoke_resize_trace(Event::Before);
            })
            .is_some();
        let result = self.route_smoke_action(action);
        if first && let Some(diagnostic) = &self.smoke_state.resize_diagnostics {
            diagnostic.snapshot(self, "after_route");
            self.observe_smoke_resize_trace(Event::AfterRoute);
        }
        if result.is_err()
            && let Some(mut diagnostic) = self.smoke_state.resize_diagnostics.take()
        {
            diagnostic.snapshot(self, "terminal_route_error");
            diagnostic
                .trace
                .terminal("terminal_route_error", self.smoke_resize_trace_snapshot());
        }
        result
    }

    pub(super) fn snapshot_smoke_resize_terminal(&mut self) {
        let Some(driver) = self.smoke.as_ref() else {
            return;
        };
        if let Some((mut diagnostic, phase)) =
            ResizeDiagnostics::terminal_snapshot(&mut self.smoke_state.resize_diagnostics, driver)
        {
            diagnostic.snapshot(self, phase);
            diagnostic
                .trace
                .terminal(phase, self.smoke_resize_trace_snapshot());
        }
    }

    fn smoke_resize_trace_snapshot(&self) -> Snapshot {
        let root = self
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok());
        Snapshot::read(root.as_ref(), Some(self.shell.layout().mode))
    }
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
