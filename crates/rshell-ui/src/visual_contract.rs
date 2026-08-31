use gtk::prelude::*;
use relm4::gtk;

use crate::{
    IconBackend, ProductIcon, SmokeDpiEvidence,
    visual_accessibility::descendants_including,
    visual_terminal_metrics::{measured_root_metrics, metric_facts},
};

pub use crate::visual_accessibility::{collect_accessibility_evidence, visual_contrast_passes};
pub(crate) use crate::visual_terminal_metrics::{
    record_terminal_metrics, record_terminal_render_quality,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokeVisualFacts {
    pub requested_width: i32,
    pub requested_height: i32,
    pub realized_width: i32,
    pub realized_height: i32,
    pub command_bar: bool,
    pub dense_sidebar: bool,
    pub tab_strip: bool,
    pub pane_command_row: bool,
    pub terminal_canvas: bool,
    pub content_dialog: bool,
    pub embedded_icon_count: usize,
    pub icon_logical_size: i32,
    pub icon_texture_width: i32,
    pub icon_texture_height: i32,
    pub icon_backend: Option<IconBackend>,
    pub effective_scale_bits: u64,
    pub effective_dpi_bits: u64,
    pub measured_cell_width_bits: u64,
    pub measured_cell_height_bits: u64,
    pub dpi_fallback_used: bool,
    pub focus_or_selection_treatment: bool,
    pub terminal_glyph_clipped_cells: usize,
    pub terminal_min_line_separation_bits: u64,
}

impl SmokeVisualFacts {
    pub const fn terminal_typography_passes(self) -> bool {
        !self.terminal_canvas
            || (self.terminal_glyph_clipped_cells == 0
                && f64::from_bits(self.terminal_min_line_separation_bits).is_finite()
                && f64::from_bits(self.terminal_min_line_separation_bits)
                    >= crate::TERMINAL_LINE_SPACING)
    }

    pub const fn contract_passes(self) -> bool {
        crate::smoke_driver_visual_matrix::supported_dimensions(
            self.requested_width,
            self.requested_height,
        ) && self.realized_width > 0
            && self.realized_height > 0
            && self.command_bar
            && self.embedded_icon_count > 0
            && self.icon_logical_size > 0
            && self.icon_texture_width >= self.icon_logical_size
            && self.icon_texture_height >= self.icon_logical_size
            && self.icon_backend.is_some()
            && (!self.terminal_canvas
                || (f64::from_bits(self.effective_scale_bits).is_finite()
                    && f64::from_bits(self.effective_scale_bits) > 0.0
                    && f64::from_bits(self.effective_dpi_bits).is_finite()
                    && f64::from_bits(self.effective_dpi_bits) > 0.0
                    && f64::from_bits(self.measured_cell_width_bits) > 0.0
                    && f64::from_bits(self.measured_cell_height_bits) > 0.0))
            && self.terminal_typography_passes()
    }
}

pub fn dpi_evidence(facts: SmokeVisualFacts) -> SmokeDpiEvidence {
    SmokeDpiEvidence {
        logical_width: facts.realized_width,
        logical_height: facts.realized_height,
        effective_scale: f64::from_bits(facts.effective_scale_bits),
        effective_dpi: f64::from_bits(facts.effective_dpi_bits),
        cell_width: f64::from_bits(facts.measured_cell_width_bits),
        cell_height: f64::from_bits(facts.measured_cell_height_bits),
        icon_logical_size: u16::try_from(facts.icon_logical_size).unwrap_or_default(),
        icon_texture_width: facts.icon_texture_width,
        icon_texture_height: facts.icon_texture_height,
        dpi_fallback_used: facts.dpi_fallback_used,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokePngEvidence {
    pub width: i32,
    pub height: i32,
    pub non_empty: bool,
    pub luminance_buckets: usize,
    pub dark_regions_required: usize,
    pub dark_regions_passed: usize,
    pub focus_or_selection_thickness_px: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokeVisualEvidence {
    pub facts: SmokeVisualFacts,
    pub png: Option<SmokePngEvidence>,
}

pub fn collect_visual_facts(root: &gtk::Widget, requested: (i32, i32)) -> SmokeVisualFacts {
    let widgets = descendants_including(root);
    let visible = |widget: &&gtk::Widget| widget.is_mapped();
    let has_visible_class = |class: &str| {
        widgets
            .iter()
            .filter(visible)
            .any(|widget| widget.has_css_class(class))
    };
    let sidebar = widgets
        .iter()
        .filter(visible)
        .find(|widget| widget.has_css_class("sidebar"));
    let dense_sidebar = sidebar.is_some_and(|sidebar| {
        widgets.iter().any(|widget| {
            widget.downcast_ref::<gtk::Paned>().is_some_and(|paned| {
                paned.start_child().as_ref() == Some(sidebar)
                    && (200..=280).contains(&paned.position())
            })
        })
    });
    let tab_strip = has_visible_class("tab-bar") && has_visible_class("terminal-tab");
    let embedded_icon_count = widgets
        .iter()
        .filter(visible)
        .filter(|widget| widget.has_css_class("product-icon"))
        .count();
    let icon = widgets.iter().filter(visible).find_map(|widget| {
        let image = widget.downcast_ref::<gtk::Image>()?;
        widget.has_css_class("product-icon").then_some(image)
    });
    let texture = icon
        .and_then(|image| image.paintable())
        .and_then(|paintable| paintable.downcast::<gtk::gdk::Texture>().ok());
    let terminal_metrics = widgets
        .iter()
        .filter(visible)
        .find(|widget| widget.has_css_class("terminal-canvas"))
        .and_then(metric_facts)
        .or_else(|| measured_root_metrics(root));
    let focus_or_selection_treatment = widgets.iter().filter(visible).any(|widget| {
        ["active-tab", "active-pane", "navigation-selected"]
            .into_iter()
            .any(|class| widget.has_css_class(class))
    });
    SmokeVisualFacts {
        requested_width: requested.0,
        requested_height: requested.1,
        realized_width: root.width(),
        realized_height: root.height(),
        command_bar: has_visible_class("command-bar"),
        dense_sidebar,
        tab_strip,
        pane_command_row: has_visible_class("pane-command-row"),
        terminal_canvas: has_visible_class("terminal-canvas"),
        content_dialog: has_visible_class("content-dialog"),
        embedded_icon_count,
        icon_logical_size: icon.map_or(0, gtk::Image::pixel_size),
        icon_texture_width: texture.as_ref().map_or(0, gtk::gdk::Texture::width),
        icon_texture_height: texture.as_ref().map_or(0, gtk::gdk::Texture::height),
        icon_backend: texture.as_ref().map(|_| ProductIcon::backend()),
        effective_scale_bits: terminal_metrics.map_or(0, |facts| facts.effective_scale_bits),
        effective_dpi_bits: terminal_metrics.map_or(0, |facts| facts.effective_dpi_bits),
        measured_cell_width_bits: terminal_metrics
            .map_or(0, |facts| facts.measured_cell_width_bits),
        measured_cell_height_bits: terminal_metrics
            .map_or(0, |facts| facts.measured_cell_height_bits),
        dpi_fallback_used: terminal_metrics.is_some_and(|facts| facts.dpi_fallback_used),
        focus_or_selection_treatment,
        terminal_glyph_clipped_cells: terminal_metrics
            .map_or(usize::MAX, |facts| facts.terminal_glyph_clipped_cells),
        terminal_min_line_separation_bits: terminal_metrics
            .map_or(0, |facts| facts.terminal_min_line_separation_bits),
    }
}

pub fn selection_treatment_surface(root: &gtk::Widget) -> Option<gtk::Widget> {
    let active_tab = descendants_including(root)
        .into_iter()
        .find(|widget| widget.is_mapped() && widget.has_css_class("active-tab"))?;
    let mut current = active_tab.parent();
    while let Some(widget) = current {
        if widget.is_mapped() && widget.has_css_class("tab-bar") {
            return Some(widget);
        }
        current = widget.parent();
    }
    None
}
