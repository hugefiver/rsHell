use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct TerminalEvidence {
    resize: Option<ResizeEvidence>,
    search: Option<SearchEvidence>,
    selection: Option<SelectionEvidence>,
    clipboard: Option<ClipboardEvidence>,
    reconnect: Option<ReconnectEvidence>,
    paste: Option<PasteEvidence>,
    color: Option<ColorEvidence>,
    tui_entered: bool,
    tui_exited: bool,
}

#[derive(Serialize)]
struct PasteEvidence {
    sequence: u64,
    expected_bytes: usize,
    actual_bytes: usize,
    command_exact: bool,
    frame_effect: bool,
}

#[derive(Serialize)]
struct ColorEvidence {
    sequence: u64,
    marker_bytes: usize,
    marker_cells: usize,
    non_default_foreground: bool,
    red_foreground: bool,
}

#[derive(Serialize)]
struct ResizeEvidence {
    sequence: u64,
    input_width: i32,
    input_height: i32,
    input_scale_bits: u64,
    requested: FrameEvidence,
    observed: Option<FrameEvidence>,
    exact: bool,
}

#[derive(Serialize)]
struct SearchEvidence {
    sequence: u64,
    query_bytes: usize,
    case_sensitive: bool,
    regex: bool,
    match_count: usize,
    current: Option<CellRangeEvidence>,
    completed: bool,
}

#[derive(Serialize)]
struct SelectionEvidence {
    sequence: u64,
    start_x_bits: u64,
    start_y_bits: u64,
    end_x_bits: u64,
    end_y_bits: u64,
    range: CellRangeEvidence,
    rectangular: bool,
    wide_midpoint: bool,
    frame_confirmed: bool,
}

#[derive(Serialize)]
struct CellRangeEvidence {
    start_row: i64,
    start_col: u16,
    end_row: i64,
    end_col: u16,
}

#[derive(Serialize)]
struct ClipboardEvidence {
    sequence: u64,
    expected_bytes: usize,
    actual_bytes: usize,
    actor_exact: bool,
    gtk_written: bool,
}

#[derive(Serialize)]
struct ReconnectEvidence {
    sequence: u64,
    old_session: String,
    new_session: Option<String>,
    old_session_absent: bool,
}

#[derive(Serialize)]
pub(crate) struct FrameEvidence {
    generation: u64,
    cols: u16,
    rows: u16,
    pixel_width: u32,
    pixel_height: u32,
    dpi: u32,
}

pub(crate) fn terminal_evidence(value: &rshell_ui::SmokeTerminalEvidence) -> TerminalEvidence {
    TerminalEvidence {
        resize: value.resize.map(|evidence| ResizeEvidence {
            sequence: evidence.sequence,
            input_width: evidence.input_width,
            input_height: evidence.input_height,
            input_scale_bits: evidence.input_scale_bits,
            requested: frame_evidence(evidence.requested),
            observed: evidence.observed.map(frame_evidence),
            exact: evidence.exact,
        }),
        search: value.search.map(|evidence| SearchEvidence {
            sequence: evidence.sequence,
            query_bytes: evidence.query_bytes,
            case_sensitive: evidence.case_sensitive,
            regex: evidence.regex,
            match_count: evidence.match_count,
            current: evidence.current.map(cell_range_evidence),
            completed: evidence.completed,
        }),
        selection: value.selection.map(|evidence| SelectionEvidence {
            sequence: evidence.sequence,
            start_x_bits: evidence.start_x_bits,
            start_y_bits: evidence.start_y_bits,
            end_x_bits: evidence.end_x_bits,
            end_y_bits: evidence.end_y_bits,
            range: cell_range_evidence(evidence.range),
            rectangular: evidence.rectangular,
            wide_midpoint: evidence.wide_midpoint,
            frame_confirmed: evidence.frame_confirmed,
        }),
        clipboard: value.clipboard.map(|evidence| ClipboardEvidence {
            sequence: evidence.sequence,
            expected_bytes: evidence.expected_bytes,
            actual_bytes: evidence.actual_bytes,
            actor_exact: evidence.actor_exact,
            gtk_written: evidence.gtk_written,
        }),
        reconnect: value.reconnect.map(|evidence| ReconnectEvidence {
            sequence: evidence.sequence,
            old_session: evidence.old_session.0.to_string(),
            new_session: evidence.new_session.map(|session| session.0.to_string()),
            old_session_absent: evidence.old_session_absent,
        }),
        paste: value.paste.map(|evidence| PasteEvidence {
            sequence: evidence.sequence,
            expected_bytes: evidence.expected_bytes,
            actual_bytes: evidence.actual_bytes,
            command_exact: evidence.command_exact,
            frame_effect: evidence.frame_effect,
        }),
        color: value.color.map(|evidence| ColorEvidence {
            sequence: evidence.sequence,
            marker_bytes: evidence.marker_bytes,
            marker_cells: evidence.marker_cells,
            non_default_foreground: evidence.non_default_foreground,
            red_foreground: evidence.red_foreground,
        }),
        tui_entered: value.tui_entered,
        tui_exited: value.tui_exited,
    }
}

pub(crate) fn frame_evidence(value: rshell_ui::SmokeFrameEvidence) -> FrameEvidence {
    FrameEvidence {
        generation: value.generation,
        cols: value.cols,
        rows: value.rows,
        pixel_width: value.pixel_width,
        pixel_height: value.pixel_height,
        dpi: value.dpi,
    }
}

fn cell_range_evidence(value: rshell_ui::SmokeCellRangeEvidence) -> CellRangeEvidence {
    CellRangeEvidence {
        start_row: value.start_row,
        start_col: value.start_col,
        end_row: value.end_row,
        end_col: value.end_col,
    }
}
