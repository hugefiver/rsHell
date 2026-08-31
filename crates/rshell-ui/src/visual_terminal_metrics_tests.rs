use crate::{
    TerminalDrawStats,
    visual_terminal_metrics::{TerminalMetricFacts, replace_terminal_render_quality},
};

#[test]
fn latest_nonempty_paint_replaces_transient_quality_failures() {
    let mut facts = TerminalMetricFacts {
        effective_scale_bits: 1.0_f64.to_bits(),
        effective_dpi_bits: 96.0_f64.to_bits(),
        measured_cell_width_bits: 11.0_f64.to_bits(),
        measured_cell_height_bits: 21.0_f64.to_bits(),
        dpi_fallback_used: false,
        terminal_glyph_clipped_cells: 8,
        terminal_min_line_separation_bits: (-1.0_f64).to_bits(),
    };
    let clean = TerminalDrawStats {
        text_runs: 1,
        glyph_clipped_cells: 0,
        minimum_line_separation: Some(2.5),
        ..Default::default()
    };

    replace_terminal_render_quality(&mut facts, &clean);

    assert_eq!(facts.terminal_glyph_clipped_cells, 0);
    assert_eq!(f64::from_bits(facts.terminal_min_line_separation_bits), 2.5);
}
