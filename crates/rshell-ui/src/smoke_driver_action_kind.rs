#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeActionKind {
    WaitWindowRealized,
    NewTab,
    OpenConnectionEditor,
    SetConnectionField,
    SubmitConnection,
    SelectConnection,
    Connect,
    RespondHostKey,
    RespondAuth,
    SendTerminalText,
    PasteTextFromEnv,
    ResizeTerminal,
    WaitFrameContains,
    SplitHorizontal,
    SplitVertical,
    SwitchTab,
    SearchTerminal,
    SelectRange,
    CopySelection,
    Reconnect,
    VisualCheckpoint,
    PreviewImport,
    CommitImport,
    CancelImport,
    CloseAll,
}

impl SmokeActionKind {
    pub const fn as_str(self) -> &'static str {
        SMOKE_ACTION_NAMES[self as usize]
    }
}

pub(crate) const SMOKE_ACTION_NAMES: [&str; 25] = [
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
];
