use std::cell::RefCell;

use gtk::prelude::*;
use relm4::gtk;
use rshell_core::{TerminalOverrides, TerminalSettingsV1};

use crate::{
    FontMetricEnvironment, FontMetricsService, MeasuredFontMetrics, MetricsChange,
    TerminalDrawStats,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalMetricFacts {
    pub(crate) effective_scale_bits: u64,
    pub(crate) effective_dpi_bits: u64,
    pub(crate) measured_cell_width_bits: u64,
    pub(crate) measured_cell_height_bits: u64,
    pub(crate) dpi_fallback_used: bool,
    pub(crate) terminal_glyph_clipped_cells: usize,
    pub(crate) terminal_min_line_separation_bits: u64,
}

thread_local! {
    static TERMINAL_METRICS: RefCell<Vec<(gtk::glib::WeakRef<gtk::Widget>, TerminalMetricFacts)>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) fn record_terminal_metrics(canvas: &gtk::DrawingArea, measured: &MeasuredFontMetrics) {
    let widget = canvas.clone().upcast::<gtk::Widget>();
    let mut facts = TerminalMetricFacts {
        effective_scale_bits: measured.environment.effective_scale.to_bits(),
        effective_dpi_bits: measured.environment.effective_dpi.to_bits(),
        measured_cell_width_bits: measured.metrics.cell_width.to_bits(),
        measured_cell_height_bits: measured.metrics.cell_height.to_bits(),
        dpi_fallback_used: measured.environment.dpi_fallback_used,
        terminal_glyph_clipped_cells: 0,
        terminal_min_line_separation_bits: measured.minimum_line_separation.to_bits(),
    };
    TERMINAL_METRICS.with(|entries| {
        let mut entries = entries.borrow_mut();
        entries.retain(|(weak, _)| weak.upgrade().is_some());
        if let Some((_, current)) = entries
            .iter_mut()
            .find(|(weak, _)| weak.upgrade().as_ref() == Some(&widget))
        {
            if current.effective_scale_bits == facts.effective_scale_bits
                && current.effective_dpi_bits == facts.effective_dpi_bits
                && current.measured_cell_width_bits == facts.measured_cell_width_bits
                && current.measured_cell_height_bits == facts.measured_cell_height_bits
            {
                facts.terminal_glyph_clipped_cells = current.terminal_glyph_clipped_cells;
                facts.terminal_min_line_separation_bits = current.terminal_min_line_separation_bits;
            }
            *current = facts;
        } else {
            entries.push((widget.downgrade(), facts));
        }
    });
}

pub(crate) fn record_terminal_render_quality(canvas: &gtk::DrawingArea, stats: &TerminalDrawStats) {
    if stats.text_runs == 0 {
        return;
    }
    let widget = canvas.clone().upcast::<gtk::Widget>();
    TERMINAL_METRICS.with(|entries| {
        if let Some((_, facts)) = entries
            .borrow_mut()
            .iter_mut()
            .find(|(weak, _)| weak.upgrade().as_ref() == Some(&widget))
        {
            facts.terminal_glyph_clipped_cells = facts
                .terminal_glyph_clipped_cells
                .saturating_add(stats.glyph_clipped_cells);
            if let Some(separation) = stats.minimum_line_separation {
                let current = f64::from_bits(facts.terminal_min_line_separation_bits);
                facts.terminal_min_line_separation_bits = current.min(separation).to_bits();
            }
        }
    });
}

pub(crate) fn metric_facts(widget: &gtk::Widget) -> Option<TerminalMetricFacts> {
    TERMINAL_METRICS.with(|entries| {
        entries
            .borrow()
            .iter()
            .find_map(|(weak, facts)| (weak.upgrade().as_ref() == Some(widget)).then_some(*facts))
    })
}

pub(crate) fn measured_root_metrics(root: &gtk::Widget) -> Option<TerminalMetricFacts> {
    let context = root.pango_context();
    let environment =
        FontMetricEnvironment::from_context(&context, f64::from(root.scale_factor())).ok()?;
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let measured = match FontMetricsService::default()
        .measure(&context, &profile, environment)
        .ok()?
    {
        MetricsChange::Unchanged(measured) | MetricsChange::Changed(measured) => measured,
    };
    Some(TerminalMetricFacts {
        effective_scale_bits: measured.environment.effective_scale.to_bits(),
        effective_dpi_bits: measured.environment.effective_dpi.to_bits(),
        measured_cell_width_bits: measured.metrics.cell_width.to_bits(),
        measured_cell_height_bits: measured.metrics.cell_height.to_bits(),
        dpi_fallback_used: measured.environment.dpi_fallback_used,
        terminal_glyph_clipped_cells: 0,
        terminal_min_line_separation_bits: measured.minimum_line_separation.to_bits(),
    })
}
