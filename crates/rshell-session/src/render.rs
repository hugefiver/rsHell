use std::sync::Arc;

use rshell_core::{
    CellAttributes, CellPosition, Color, CursorShape, RenderCell, RenderCursor, RenderFrame,
    RenderRow, SelectionRange, Viewport,
};
use wezterm_term::{Intensity, Line, Underline, color::ColorAttribute};

use crate::{text::selection_columns, wezterm_adapter::WezTermAdapter};

pub(crate) fn snapshot(
    adapter: &WezTermAdapter,
    viewport: Viewport,
    selection: Option<SelectionRange>,
) -> RenderFrame {
    let terminal = adapter.terminal();
    let screen = terminal.screen();
    let total_rows = screen.scrollback_rows();
    let end_stable = screen.phys_to_stable_row_index(total_rows) as i64;
    let start = adapter.viewport_bounds().clamp_top(viewport.top_stable_row);
    let end = start
        .saturating_add(i64::from(viewport.rows))
        .min(end_stable);
    let phys_start = screen
        .stable_row_to_phys(start as isize)
        .unwrap_or(total_rows);
    let phys_end = screen
        .stable_row_to_phys(end as isize)
        .unwrap_or(total_rows);
    let mut rows = Vec::with_capacity(phys_end.saturating_sub(phys_start));
    screen.with_phys_lines(phys_start..phys_end, |lines| {
        for (offset, line) in lines.iter().enumerate() {
            let stable_row = screen.phys_to_stable_row_index(phys_start + offset) as i64;
            rows.push(convert_row(
                line,
                stable_row,
                usize::from(adapter.size().cols),
                selection,
            ));
        }
    });

    let cursor = terminal.cursor_pos();
    let cursor_stable = screen.visible_row_to_stable_row(cursor.y) as i64;
    RenderFrame {
        generation: 0,
        size: adapter.size(),
        viewport_top: start,
        rows: Arc::from(rows),
        cursor: Some(RenderCursor {
            position: CellPosition {
                stable_row: cursor_stable,
                column: cursor.x.min(usize::from(u16::MAX)) as u16,
            },
            shape: convert_cursor_shape(cursor.shape),
            visible: cursor.visibility == Default::default(),
        }),
        title: terminal.get_title().into(),
        alternate_screen: terminal.is_alt_screen_active(),
        mouse_reporting: adapter.mouse_reporting_allowed() && terminal.is_mouse_grabbed(),
    }
}

fn convert_row(
    line: &Line,
    stable_row: i64,
    columns: usize,
    selection: Option<SelectionRange>,
) -> RenderRow {
    let selected = selection.and_then(|range| selection_columns(range, stable_row, columns));
    let mut cells = Vec::with_capacity(columns);
    let mut column = 0;
    for cell in line.visible_cells() {
        let cell_column = cell.cell_index().min(columns);
        while column < cell_column {
            cells.push(blank_cell(is_selected(selected.clone(), column, 1)));
            column += 1;
        }
        if cell_column >= columns {
            break;
        }
        let width = cell.width().max(1).min(columns - cell_column);
        let attributes = cell.attrs();
        cells.push(RenderCell {
            text: cell.str().into(),
            width: width.min(usize::from(u8::MAX)) as u8,
            foreground: convert_color(attributes.foreground()),
            background: convert_color(attributes.background()),
            attributes: CellAttributes {
                bold: attributes.intensity() == Intensity::Bold,
                italic: attributes.italic(),
                underline: attributes.underline() != Underline::None,
                strike: attributes.strikethrough(),
                reverse: attributes.reverse(),
            },
            selected: is_selected(selected.clone(), cell_column, width),
        });
        column = cell_column + width;
    }
    while column < columns {
        cells.push(blank_cell(is_selected(selected.clone(), column, 1)));
        column += 1;
    }
    RenderRow {
        stable_row,
        wrapped: line.last_cell_was_wrapped(),
        cells: Arc::from(cells),
    }
}

fn blank_cell(selected: bool) -> RenderCell {
    RenderCell {
        text: " ".into(),
        width: 1,
        foreground: Color::Default,
        background: Color::Default,
        attributes: CellAttributes::default(),
        selected,
    }
}

fn is_selected(selected: Option<std::ops::Range<usize>>, column: usize, width: usize) -> bool {
    selected
        .map(|range| range.start < column + width && column < range.end)
        .unwrap_or(false)
}

fn convert_color(color: ColorAttribute) -> Color {
    match color {
        ColorAttribute::Default => Color::Default,
        ColorAttribute::PaletteIndex(index) => Color::Ansi(index),
        ColorAttribute::TrueColorWithPaletteFallback(color, _)
        | ColorAttribute::TrueColorWithDefaultFallback(color) => Color::Rgb(
            (color.0 * 255.0).round() as u8,
            (color.1 * 255.0).round() as u8,
            (color.2 * 255.0).round() as u8,
        ),
    }
}

fn convert_cursor_shape(shape: impl std::fmt::Debug) -> CursorShape {
    match format!("{shape:?}").as_str() {
        "BlinkingUnderline" | "SteadyUnderline" => CursorShape::Underline,
        "BlinkingBar" | "SteadyBar" => CursorShape::Beam,
        _ => CursorShape::Block,
    }
}
