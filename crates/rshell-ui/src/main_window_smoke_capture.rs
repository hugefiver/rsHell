use gtk::gdk::prelude::TextureExtManual;
use gtk::prelude::*;
use relm4::gtk;

use crate::{
    SmokeVisualEvidence, SmokeVisualFacts, selection_treatment_surface,
    visual_png::{NativeByteOrder, analyze_rgba_with_accent, argb32_native_to_rgba},
};

pub(crate) fn capture_widget_png(
    paintable: &gtk::WidgetPaintable,
    path: &std::path::Path,
    facts: SmokeVisualFacts,
) -> Result<SmokeVisualEvidence, &'static str> {
    capture_widget_png_with_accent(paintable, None, path, facts)
}

pub(crate) fn capture_widget_png_with_accent(
    paintable: &gtk::WidgetPaintable,
    accent_paintable: Option<&gtk::WidgetPaintable>,
    path: &std::path::Path,
    facts: SmokeVisualFacts,
) -> Result<SmokeVisualEvidence, &'static str> {
    let widget = paintable.widget().ok_or("snapshot_widget_unavailable")?;
    let texture = render_paintable(paintable).map_err(|error| match error {
        "snapshot_node_unavailable" => "visual_root_snapshot_unavailable",
        other => other,
    })?;
    let (rgba, width, height) = download_rgba(&texture)?;
    let png = if let Some(accent_paintable) = accent_paintable {
        let (accent_rgba, accent_width, accent_height) = render_paintable(accent_paintable)
            .map_err(|error| match error {
                "snapshot_node_unavailable" => "visual_accent_snapshot_unavailable",
                other => other,
            })
            .and_then(|texture| download_rgba(&texture))?;
        analyze_rgba_with_accent(
            &rgba,
            width,
            height,
            &accent_rgba,
            accent_width,
            accent_height,
        )?
    } else if let Some(accent_widget) = selection_treatment_surface(&widget) {
        let accent_paintable = gtk::WidgetPaintable::new(Some(&accent_widget));
        match render_paintable(&accent_paintable).and_then(|texture| download_rgba(&texture)) {
            Ok((accent_rgba, accent_width, accent_height)) => analyze_rgba_with_accent(
                &rgba,
                width,
                height,
                &accent_rgba,
                accent_width,
                accent_height,
            )?,
            Err("snapshot_node_unavailable") => {
                crate::visual_png::analyze_rgba(&rgba, width, height)?
            }
            Err(error) => return Err(error),
        }
    } else {
        crate::visual_png::analyze_rgba(&rgba, width, height)?
    };
    texture
        .save_to_png(path)
        .map_err(|_| "snapshot_write_failed")?;
    widget.queue_draw();
    paintable.invalidate_contents();
    Ok(SmokeVisualEvidence {
        facts,
        png: Some(png),
    })
}

fn render_paintable(paintable: &gtk::WidgetPaintable) -> Result<gtk::gdk::Texture, &'static str> {
    let widget = paintable.widget().ok_or("snapshot_widget_unavailable")?;
    let width = widget.width();
    let height = widget.height();
    if width <= 0 || height <= 0 {
        return Err("snapshot_allocation_unavailable");
    }
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let Some(node) = snapshot.to_node() else {
        return paintable
            .current_image()
            .downcast::<gtk::gdk::Texture>()
            .map_err(|_| "snapshot_node_unavailable");
    };
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer
        .realize(None)
        .map_err(|_| "snapshot_renderer_unavailable")?;
    let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&viewport));
    renderer.unrealize();
    Ok(texture)
}

fn download_rgba(texture: &gtk::gdk::Texture) -> Result<(Vec<u8>, i32, i32), &'static str> {
    let width = texture.width();
    let height = texture.height();
    let stride = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or("snapshot_allocation_unavailable")?;
    let length = stride
        .checked_mul(usize::try_from(height).map_err(|_| "snapshot_allocation_unavailable")?)
        .ok_or("snapshot_allocation_unavailable")?;
    let mut argb32 = vec![0; length];
    texture.download(&mut argb32, stride);
    let rgba = argb32_native_to_rgba(&argb32, NativeByteOrder::current())?;
    Ok((rgba, width, height))
}
