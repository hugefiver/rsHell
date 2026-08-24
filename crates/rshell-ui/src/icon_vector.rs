use gtk::cairo;
use gtk::prelude::*;
use relm4::gtk;

use crate::icon_registry::{IconRenderError, ProductIcon};
use crate::icon_vector_data::{VectorOp, operations};

pub(crate) fn render(icon: ProductIcon) -> Result<gtk::gdk::Texture, IconRenderError> {
    let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 16, 16)
        .map_err(|_| IconRenderError::InternalVector)?;
    let context = cairo::Context::new(&surface).map_err(|_| IconRenderError::InternalVector)?;
    context.set_operator(cairo::Operator::Source);
    context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    context
        .paint()
        .map_err(|_| IconRenderError::InternalVector)?;
    context.set_operator(cairo::Operator::Over);
    context.set_source_rgba(0.831, 0.831, 0.831, 1.0);
    context.set_line_width(1.5);
    context.set_line_cap(cairo::LineCap::Round);
    context.set_line_join(cairo::LineJoin::Round);

    for operation in operations(icon) {
        match *operation {
            VectorOp::MoveTo(x, y) => context.move_to(x, y),
            VectorOp::LineTo(x, y) => context.line_to(x, y),
            VectorOp::Arc(x, y, radius, start, end) => context.arc(x, y, radius, start, end),
            VectorOp::Close => context.close_path(),
        }
    }
    context
        .stroke()
        .map_err(|_| IconRenderError::InternalVector)?;
    drop(context);
    surface.flush();

    let rgba = cairo_argb32_to_rgba(
        surface
            .data()
            .map_err(|_| IconRenderError::InternalVector)?
            .as_ref(),
    );
    let bytes = gtk::glib::Bytes::from_owned(rgba);
    Ok(gtk::gdk::MemoryTexture::new(16, 16, gtk::gdk::MemoryFormat::R8g8b8a8, &bytes, 64).upcast())
}

fn cairo_argb32_to_rgba(source: &[u8]) -> Vec<u8> {
    source
        .chunks_exact(4)
        .flat_map(|pixel| {
            #[cfg(target_endian = "little")]
            let (red, green, blue, alpha) = (pixel[2], pixel[1], pixel[0], pixel[3]);
            #[cfg(target_endian = "big")]
            let (red, green, blue, alpha) = (pixel[1], pixel[2], pixel[3], pixel[0]);
            [
                unpremultiply(red, alpha),
                unpremultiply(green, alpha),
                unpremultiply(blue, alpha),
                alpha,
            ]
        })
        .collect()
}

fn unpremultiply(channel: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        0
    } else {
        ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
    }
}
