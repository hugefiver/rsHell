use gtk::{cairo, pango};
use rshell_core::RenderCell;

use crate::{
    terminal_input::TerminalViewError, terminal_palette::Rgb, terminal_renderer::TerminalDrawStats,
};

#[derive(Clone, Copy)]
pub(crate) struct CellRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) fn paint_text(
    context: &cairo::Context,
    cell: &RenderCell,
    rect: CellRect,
    foreground: Rgb,
    base_font: &pango::FontDescription,
) -> Result<(), TerminalViewError> {
    let layout = pangocairo::functions::create_layout(context);
    let mut font = base_font.clone();
    if cell.attributes.bold {
        font.set_weight(pango::Weight::Bold);
    }
    if cell.attributes.italic {
        font.set_style(pango::Style::Italic);
    }
    layout.set_font_description(Some(&font));
    layout.set_text(&cell.text);
    let (_, text_height) = layout.pixel_size();
    context
        .save()
        .map_err(|_| TerminalViewError::DrawingFailed)?;
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    source(context, foreground);
    context.move_to(
        rect.x,
        rect.y + (rect.height - f64::from(text_height)).max(0.0) / 2.0,
    );
    pangocairo::functions::show_layout(context, &layout);
    context
        .restore()
        .map_err(|_| TerminalViewError::DrawingFailed)?;
    if cell.attributes.underline {
        line(
            context,
            rect.x,
            rect.y + rect.height - 2.0,
            rect.width,
            foreground,
        )?;
    }
    if cell.attributes.strike {
        line(
            context,
            rect.x,
            rect.y + rect.height / 2.0,
            rect.width,
            foreground,
        )?;
    }
    Ok(())
}

pub(crate) fn update_stats(cell: &RenderCell, stats: &mut TerminalDrawStats) {
    stats.text_runs += 1;
    stats.wide_cells += usize::from(cell.width > 1);
    stats.combining_cells += usize::from(has_combining_mark(&cell.text));
    stats.bold_cells += usize::from(cell.attributes.bold);
    stats.italic_cells += usize::from(cell.attributes.italic);
    stats.underlined_cells += usize::from(cell.attributes.underline);
    stats.struck_cells += usize::from(cell.attributes.strike);
}

pub(crate) fn source(context: &cairo::Context, color: Rgb) {
    context.set_source_rgb(color.0, color.1, color.2);
}

pub(crate) fn fill(
    context: &cairo::Context,
    rect: CellRect,
    color: Rgb,
    alpha: f64,
) -> Result<(), TerminalViewError> {
    context.set_source_rgba(color.0, color.1, color.2, alpha);
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    context.fill().map_err(|_| TerminalViewError::DrawingFailed)
}

fn line(
    context: &cairo::Context,
    x: f64,
    y: f64,
    width: f64,
    color: Rgb,
) -> Result<(), TerminalViewError> {
    source(context, color);
    context.set_line_width(1.0);
    context.move_to(x, y);
    context.line_to(x + width, y);
    context
        .stroke()
        .map_err(|_| TerminalViewError::DrawingFailed)
}

fn has_combining_mark(text: &str) -> bool {
    text.chars().skip(1).any(|value| {
        matches!(value, '\u{0300}'..='\u{036f}' | '\u{1ab0}'..='\u{1aff}' | '\u{1dc0}'..='\u{1dff}')
    })
}
