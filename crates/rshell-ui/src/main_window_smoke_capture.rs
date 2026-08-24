use gtk::gdk::prelude::TextureExtManual;
use gtk::prelude::*;
use relm4::gtk;

use crate::{
    SmokeVisualEvidence, SmokeVisualFacts,
    visual_png::{NativeByteOrder, analyze_rgba, argb32_native_to_rgba},
};

pub(crate) fn capture_widget_png(
    paintable: &gtk::WidgetPaintable,
    path: &std::path::Path,
    facts: SmokeVisualFacts,
) -> Result<SmokeVisualEvidence, &'static str> {
    let widget = paintable.widget().ok_or("snapshot_widget_unavailable")?;
    let width = widget.width();
    let height = widget.height();
    if width <= 0 || height <= 0 {
        return Err("snapshot_allocation_unavailable");
    }
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot.to_node().ok_or("snapshot_node_unavailable")?;
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer
        .realize(None)
        .map_err(|_| "snapshot_renderer_unavailable")?;
    let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&viewport));
    renderer.unrealize();
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
    let png = analyze_rgba(&rgba, width, height)?;
    texture
        .save_to_png(path)
        .map_err(|_| "snapshot_write_failed")?;
    Ok(SmokeVisualEvidence {
        facts,
        png: Some(png),
    })
}
