use std::{collections::BTreeSet, sync::Arc};

use gtk::cairo::{self, Format, ImageSurface};
use rshell_core::{RenderFrame, SearchMatch};

use crate::{
    TerminalDecorations, TerminalDrawStats, TerminalRenderer, TerminalViewError,
    terminal_frame::dirty_frame_rows,
};

pub struct TerminalRenderCache {
    surface: Option<ImageSurface>,
    width: i32,
    height: i32,
    scale_factor: i32,
    renderer: Option<TerminalRenderer>,
    frame: Option<Arc<RenderFrame>>,
    decorations: TerminalDecorations,
}

impl TerminalRenderCache {
    pub fn new() -> Self {
        Self {
            surface: None,
            width: 0,
            height: 0,
            scale_factor: 0,
            renderer: None,
            frame: None,
            decorations: TerminalDecorations::default(),
        }
    }

    pub fn update(
        &mut self,
        renderer: &TerminalRenderer,
        frame: Arc<RenderFrame>,
        decorations: &TerminalDecorations,
        width: i32,
        height: i32,
        scale_factor: i32,
    ) -> Result<TerminalDrawStats, TerminalViewError> {
        if width <= 0 || height <= 0 {
            return Err(TerminalViewError::InvalidAllocation);
        }
        if scale_factor <= 0 {
            return Err(TerminalViewError::InvalidScale);
        }
        let pixel_width = width
            .checked_mul(scale_factor)
            .ok_or(TerminalViewError::GeometryOverflow)?;
        let pixel_height = height
            .checked_mul(scale_factor)
            .ok_or(TerminalViewError::GeometryOverflow)?;
        let rebuild = self.surface.is_none()
            || self.width != width
            || self.height != height
            || self.scale_factor != scale_factor
            || self.renderer.as_ref() != Some(renderer);
        if self
            .frame
            .as_ref()
            .is_some_and(|current| frame.generation < current.generation)
        {
            return Ok(TerminalDrawStats::default());
        }
        let frame = self
            .frame
            .as_ref()
            .filter(|current| frame.generation == current.generation)
            .cloned()
            .unwrap_or(frame);

        let mut dirty = if rebuild {
            (0..frame.rows.len()).collect::<BTreeSet<_>>()
        } else if self
            .frame
            .as_ref()
            .is_some_and(|current| frame.generation > current.generation)
        {
            dirty_frame_rows(self.frame.as_deref(), &frame)
                .into_iter()
                .collect()
        } else {
            BTreeSet::new()
        };
        add_decoration_rows(
            self.frame.as_deref(),
            &self.decorations,
            &frame,
            decorations,
            &mut dirty,
        );

        if rebuild {
            let surface = ImageSurface::create(Format::ARgb32, pixel_width, pixel_height)
                .map_err(|_| TerminalViewError::DrawingFailed)?;
            surface.set_device_scale(f64::from(scale_factor), f64::from(scale_factor));
            self.surface = Some(surface);
        }
        let context = cairo::Context::new(
            self.surface
                .as_ref()
                .ok_or(TerminalViewError::DrawingFailed)?,
        )
        .map_err(|_| TerminalViewError::DrawingFailed)?;
        if rebuild {
            renderer.paint_background(&context)?;
        }
        let rows = dirty.into_iter().collect::<Vec<_>>();
        let stats = renderer.paint_rows(&context, &frame, decorations, &rows, width)?;
        self.surface.as_ref().expect("surface created").flush();
        self.width = width;
        self.height = height;
        self.scale_factor = scale_factor;
        self.renderer = Some(renderer.clone());
        self.frame = Some(frame);
        self.decorations = decorations.clone();
        Ok(stats)
    }

    pub fn invalidate_metrics(&mut self) {
        self.surface = None;
        self.renderer = None;
    }

    pub fn paint(&self, context: &cairo::Context) -> Result<(), TerminalViewError> {
        let surface = self
            .surface
            .as_ref()
            .ok_or(TerminalViewError::DrawingFailed)?;
        context
            .set_source_surface(surface, 0.0, 0.0)
            .map_err(|_| TerminalViewError::DrawingFailed)?;
        context
            .paint()
            .map_err(|_| TerminalViewError::DrawingFailed)
    }
}

impl Default for TerminalRenderCache {
    fn default() -> Self {
        Self::new()
    }
}

fn add_decoration_rows(
    previous_frame: Option<&RenderFrame>,
    previous: &TerminalDecorations,
    frame: &RenderFrame,
    decorations: &TerminalDecorations,
    dirty: &mut BTreeSet<usize>,
) {
    if previous.search_matches != decorations.search_matches {
        add_match_rows(previous_frame, &previous.search_matches, dirty);
        add_match_rows(Some(frame), &decorations.search_matches, dirty);
    } else if previous.current_match != decorations.current_match {
        add_current_row(previous_frame, previous, dirty);
        add_current_row(Some(frame), decorations, dirty);
    }
}

fn add_current_row(
    frame: Option<&RenderFrame>,
    decorations: &TerminalDecorations,
    dirty: &mut BTreeSet<usize>,
) {
    let Some(found) = decorations
        .current_match
        .and_then(|index| decorations.search_matches.get(index))
    else {
        return;
    };
    add_match_rows(frame, std::slice::from_ref(found), dirty);
}

fn add_match_rows(
    frame: Option<&RenderFrame>,
    matches: &[SearchMatch],
    dirty: &mut BTreeSet<usize>,
) {
    let Some(frame) = frame else { return };
    for (index, row) in frame.rows.iter().enumerate() {
        if matches.iter().any(|found| {
            row.stable_row >= found.start.stable_row && row.stable_row <= found.end.stable_row
        }) {
            dirty.insert(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontMetrics;
    use rshell_core::{TerminalOverrides, TerminalSettingsV1, TerminalSize};

    #[test]
    fn hidpi_cache_has_physical_backing_pixels_and_logical_device_scale() {
        let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
        let renderer = TerminalRenderer::new(&profile, FontMetrics::new(12.0, 24.0).unwrap());
        let frame = Arc::new(RenderFrame {
            generation: 1,
            size: TerminalSize {
                cols: 4,
                rows: 1,
                pixel_width: 72,
                pixel_height: 36,
                dpi: 192,
            },
            viewport_top: 0,
            rows: Arc::from([]),
            cursor: None,
            title: String::new(),
            display_modes: Default::default(),
            alternate_screen: false,
            mouse_reporting: false,
        });
        let mut cache = TerminalRenderCache::new();

        cache
            .update(&renderer, frame, &TerminalDecorations::default(), 36, 18, 2)
            .unwrap();

        let surface = cache.surface.as_ref().unwrap();
        assert_eq!((surface.width(), surface.height()), (72, 36));
        assert_eq!(surface.device_scale(), (2.0, 2.0));

        cache.invalidate_metrics();
        assert!(cache.surface.is_none());
        assert!(cache.renderer.is_none());
        assert!(cache.frame.is_some());
    }
}
