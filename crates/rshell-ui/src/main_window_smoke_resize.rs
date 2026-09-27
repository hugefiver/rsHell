use rshell_core::{RenderFrame, SessionId, TerminalSize};

use crate::{
    MainWindow, ShellLayoutMode, SmokeFrameEvidence, SmokeResizeEvidence,
    SmokeWindowResizeEvidence, main_window_smoke_frame::checkpoint_trace,
    smoke_driver_evidence::window_resize_pending,
};
use gtk::prelude::*;

pub(crate) struct PendingResize {
    session: SessionId,
    generation: u64,
    size: TerminalSize,
}

#[derive(Clone, Copy)]
pub(crate) struct PreparedResize {
    width: i32,
    height: i32,
    scale_bits: u64,
    pixel_width: u32,
    pixel_height: u32,
}

impl PreparedResize {
    pub(crate) fn matches(self, size: TerminalSize) -> bool {
        size.pixel_width == self.pixel_width
            && size.pixel_height == self.pixel_height
            && size.dpi > 0
    }
}

pub(crate) fn prepared_smoke_resize(width: i32, height: i32, scale: f64) -> Option<PreparedResize> {
    if width <= 0 || height <= 0 || !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let pixel_width = (f64::from(width) * scale).floor();
    let pixel_height = (f64::from(height) * scale).floor();
    if pixel_width < 1.0
        || pixel_height < 1.0
        || pixel_width > f64::from(u32::MAX)
        || pixel_height > f64::from(u32::MAX)
    {
        return None;
    }
    Some(PreparedResize {
        width,
        height,
        scale_bits: scale.to_bits(),
        pixel_width: pixel_width as u32,
        pixel_height: pixel_height as u32,
    })
}

impl MainWindow {
    pub(crate) fn route_smoke_window_resize(
        &mut self,
        width: i32,
        height: i32,
        expected_mode: ShellLayoutMode,
    ) -> Result<(), &'static str> {
        let trace = self.smoke_state.trace_first_resize
            && self
                .smoke
                .as_ref()
                .and_then(|driver| driver.current.as_ref())
                .is_some_and(|step| step.index == 1);
        let window = self
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
            .ok_or("smoke_window_unavailable")?;
        let sequence = self.next_smoke_evidence_sequence();
        self.smoke_state.window_resize = Some(SmokeWindowResizeEvidence {
            sequence,
            requested_width: width,
            requested_height: height,
            realized_width: 0,
            realized_height: 0,
            expected_layout: expected_mode,
            layout: self.shell.layout().mode,
        });
        checkpoint_trace(trace, "default_size_enter", "");
        window.set_default_size(width, height);
        checkpoint_trace(trace, "default_size_return", "");
        checkpoint_trace(trace, "queue_resize_enter", "");
        window.queue_resize();
        checkpoint_trace(trace, "queue_resize_return", "");
        checkpoint_trace(trace, "present_enter", "");
        window.present();
        checkpoint_trace(trace, "present_return", "");
        checkpoint_trace(trace, "layout_enter", "");
        self.apply_shell_layout(width);
        checkpoint_trace(trace, "layout_return", "");
        if self.shell.layout().mode != expected_mode {
            return Err("smoke_window_mode_mismatch");
        }
        Ok(())
    }

    pub(crate) fn prepare_smoke_resize(&mut self, width: i32, height: i32, scale: f64) {
        self.smoke_state.resize_input = prepared_smoke_resize(width, height, scale);
    }

    pub(crate) fn observe_smoke_window_allocation(&mut self, _width: i32, _height: i32) {
        // Modal resize passes surface dimensions; sample the GTK widget for smoke evidence.
        let Some(window) = self
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
        else {
            return;
        };
        update_window_allocation(
            &mut self.smoke_state.window_resize,
            window.width(),
            window.height(),
            self.shell.layout().mode,
        );
    }

    pub(crate) fn refresh_smoke_window_allocation(&mut self) {
        if !window_resize_pending(self.smoke_state.window_resize) {
            return;
        }
        let Some(window) = self
            .shell
            .overlay
            .root()
            .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
        else {
            return;
        };
        let (width, height) = (window.width(), window.height());
        if width <= 0 || height <= 0 {
            return;
        }
        self.apply_shell_layout(width);
        update_window_allocation(
            &mut self.smoke_state.window_resize,
            width,
            height,
            self.shell.layout().mode,
        );
    }

    pub(crate) fn observe_smoke_resize_command(
        &mut self,
        session: SessionId,
        size: TerminalSize,
        generation: u64,
    ) {
        let Some(prepared) = self
            .smoke_state
            .resize_input
            .filter(|prepared| prepared.matches(size))
        else {
            return;
        };
        self.smoke_state.resize_input = None;
        self.smoke_state.pending_resize = Some(PendingResize {
            session,
            generation,
            size,
        });
        self.smoke_state.terminal.resize = Some(SmokeResizeEvidence {
            sequence: 0,
            input_width: prepared.width,
            input_height: prepared.height,
            input_scale_bits: prepared.scale_bits,
            requested: frame_evidence(generation, size),
            observed: None,
            exact: true,
        });
    }

    pub(crate) fn observe_smoke_resize_frame(&mut self, session: SessionId, frame: &RenderFrame) {
        let Some(pending) = self.smoke_state.pending_resize.as_ref() else {
            return;
        };
        if pending.session != session || frame.generation <= pending.generation {
            return;
        }
        let observed = frame_evidence(frame.generation, frame.size);
        if let Some(evidence) = &mut self.smoke_state.terminal.resize {
            evidence.observed = Some(observed);
            evidence.exact &= evidence.requested.pixel_width == pending.size.pixel_width
                && evidence.requested.pixel_height == pending.size.pixel_height;
        }
        let sequence = self.next_smoke_evidence_sequence();
        if let Some(evidence) = &mut self.smoke_state.terminal.resize {
            evidence.sequence = sequence;
        }
        self.smoke_state.pending_resize = None;
    }
}

pub(crate) fn window_surface_size(window: &gtk::ApplicationWindow) -> Option<(i32, i32)> {
    let surface = window.surface()?;
    let width = surface.width();
    let height = surface.height();
    (width > 0 && height > 0).then_some((width, height))
}

fn update_window_allocation(
    evidence: &mut Option<SmokeWindowResizeEvidence>,
    width: i32,
    height: i32,
    layout: ShellLayoutMode,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    if let Some(evidence) = evidence.as_mut() {
        evidence.realized_width = width;
        evidence.realized_height = height;
        evidence.layout = layout;
    }
}

fn frame_evidence(generation: u64, size: TerminalSize) -> SmokeFrameEvidence {
    SmokeFrameEvidence {
        generation,
        cols: size.cols,
        rows: size.rows,
        pixel_width: size.pixel_width,
        pixel_height: size.pixel_height,
        dpi: size.dpi,
    }
}

#[cfg(test)]
#[path = "main_window_smoke_resize_tests.rs"]
mod tests;
