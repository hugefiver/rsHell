#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeActionKind {
    WaitWindowRealized = 0,
    NewTab = 1,
    OpenConnectionEditor = 2,
    SetConnectionField = 3,
    SubmitConnection = 4,
    SelectConnection = 5,
    Connect = 6,
    RespondHostKey = 7,
    RespondAuth = 8,
    SendTerminalText = 9,
    PasteTextFromEnv = 10,
    ResizeTerminal = 11,
    WaitFrameContains = 12,
    SplitHorizontal = 13,
    SplitVertical = 14,
    SwitchTab = 15,
    SearchTerminal = 16,
    SelectRange = 17,
    CopySelection = 18,
    Reconnect = 19,
    VisualCheckpoint = 20,
    PreviewImport = 21,
    CommitImport = 22,
    CancelImport = 23,
    CloseAll = 24,
    InterruptTerminal = 25,
    ResetDisplay = 26,
    ResizeWindow = 27,
}

impl SmokeActionKind {
    pub const fn as_str(self) -> &'static str {
        SMOKE_ACTION_NAMES[self as usize]
    }
}

pub(crate) const SMOKE_ACTION_NAMES: [&str; 28] = [
    "wait_window_realized",
    "new_tab",
    "open_connection_editor",
    "set_connection_field",
    "submit_connection",
    "select_connection",
    "connect",
    "respond_host_key",
    "respond_auth",
    "send_terminal_text",
    "paste_text_from_env",
    "resize_terminal",
    "wait_frame_contains",
    "split_horizontal",
    "split_vertical",
    "switch_tab",
    "search_terminal",
    "select_range",
    "copy_selection",
    "reconnect",
    "visual_checkpoint",
    "preview_import",
    "commit_import",
    "cancel_import",
    "close_all",
    "interrupt_terminal",
    "reset_display",
    "resize_window",
];
