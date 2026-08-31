use std::collections::BTreeMap;

use rshell_ui::SmokeVisualCheckpointEvidence;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct P0VisualCheckpointEvidence {
    checkpoint_id: String,
    state: &'static str,
    layout: &'static str,
    facts: P0VisualFacts,
    png: P0PngEvidence,
    dpi: P0DpiEvidence,
    accessibility: P0AccessibilityEvidence,
}

#[derive(Serialize)]
struct P0VisualFacts {
    requested_width: i32,
    requested_height: i32,
    realized_width: i32,
    realized_height: i32,
    command_bar: bool,
    dense_sidebar: bool,
    tab_strip: bool,
    pane_command_row: bool,
    terminal_canvas: bool,
    content_dialog: bool,
    embedded_icon_count: usize,
    icon_logical_size: i32,
    icon_texture_width: i32,
    icon_texture_height: i32,
    icon_backend: Option<&'static str>,
    effective_scale_bits: u64,
    effective_dpi_bits: u64,
    measured_cell_width_bits: u64,
    measured_cell_height_bits: u64,
    dpi_fallback_used: bool,
    focus_or_selection_treatment: bool,
    terminal_glyph_clipped_cells: usize,
    terminal_min_line_separation: f64,
}

#[derive(Serialize)]
struct P0PngEvidence {
    width: i32,
    height: i32,
    non_empty: bool,
    luminance_buckets: usize,
    dark_regions_required: usize,
    dark_regions_passed: usize,
    focus_or_selection_thickness_px: usize,
}

#[derive(Serialize)]
struct P0DpiEvidence {
    logical_width: i32,
    logical_height: i32,
    effective_scale: f64,
    effective_dpi: f64,
    cell_width: f64,
    cell_height: f64,
    icon_logical_size: u16,
    icon_texture_width: i32,
    icon_texture_height: i32,
    dpi_fallback_used: bool,
}

#[derive(Serialize)]
struct P0AccessibilityEvidence {
    unnamed_icon_controls: usize,
    hidden_primary_actions: usize,
    zero_size_panes: usize,
    horizontal_clipping: bool,
    background_insensitive: bool,
    focus_contained: bool,
    focus_restored: bool,
    escape_cancelled: bool,
}

pub(crate) fn visual_evidence(
    values: &BTreeMap<String, SmokeVisualCheckpointEvidence>,
) -> BTreeMap<String, P0VisualCheckpointEvidence> {
    values
        .iter()
        .map(|(id, value)| (id.clone(), checkpoint_evidence(value)))
        .collect()
}

fn checkpoint_evidence(value: &SmokeVisualCheckpointEvidence) -> P0VisualCheckpointEvidence {
    let facts = value.facts;
    let png = value.png;
    let dpi = value.dpi;
    let accessibility = value.accessibility;
    P0VisualCheckpointEvidence {
        checkpoint_id: value.checkpoint_id.clone(),
        state: value.state.as_str(),
        layout: value.layout.as_str(),
        facts: P0VisualFacts {
            requested_width: facts.requested_width,
            requested_height: facts.requested_height,
            realized_width: facts.realized_width,
            realized_height: facts.realized_height,
            command_bar: facts.command_bar,
            dense_sidebar: facts.dense_sidebar,
            tab_strip: facts.tab_strip,
            pane_command_row: facts.pane_command_row,
            terminal_canvas: facts.terminal_canvas,
            content_dialog: facts.content_dialog,
            embedded_icon_count: facts.embedded_icon_count,
            icon_logical_size: facts.icon_logical_size,
            icon_texture_width: facts.icon_texture_width,
            icon_texture_height: facts.icon_texture_height,
            icon_backend: facts.icon_backend.map(rshell_ui::IconBackend::as_str),
            effective_scale_bits: facts.effective_scale_bits,
            effective_dpi_bits: facts.effective_dpi_bits,
            measured_cell_width_bits: facts.measured_cell_width_bits,
            measured_cell_height_bits: facts.measured_cell_height_bits,
            dpi_fallback_used: facts.dpi_fallback_used,
            focus_or_selection_treatment: facts.focus_or_selection_treatment,
            terminal_glyph_clipped_cells: facts.terminal_glyph_clipped_cells,
            terminal_min_line_separation: f64::from_bits(facts.terminal_min_line_separation_bits),
        },
        png: P0PngEvidence {
            width: png.width,
            height: png.height,
            non_empty: png.non_empty,
            luminance_buckets: png.luminance_buckets,
            dark_regions_required: png.dark_regions_required,
            dark_regions_passed: png.dark_regions_passed,
            focus_or_selection_thickness_px: png.focus_or_selection_thickness_px,
        },
        dpi: P0DpiEvidence {
            logical_width: dpi.logical_width,
            logical_height: dpi.logical_height,
            effective_scale: dpi.effective_scale,
            effective_dpi: dpi.effective_dpi,
            cell_width: dpi.cell_width,
            cell_height: dpi.cell_height,
            icon_logical_size: dpi.icon_logical_size,
            icon_texture_width: dpi.icon_texture_width,
            icon_texture_height: dpi.icon_texture_height,
            dpi_fallback_used: dpi.dpi_fallback_used,
        },
        accessibility: P0AccessibilityEvidence {
            unnamed_icon_controls: accessibility.unnamed_icon_controls,
            hidden_primary_actions: accessibility.hidden_primary_actions,
            zero_size_panes: accessibility.zero_size_panes,
            horizontal_clipping: accessibility.horizontal_clipping,
            background_insensitive: accessibility.background_insensitive,
            focus_contained: accessibility.focus_contained,
            focus_restored: accessibility.focus_restored,
            escape_cancelled: accessibility.escape_cancelled,
        },
    }
}
