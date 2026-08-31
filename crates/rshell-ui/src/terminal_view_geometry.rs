use rshell_core::{
    MouseButton, MouseEventKind, SelectionRange, SessionUiCommand, TerminalMouseEvent, UiCommand,
};

use crate::{
    FontMetrics, MeasuredFontMetrics, PointerEvent, TerminalGeometryInput, TerminalViewError,
    TerminalViewModel, ViewRect,
    terminal_geometry::{
        checked_pixel, logical_cell, point_to_cell, point_to_view_cell, terminal_size,
    },
};

impl TerminalViewModel {
    pub fn metrics(&self) -> FontMetrics {
        self.measured.metrics
    }

    pub fn measured_metrics(&self) -> &MeasuredFontMetrics {
        &self.measured
    }

    pub(crate) fn has_positive_emitted_geometry(&self) -> bool {
        self.last_emitted_size.is_some_and(|size| {
            size.cols > 0
                && size.rows > 0
                && size.pixel_width > 0
                && size.pixel_height > 0
                && size.dpi > 0
        })
    }

    pub(crate) fn replay_geometry(&self) -> Option<UiCommand> {
        self.last_emitted_size
            .map(|size| self.command(SessionUiCommand::Resize(size)))
    }

    pub fn cursor_rect(&self) -> Option<ViewRect> {
        let frame = self.frame.as_ref()?;
        let cursor = frame.cursor.filter(|cursor| cursor.visible)?;
        let row_index = frame
            .rows
            .iter()
            .position(|row| row.stable_row == cursor.position.stable_row)?;
        let (start, width) = logical_cell(frame, cursor.position)?;
        let metrics = self.metrics();
        Some(ViewRect {
            x: f64::from(start) * metrics.cell_width,
            y: row_index as f64 * metrics.cell_height,
            width: f64::from(width) * metrics.cell_width,
            height: metrics.cell_height,
        })
    }

    pub fn apply_geometry(
        &mut self,
        input: TerminalGeometryInput,
    ) -> Result<Option<UiCommand>, TerminalViewError> {
        if input.metrics != self.metrics() {
            return Err(TerminalViewError::InvalidFontMetrics);
        }
        let size = terminal_size(input)?;
        self.last_logical_allocation = Some((input.logical_width, input.logical_height));
        if self.last_emitted_size == Some(size) {
            return Ok(None);
        }
        self.last_emitted_size = Some(size);
        Ok(Some(self.command(SessionUiCommand::Resize(size))))
    }

    pub fn apply_metrics(
        &mut self,
        measured: MeasuredFontMetrics,
        allocation: Option<(i32, i32)>,
    ) -> Result<Option<UiCommand>, TerminalViewError> {
        FontMetrics::new(measured.metrics.cell_width, measured.metrics.cell_height)?;
        measured.environment.validate()?;
        let Some((logical_width, logical_height)) = allocation.or(self.last_logical_allocation)
        else {
            self.measured = measured;
            return Ok(None);
        };
        let input = TerminalGeometryInput {
            logical_width,
            logical_height,
            metrics: measured.metrics,
            environment: measured.environment,
        };
        let size = terminal_size(input)?;
        self.measured = measured;
        self.last_logical_allocation = Some((logical_width, logical_height));
        if self.last_emitted_size == Some(size) {
            return Ok(None);
        }
        self.last_emitted_size = Some(size);
        Ok(Some(self.command(SessionUiCommand::Resize(size))))
    }

    pub fn mouse(&self, event: PointerEvent) -> Result<Option<UiCommand>, TerminalViewError> {
        let frame = self.frame.as_ref().ok_or(TerminalViewError::OutOfBounds)?;
        let reports_mouse = self.profile.mouse_reporting && frame.mouse_reporting;
        if !reports_mouse && event.kind != MouseEventKind::Scroll {
            return Ok(None);
        }
        if !reports_mouse {
            return Ok((event.scroll_delta != 0)
                .then(|| self.command(SessionUiCommand::Scroll(event.scroll_delta))));
        }
        let (cell, viewport_row) = point_to_view_cell(frame, self.metrics(), event.x, event.y)?;
        if event.kind == MouseEventKind::Scroll && event.scroll_delta == 0 {
            return Ok(None);
        }
        let button = pointer_button(event);
        Ok(Some(self.command(SessionUiCommand::Mouse(
            TerminalMouseEvent {
                kind: event.kind,
                button,
                cell,
                viewport_row,
                pixel_x: checked_pixel(event.x, event.scale)?,
                pixel_y: checked_pixel(event.y, event.scale)?,
                modifiers: event.modifiers,
            },
        ))))
    }

    pub fn selection(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        rectangular: bool,
    ) -> Result<UiCommand, TerminalViewError> {
        let frame = self.frame.as_ref().ok_or(TerminalViewError::OutOfBounds)?;
        Ok(self.command(SessionUiCommand::Select(SelectionRange {
            start: point_to_cell(frame, self.metrics(), start_x, start_y)?,
            end: point_to_cell(frame, self.metrics(), end_x, end_y)?,
            rectangular,
        })))
    }
}

fn pointer_button(event: PointerEvent) -> Option<MouseButton> {
    if event.kind != MouseEventKind::Scroll {
        return event.button;
    }
    match event.scroll_delta.cmp(&0) {
        std::cmp::Ordering::Less => Some(MouseButton::WheelUp),
        std::cmp::Ordering::Greater => Some(MouseButton::WheelDown),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use gtk::pango;
    use rshell_core::{ColorScheme, SessionId};

    use super::*;
    use crate::{FontMetricEnvironment, FontMetricKey, MeasuredFontMetrics};

    #[test]
    fn measured_geometry_can_be_replayed_after_an_early_output_is_lost() {
        let environment = FontMetricEnvironment::new(1.0, 96.0).unwrap();
        let measured = MeasuredFontMetrics {
            metrics: FontMetrics::new(11.0, 20.0).unwrap(),
            key: FontMetricKey {
                family: "Monospace".into(),
                font_size_bits: 15.0_f32.to_bits(),
                effective_scale_bits: 1.0_f64.to_bits(),
                effective_dpi_bits: 96.0_f64.to_bits(),
                dpi_fallback_used: false,
                color_scheme: ColorScheme::default(),
            },
            environment,
            fallback_used: false,
            font_description: pango::FontDescription::from_string("Monospace 15"),
            minimum_line_separation: 2.0,
        };
        let mut model = TerminalViewModel::new(SessionId::new(), measured);
        let input = TerminalGeometryInput {
            logical_width: 110,
            logical_height: 80,
            metrics: model.metrics(),
            environment,
        };

        let initial = model.apply_geometry(input).unwrap().unwrap();
        let (expected_session, expected_size) = match initial {
            UiCommand::Session {
                session,
                command: SessionUiCommand::Resize(size),
            } => (session, size),
            _ => panic!("initial geometry command must be a session resize"),
        };
        assert!(model.apply_geometry(input).unwrap().is_none());
        assert!(matches!(
            model.replay_geometry(),
            Some(UiCommand::Session {
                session,
                command: SessionUiCommand::Resize(size),
            }) if session == expected_session && size == expected_size
        ));
    }
}
