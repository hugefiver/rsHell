use gtk::{cairo, pango};
use rshell_core::{CursorShape, RenderCell, RenderFrame, ResolvedTerminalProfile, SearchMatch};

use crate::{
    terminal_geometry::logical_cell,
    terminal_input::{FontMetrics, TerminalViewError},
    terminal_paint::{CellRect, fill, paint_text, source, update_stats},
    terminal_palette::TerminalPalette,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalDecorations {
    pub search_matches: Vec<SearchMatch>,
    pub current_match: Option<usize>,
}

impl TerminalDecorations {
    pub fn new(search_matches: Vec<SearchMatch>, current_match: Option<usize>) -> Self {
        Self {
            search_matches,
            current_match,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerminalDrawStats {
    pub rows: usize,
    pub text_runs: usize,
    pub wide_cells: usize,
    pub combining_cells: usize,
    pub selected_cells: usize,
    pub search_cells: usize,
    pub bold_cells: usize,
    pub italic_cells: usize,
    pub underlined_cells: usize,
    pub struck_cells: usize,
    pub reversed_cells: usize,
    pub cursor_shape: Option<CursorShape>,
    pub cursor_width: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalRenderer {
    metrics: FontMetrics,
    font: pango::FontDescription,
    palette: TerminalPalette,
}

#[derive(Clone, Copy)]
struct CellSite {
    stable_row: i64,
    column: u16,
    rect: CellRect,
}

impl TerminalRenderer {
    pub fn new(profile: &ResolvedTerminalProfile, metrics: FontMetrics) -> Self {
        let mut font = pango::FontDescription::new();
        font.set_family(&profile.font_family);
        font.set_size((f64::from(pango::SCALE) * f64::from(profile.font_size)).round() as i32);
        Self {
            metrics,
            font,
            palette: TerminalPalette::for_scheme(profile.color_scheme),
        }
    }

    pub fn draw(
        &self,
        context: &cairo::Context,
        frame: &RenderFrame,
        decorations: &TerminalDecorations,
        width: i32,
        height: i32,
    ) -> Result<TerminalDrawStats, TerminalViewError> {
        if width <= 0 || height <= 0 {
            return Err(TerminalViewError::InvalidAllocation);
        }
        self.paint_background(context)?;
        self.paint_rows(
            context,
            frame,
            decorations,
            &(0..frame.rows.len()).collect::<Vec<_>>(),
            width,
        )
    }

    pub(crate) fn paint_background(
        &self,
        context: &cairo::Context,
    ) -> Result<(), TerminalViewError> {
        source(context, self.palette.background);
        context
            .paint()
            .map_err(|_| TerminalViewError::DrawingFailed)
    }

    pub(crate) fn paint_rows(
        &self,
        context: &cairo::Context,
        frame: &RenderFrame,
        decorations: &TerminalDecorations,
        rows: &[usize],
        width: i32,
    ) -> Result<TerminalDrawStats, TerminalViewError> {
        let mut stats = TerminalDrawStats::default();
        for &row_index in rows {
            fill(
                context,
                CellRect {
                    x: 0.0,
                    y: row_index as f64 * self.metrics.cell_height,
                    width: f64::from(width),
                    height: self.metrics.cell_height,
                },
                self.palette.background,
                1.0,
            )?;
            stats.rows += 1;
            let Some(row) = frame.rows.get(row_index) else {
                continue;
            };
            let mut column = 0u16;
            for cell in row.cells.iter() {
                let cell_width = cell.width.max(1);
                let rect = CellRect {
                    x: f64::from(column) * self.metrics.cell_width,
                    y: row_index as f64 * self.metrics.cell_height,
                    width: f64::from(cell_width) * self.metrics.cell_width,
                    height: self.metrics.cell_height,
                };
                let site = CellSite {
                    stable_row: row.stable_row,
                    column,
                    rect,
                };
                self.paint_cell(context, cell, site, decorations, &mut stats)?;
                column = column.saturating_add(u16::from(cell_width));
            }
            self.paint_cursor_row(context, frame, row_index, &mut stats)?;
        }
        Ok(stats)
    }

    fn paint_cell(
        &self,
        context: &cairo::Context,
        cell: &RenderCell,
        site: CellSite,
        decorations: &TerminalDecorations,
        stats: &mut TerminalDrawStats,
    ) -> Result<(), TerminalViewError> {
        let (mut foreground, mut background) = (
            self.palette
                .resolve(cell.foreground, self.palette.foreground),
            self.palette
                .resolve(cell.background, self.palette.background),
        );
        if cell.attributes.reverse {
            std::mem::swap(&mut foreground, &mut background);
            stats.reversed_cells += 1;
        }
        fill(context, site.rect, background, 1.0)?;
        if let Some(current) = search_index(
            &decorations.search_matches,
            site.stable_row,
            site.column,
            cell.width.max(1),
        ) {
            let color = if Some(current) == decorations.current_match {
                self.palette.current_search
            } else {
                self.palette.search
            };
            fill(context, site.rect, color, 0.55)?;
            stats.search_cells += 1;
        }
        if cell.selected {
            fill(context, site.rect, self.palette.selection, 0.72)?;
            stats.selected_cells += 1;
        }
        update_stats(cell, stats);
        paint_text(context, cell, site.rect, foreground, &self.font)
    }

    fn paint_cursor_row(
        &self,
        context: &cairo::Context,
        frame: &RenderFrame,
        row_index: usize,
        stats: &mut TerminalDrawStats,
    ) -> Result<(), TerminalViewError> {
        let Some(cursor) = frame.cursor.filter(|cursor| cursor.visible) else {
            return Ok(());
        };
        if frame.rows.get(row_index).map(|row| row.stable_row) != Some(cursor.position.stable_row) {
            return Ok(());
        }
        let Some((column, cell_width)) = logical_cell(frame, cursor.position) else {
            return Ok(());
        };
        let rect = CellRect {
            x: f64::from(column) * self.metrics.cell_width,
            y: row_index as f64 * self.metrics.cell_height,
            width: f64::from(cell_width) * self.metrics.cell_width,
            height: self.metrics.cell_height,
        };
        match cursor.shape {
            CursorShape::Block => fill(context, rect, self.palette.cursor, 0.5)?,
            CursorShape::Beam => fill(
                context,
                CellRect { width: 2.0, ..rect },
                self.palette.cursor,
                1.0,
            )?,
            CursorShape::Underline => fill(
                context,
                CellRect {
                    y: rect.y + rect.height - 2.0,
                    height: 2.0,
                    ..rect
                },
                self.palette.cursor,
                1.0,
            )?,
        }
        stats.cursor_shape = Some(cursor.shape);
        stats.cursor_width = Some(rect.width);
        Ok(())
    }
}

fn search_index(matches: &[SearchMatch], row: i64, column: u16, width: u8) -> Option<usize> {
    let end = column.saturating_add(u16::from(width));
    matches.iter().position(|found| {
        if row < found.start.stable_row || row > found.end.stable_row {
            return false;
        }
        if found.start.stable_row == found.end.stable_row {
            return found.start.column < end && column < found.end.column;
        }
        (row != found.start.stable_row || found.start.column < end)
            && (row != found.end.stable_row || column < found.end.column)
    })
}
