use gtk::{cairo, pango};
use rshell_core::RenderCell;

use crate::{
    terminal_font, terminal_input::TerminalViewError, terminal_palette::Rgb,
    terminal_renderer::TerminalDrawStats,
};

#[derive(Clone, Copy)]
pub(crate) struct CellRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct TextPaintEvidence {
    pub(crate) ink_clipped: bool,
    pub(crate) line_separation: f64,
}

pub(crate) fn paint_text(
    context: &cairo::Context,
    cell: &RenderCell,
    rect: CellRect,
    foreground: Rgb,
    base_font: &pango::FontDescription,
) -> Result<TextPaintEvidence, TerminalViewError> {
    let layout = pangocairo::functions::create_layout(context);
    let mut font = terminal_font::for_text(base_font, &cell.text);
    if cell.attributes.bold {
        font.set_weight(pango::Weight::Bold);
    }
    if cell.attributes.italic {
        font.set_style(pango::Style::Italic);
    }
    layout.set_font_description(Some(&font));
    layout.set_text(&cell.text);
    let extents = text_extents(&layout);
    let fit = fit_ratio(extents, rect);
    let render_scale = if fit < 1.0 { fit * 0.98 } else { 1.0 };
    let extents = extents.scaled(render_scale);
    let TextExtents {
        ink_x,
        ink_y,
        ink_width,
        ink_height,
        logical_width,
        logical_height,
    } = extents;
    let preferred_x = rect.x + (rect.width - logical_width) / 2.0;
    let preferred_y = rect.y + (rect.height - logical_height) / 2.0;
    let origin_x = fitted_origin(
        preferred_x,
        rect.x - ink_x,
        rect.x + rect.width - (ink_x + ink_width),
    );
    let origin_y = fitted_origin(
        preferred_y,
        rect.y - ink_y,
        rect.y + rect.height - (ink_y + ink_height),
    );
    let ink_left = origin_x + ink_x;
    let ink_top = origin_y + ink_y;
    let ink_right = ink_left + ink_width;
    let ink_bottom = ink_top + ink_height;
    let ink_clipped = ink_left < rect.x
        || ink_top < rect.y
        || ink_right > rect.x + rect.width
        || ink_bottom > rect.y + rect.height;
    context
        .save()
        .map_err(|_| TerminalViewError::DrawingFailed)?;
    context.rectangle(rect.x, rect.y, rect.width, rect.height);
    context.clip();
    source(context, foreground);
    context.translate(origin_x, origin_y);
    context.scale(render_scale, render_scale);
    context.move_to(0.0, 0.0);
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
    Ok(TextPaintEvidence {
        ink_clipped,
        line_separation: rect.height - ink_height,
    })
}

#[derive(Clone, Copy)]
struct TextExtents {
    ink_x: f64,
    ink_y: f64,
    ink_width: f64,
    ink_height: f64,
    logical_width: f64,
    logical_height: f64,
}

impl TextExtents {
    fn scaled(self, factor: f64) -> Self {
        Self {
            ink_x: self.ink_x * factor,
            ink_y: self.ink_y * factor,
            ink_width: self.ink_width * factor,
            ink_height: self.ink_height * factor,
            logical_width: self.logical_width * factor,
            logical_height: self.logical_height * factor,
        }
    }
}

fn text_extents(layout: &pango::Layout) -> TextExtents {
    let (ink, logical) = layout.extents();
    let scale = f64::from(pango::SCALE);
    TextExtents {
        ink_x: f64::from(ink.x()) / scale,
        ink_y: f64::from(ink.y()) / scale,
        ink_width: f64::from(ink.width()) / scale,
        ink_height: f64::from(ink.height()) / scale,
        logical_width: f64::from(logical.width()) / scale,
        logical_height: f64::from(logical.height()) / scale,
    }
}

fn fit_ratio(extents: TextExtents, rect: CellRect) -> f64 {
    let horizontal = if extents.ink_width > 0.0 {
        rect.width / extents.ink_width
    } else {
        1.0
    };
    let vertical = if extents.ink_height > 0.0 {
        rect.height / extents.ink_height
    } else {
        1.0
    };
    horizontal.min(vertical).min(1.0)
}

fn fitted_origin(preferred: f64, minimum: f64, maximum: f64) -> f64 {
    if minimum <= maximum {
        preferred.clamp(minimum, maximum)
    } else {
        preferred
    }
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
