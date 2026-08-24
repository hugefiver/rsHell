use rshell_ui::SmokeVisualEvidence;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct P0VisualEvidence {
    facts: P0VisualFacts,
    png: Option<P0PngEvidence>,
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
    focus_or_selection_treatment: bool,
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

pub(crate) fn visual_evidence(value: &SmokeVisualEvidence) -> P0VisualEvidence {
    let facts = value.facts;
    P0VisualEvidence {
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
            focus_or_selection_treatment: facts.focus_or_selection_treatment,
        },
        png: value.png.map(|png| P0PngEvidence {
            width: png.width,
            height: png.height,
            non_empty: png.non_empty,
            luminance_buckets: png.luminance_buckets,
            dark_regions_required: png.dark_regions_required,
            dark_regions_passed: png.dark_regions_passed,
            focus_or_selection_thickness_px: png.focus_or_selection_thickness_px,
        }),
    }
}
