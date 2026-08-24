use rshell_core::{RenderFrame, SessionId, TerminalSize};

use crate::{
    MainWindow, SmokeFrameEvidence, SmokeResizeEvidence,
    terminal_geometry::{terminal_font_metrics, terminal_size},
};

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
    size: TerminalSize,
}

impl PreparedResize {
    pub(crate) fn matches(self, size: TerminalSize) -> bool {
        self.size == size
    }
}

pub(crate) fn prepared_smoke_resize(width: i32, height: i32, scale: f64) -> Option<PreparedResize> {
    terminal_size(width, height, scale, terminal_font_metrics())
        .ok()
        .map(|size| PreparedResize {
            width,
            height,
            scale_bits: scale.to_bits(),
            size,
        })
}

impl MainWindow {
    pub(crate) fn prepare_smoke_resize(&mut self, width: i32, height: i32, scale: f64) {
        self.smoke_state.resize_input = prepared_smoke_resize(width, height, scale);
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
            exact: false,
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
            evidence.exact = frame.size == pending.size;
        }
        if frame.size == pending.size {
            let sequence = self.next_smoke_evidence_sequence();
            if let Some(evidence) = &mut self.smoke_state.terminal.resize {
                evidence.sequence = sequence;
            }
            self.smoke_state.pending_resize = None;
        }
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
