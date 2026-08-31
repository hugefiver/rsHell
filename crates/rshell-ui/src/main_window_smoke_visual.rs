use gtk::prelude::*;
use relm4::gtk;

use crate::{
    MainWindow, SmokeBindingEvidence, SmokeVisualCheckpoint, SmokeVisualCheckpointEvidence,
    SmokeVisualState, collect_accessibility_evidence, collect_visual_facts, dpi_evidence,
    main_window_smoke_capture::capture_widget_png_with_accent,
    main_window_smoke_matrix::{focus_restored, press_escape},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum VisualCheckpointPhase {
    #[default]
    Idle,
    Opening,
    Observed,
    Closing,
    Complete,
}

impl MainWindow {
    pub(crate) fn route_visual_checkpoint(
        &mut self,
        checkpoint: SmokeVisualCheckpoint,
    ) -> Result<bool, &'static str> {
        if self
            .smoke_state
            .active_visual_checkpoint
            .as_ref()
            .is_none_or(|active| active.id != checkpoint.id)
        {
            self.smoke_state.active_visual_checkpoint = Some(checkpoint.clone());
            self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Idle;
            self.smoke_state.visual_capture_attempted = false;
            self.smoke_state.visual_stage_count = None;
        }
        match self.smoke_state.visual_checkpoint {
            VisualCheckpointPhase::Idle => {
                self.begin_smoke_checkpoint(&checkpoint);
                self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Opening;
                Ok(false)
            }
            VisualCheckpointPhase::Opening => self.capture_visual_checkpoint(&checkpoint),
            VisualCheckpointPhase::Observed => self.close_visual_checkpoint(&checkpoint),
            VisualCheckpointPhase::Closing => self.finish_visual_checkpoint(&checkpoint),
            VisualCheckpointPhase::Complete => Ok(true),
        }
    }

    fn capture_visual_checkpoint(
        &mut self,
        checkpoint: &SmokeVisualCheckpoint,
    ) -> Result<bool, &'static str> {
        if !self.advance_smoke_checkpoint(checkpoint.state)? {
            return Ok(false);
        }
        if self.smoke_state.visual_paintable.is_none() {
            self.prepare_smoke_paintable(checkpoint.state)?;
            return Ok(false);
        }
        let root = self.smoke_root()?;
        if root.width() <= 0
            || root.height() <= 0
            || self.shell.layout().mode != checkpoint.expected_mode
        {
            return Ok(false);
        }
        if !self
            .smoke_state
            .visual_paintable
            .as_ref()
            .is_some_and(|paintable| {
                let image = paintable.current_image();
                image.intrinsic_width() > 0 && image.intrinsic_height() > 0
            })
        {
            return Ok(false);
        }
        let paintable = self
            .smoke_state
            .visual_paintable
            .as_ref()
            .cloned()
            .ok_or("visual_paintable_unavailable")?;
        let capture_root = paintable.widget().ok_or("visual_paintable_unavailable")?;
        let facts = collect_visual_facts(&capture_root, (checkpoint.width, checkpoint.height));
        if facts.terminal_glyph_clipped_cells > 0 {
            return Err("visual_terminal_glyph_clipped");
        }
        if !facts.terminal_typography_passes() {
            return Err("visual_terminal_line_separation_incomplete");
        }
        if !facts.contract_passes() {
            return Err("visual_contract_incomplete");
        }
        let mut accessibility = collect_accessibility_evidence(&capture_root);
        if accessibility.zero_size_panes > 0 {
            capture_root.queue_resize();
            capture_root.queue_draw();
            return Ok(false);
        }
        if accessibility.unnamed_icon_controls > 0
            || accessibility.hidden_primary_actions > 0
            || accessibility.horizontal_clipping
        {
            return Err("visual_accessibility_incomplete");
        }
        let path = self.checkpoint_png_path(&checkpoint.id)?;
        if let Some(driver) = &self.smoke {
            driver.record_requested_png_path(path.clone());
        }
        let captured = match capture_widget_png_with_accent(
            &paintable,
            self.smoke_state.visual_accent_paintable.as_ref(),
            &path,
            facts,
        ) {
            Ok(captured) => captured,
            Err(
                _error
                @ ("visual_root_snapshot_unavailable" | "visual_accent_snapshot_unavailable"),
            ) => {
                let widget = paintable.widget().ok_or("visual_paintable_unavailable")?;
                widget.queue_resize();
                widget.queue_draw();
                if let Some(accent) = self
                    .smoke_state
                    .visual_accent_paintable
                    .as_ref()
                    .and_then(gtk::WidgetPaintable::widget)
                {
                    accent.queue_draw();
                }
                return Ok(false);
            }
            Err(error) => return Err(error),
        };
        self.smoke_state.visual_paintable = None;
        if let Some(accent) = self.smoke_state.visual_accent_paintable.take() {
            accent.set_widget(gtk::Widget::NONE);
        }
        paintable.set_widget(gtk::Widget::NONE);
        let png = captured.png.ok_or("visual_capture_failed")?;
        accessibility.focus_restored = self.smoke_state.modal_focus_restore_verified;
        accessibility.escape_cancelled = self.smoke_state.modal_escape_verified;
        self.smoke_state.visual = Some(captured);
        self.smoke_state.visuals.insert(
            checkpoint.id.clone(),
            SmokeVisualCheckpointEvidence {
                checkpoint_id: checkpoint.id.clone(),
                state: checkpoint.state,
                layout: checkpoint.expected_mode,
                facts,
                png,
                dpi: dpi_evidence(facts),
                accessibility,
            },
        );
        if let Some(driver) = &self.smoke {
            driver.record_png_path(path);
        }
        self.smoke_state.visual_capture_attempted = true;
        if matches!(
            checkpoint.state,
            SmokeVisualState::Editor | SmokeVisualState::Settings | SmokeVisualState::Import
        ) {
            self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Observed;
            Ok(false)
        } else {
            self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Complete;
            self.smoke_state.visual_completion_tick_pending = true;
            Ok(true)
        }
    }

    fn close_visual_checkpoint(
        &mut self,
        checkpoint: &SmokeVisualCheckpoint,
    ) -> Result<bool, &'static str> {
        if matches!(
            checkpoint.state,
            SmokeVisualState::Editor | SmokeVisualState::Settings | SmokeVisualState::Import
        ) {
            let surface = self
                .smoke_checkpoint_surface(checkpoint.state)
                .ok_or("visual_modal_unavailable")?;
            if !press_escape(&surface) {
                return Err("visual_escape_not_handled");
            }
            self.smoke_state.modal_escape_verified = true;
            self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Closing;
            Ok(false)
        } else {
            self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Complete;
            Ok(true)
        }
    }

    fn finish_visual_checkpoint(
        &mut self,
        checkpoint: &SmokeVisualCheckpoint,
    ) -> Result<bool, &'static str> {
        if self
            .smoke_checkpoint_surface(checkpoint.state)
            .is_some_and(|surface| surface.is_visible())
        {
            return Ok(false);
        }
        let root = self.smoke_root()?;
        self.smoke_state.modal_focus_restore_verified = focus_restored(
            root.upcast_ref(),
            self.smoke_state.visual_focus_trigger.as_ref(),
        );
        if !self.smoke_state.modal_focus_restore_verified {
            return Ok(false);
        }
        let Some(evidence) = self.smoke_state.visuals.get_mut(&checkpoint.id) else {
            return Err("visual_evidence_missing");
        };
        evidence.accessibility.focus_restored = self.smoke_state.modal_focus_restore_verified;
        evidence.accessibility.escape_cancelled = self.smoke_state.modal_escape_verified;
        if !evidence.contract_passes() {
            return Err("visual_checkpoint_incomplete");
        }
        self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Complete;
        self.smoke_state.visual_completion_tick_pending = true;
        Ok(true)
    }
}

pub(crate) fn visual_checkpoint_component_verified(
    phase: VisualCheckpointPhase,
    visual: Option<&SmokeVisualCheckpointEvidence>,
) -> bool {
    phase == VisualCheckpointPhase::Complete
        && visual.is_some_and(SmokeVisualCheckpointEvidence::contract_passes)
}

pub(crate) fn visual_checkpoint_binding(
    surface: Option<&str>,
    connection: Option<&str>,
    component_verified: bool,
) -> SmokeBindingEvidence {
    SmokeBindingEvidence {
        verified: component_verified && surface == Some("gtk") && connection.is_none(),
        component_verified,
        actual_label: Some("main_window".to_owned()),
        connection_id: None,
        profile_name: None,
        endpoint: None,
        pane_id: None,
        session_id: None,
        local: false,
    }
}
