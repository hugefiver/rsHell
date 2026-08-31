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
