use rshell_core::{
    CellPosition, KeyModifiers, MouseButton, MouseEventKind, RenderFrame, TerminalSize,
};

use crate::terminal_input::{FontMetrics, TerminalViewError, positive_finite};

pub(crate) fn terminal_font_metrics() -> FontMetrics {
    FontMetrics::new(9.0, 18.0).expect("static terminal metrics")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    pub kind: MouseEventKind,
    pub button: Option<MouseButton>,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub scroll_delta: i32,
    pub modifiers: KeyModifiers,
}

impl PointerEvent {
    pub fn press(x: f64, y: f64, scale: f64, button: MouseButton) -> Self {
        Self::new(MouseEventKind::Press, Some(button), x, y, scale, 0)
    }

    pub fn release(x: f64, y: f64, scale: f64, button: MouseButton) -> Self {
        Self::new(MouseEventKind::Release, Some(button), x, y, scale, 0)
    }

    pub fn movement(x: f64, y: f64, scale: f64, button: Option<MouseButton>) -> Self {
        Self::new(MouseEventKind::Move, button, x, y, scale, 0)
    }

    pub fn scroll(x: f64, y: f64, scale: f64, delta: i32) -> Self {
        Self::new(MouseEventKind::Scroll, None, x, y, scale, delta)
    }

    pub fn with_modifiers(mut self, modifiers: KeyModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    fn new(
        kind: MouseEventKind,
        button: Option<MouseButton>,
        x: f64,
        y: f64,
        scale: f64,
        scroll_delta: i32,
    ) -> Self {
        Self {
            kind,
            button,
            x,
            y,
            scale,
            scroll_delta,
            modifiers: KeyModifiers::default(),
        }
    }
}

pub(crate) fn terminal_size(
    width: i32,
    height: i32,
    scale: f64,
    metrics: FontMetrics,
) -> Result<TerminalSize, TerminalViewError> {
    if width <= 0 || height <= 0 {
        return Err(TerminalViewError::InvalidAllocation);
    }
    if !positive_finite(scale) {
        return Err(TerminalViewError::InvalidScale);
    }
    Ok(TerminalSize {
        cols: checked_dimension((f64::from(width) / metrics.cell_width).max(1.0), u16::MAX)? as u16,
        rows: checked_dimension((f64::from(height) / metrics.cell_height).max(1.0), u16::MAX)?
            as u16,
        pixel_width: checked_dimension(f64::from(width) * scale, u32::MAX)? as u32,
        pixel_height: checked_dimension(f64::from(height) * scale, u32::MAX)? as u32,
        dpi: checked_dimension(96.0 * scale, u32::MAX)? as u32,
    })
}

pub(crate) fn point_to_cell(
    frame: &RenderFrame,
    metrics: FontMetrics,
    x: f64,
    y: f64,
) -> Result<CellPosition, TerminalViewError> {
    Ok(point_to_view_cell(frame, metrics, x, y)?.0)
}

pub(crate) fn point_to_view_cell(
    frame: &RenderFrame,
    metrics: FontMetrics,
    x: f64,
    y: f64,
) -> Result<(CellPosition, u16), TerminalViewError> {
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || frame.rows.is_empty() {
        return Err(TerminalViewError::OutOfBounds);
    }
    let row_index = ((y / metrics.cell_height).floor() as usize).min(frame.rows.len() - 1);
    let row = &frame.rows[row_index];
    let max_column = frame.size.cols.saturating_sub(1);
    let column = ((x / metrics.cell_width).floor().min(f64::from(max_column))) as u16;
    let viewport_row = u16::try_from(row_index).map_err(|_| TerminalViewError::GeometryOverflow)?;
    Ok((
        CellPosition {
            stable_row: row.stable_row,
            column,
        },
        viewport_row,
    ))
}

pub(crate) fn logical_cell(frame: &RenderFrame, position: CellPosition) -> Option<(u16, u8)> {
    let row = frame
        .rows
        .iter()
        .find(|row| row.stable_row == position.stable_row)?;
    let mut column = 0u16;
    for cell in row.cells.iter() {
        let width = u16::from(cell.width.max(1));
        if position.column >= column && position.column < column.saturating_add(width) {
            return Some((column, cell.width.max(1)));
        }
        column = column.saturating_add(width);
    }
    None
}

pub(crate) fn checked_pixel(value: f64, scale: f64) -> Result<u32, TerminalViewError> {
    if value < 0.0 || !value.is_finite() || !positive_finite(scale) {
        return Err(TerminalViewError::OutOfBounds);
    }
    let value = (value * scale).round();
    if value > f64::from(u32::MAX) {
        return Err(TerminalViewError::GeometryOverflow);
    }
    Ok(value as u32)
}

fn checked_dimension(value: f64, max: impl Into<f64>) -> Result<u64, TerminalViewError> {
    let value = value.floor();
    if !value.is_finite() || value < 1.0 || value > max.into() {
        return Err(TerminalViewError::GeometryOverflow);
    }
    Ok(value as u64)
}
