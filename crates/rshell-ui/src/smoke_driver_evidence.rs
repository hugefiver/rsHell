use rshell_core::{AuthenticationKind, ImportSourceKind, SessionId};

use crate::{ShellLayoutMode, SmokePngEvidence, SmokeVisualFacts, SmokeVisualState};

#[derive(Debug, Clone, Default)]
pub struct SmokeTerminalEvidence {
    pub resize: Option<SmokeResizeEvidence>,
    pub search: Option<SmokeSearchEvidence>,
    pub selection: Option<SmokeSelectionEvidence>,
    pub clipboard: Option<SmokeClipboardEvidence>,
    pub reconnect: Option<SmokeReconnectEvidence>,
    pub paste: Option<SmokePasteEvidence>,
    pub color: Option<SmokeColorEvidence>,
    pub interrupt: Option<SmokeInterruptEvidence>,
    pub tui_entered: bool,
    pub tui_exited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeInterruptEvidence {
    pub sequence: u64,
    pub command_count: u64,
    pub wire_byte: u8,
    pub exact_etx: bool,
    pub enhanced_encoder_bypassed: bool,
    pub surviving_tui: bool,
    pub notice_visible: bool,
    pub reset_generation: Option<u64>,
    pub modes_clean: bool,
    pub replacement_character_count: usize,
    pub old_tui_overlap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokeDpiEvidence {
    pub logical_width: i32,
    pub logical_height: i32,
    pub effective_scale: f64,
    pub effective_dpi: f64,
    pub cell_width: f64,
    pub cell_height: f64,
    pub icon_logical_size: u16,
    pub icon_texture_width: i32,
    pub icon_texture_height: i32,
    pub dpi_fallback_used: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SmokeAccessibilityEvidence {
    pub unnamed_icon_controls: usize,
    pub hidden_primary_actions: usize,
    pub zero_size_panes: usize,
    pub horizontal_clipping: bool,
    pub background_insensitive: bool,
    pub focus_contained: bool,
    pub focus_restored: bool,
    pub escape_cancelled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeWindowResizeEvidence {
    pub sequence: u64,
    pub requested_width: i32,
    pub requested_height: i32,
    pub realized_width: i32,
    pub realized_height: i32,
    pub expected_layout: ShellLayoutMode,
    pub layout: ShellLayoutMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmokeVisualCheckpointEvidence {
    pub checkpoint_id: String,
    pub state: SmokeVisualState,
    pub layout: ShellLayoutMode,
    pub facts: SmokeVisualFacts,
    pub png: SmokePngEvidence,
    pub dpi: SmokeDpiEvidence,
    pub accessibility: SmokeAccessibilityEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokePasteEvidence {
    pub sequence: u64,
    pub expected_bytes: usize,
    pub actual_bytes: usize,
    pub command_exact: bool,
    pub frame_effect: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeColorEvidence {
    pub sequence: u64,
    pub marker_bytes: usize,
    pub marker_cells: usize,
    pub non_default_foreground: bool,
    pub red_foreground: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeResizeEvidence {
    pub sequence: u64,
    pub input_width: i32,
    pub input_height: i32,
    pub input_scale_bits: u64,
    pub requested: SmokeFrameEvidence,
    pub observed: Option<SmokeFrameEvidence>,
    pub exact: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeSearchEvidence {
    pub sequence: u64,
    pub query_bytes: usize,
    pub case_sensitive: bool,
    pub regex: bool,
    pub match_count: usize,
    pub current: Option<SmokeCellRangeEvidence>,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeSelectionEvidence {
    pub sequence: u64,
    pub start_x_bits: u64,
    pub start_y_bits: u64,
    pub end_x_bits: u64,
    pub end_y_bits: u64,
    pub range: SmokeCellRangeEvidence,
    pub rectangular: bool,
    pub wide_midpoint: bool,
    pub frame_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeCellRangeEvidence {
    pub start_row: i64,
    pub start_col: u16,
    pub end_row: i64,
    pub end_col: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeClipboardEvidence {
    pub sequence: u64,
    pub expected_bytes: usize,
    pub actual_bytes: usize,
    pub actor_exact: bool,
    pub gtk_written: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeReconnectEvidence {
    pub sequence: u64,
    pub old_session: SessionId,
    pub new_session: Option<SessionId>,
    pub old_session_absent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SmokeImportEvidence {
    pub sequence: u64,
    pub completed: bool,
    pub commit_source: Option<ImportSourceKind>,
    pub expected_groups: usize,
    pub expected_connections: usize,
    pub imported_groups: usize,
    pub imported_connections: usize,
    pub exact_group: bool,
    pub exact_connection: bool,
    pub authentication: Option<AuthenticationKind>,
    pub authentication_matches: bool,
    pub credential_reference_matches: bool,
    pub terminal_override_matches: bool,
    pub pending_preview_count: usize,
    pub cancel_pending_zero: bool,
    pub preview: Option<SmokeImportPreviewEvidence>,
    pub cancel_sequence: u64,
    pub cancelled_preview_matches: bool,
}

#[derive(Debug, Clone)]
pub struct SmokeImportPreviewEvidence {
    pub sequence: u64,
    pub source: ImportSourceKind,
    pub expected_groups: usize,
    pub expected_candidates: usize,
    pub actual_groups: usize,
    pub actual_candidates: usize,
    pub actual_group_name: Option<String>,
    pub actual_candidate_name: Option<String>,
    pub actual_host: Option<String>,
    pub authentication: Option<AuthenticationKind>,
    pub credential_reference_present: Option<bool>,
    pub terminal_override_present: Option<bool>,
    pub importable: Option<bool>,
    pub wildcard: Option<bool>,
    pub exact_group: bool,
    pub exact_candidate: bool,
    pub authentication_matches: bool,
    pub credential_reference_matches: bool,
    pub terminal_override_matches: bool,
    pub importable_matches: bool,
    pub wildcard_matches: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmokeFrameEvidence {
    pub generation: u64,
    pub cols: u16,
    pub rows: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub dpi: u32,
}
