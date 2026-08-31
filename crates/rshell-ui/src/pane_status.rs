use rshell_core::{PaneId, SessionId, SessionState, SessionUiCommand, SplitAxis, UiCommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanePageKind {
    Pending,
    Terminal,
    Status,
    Error,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneAction {
    ResetDisplay,
    SplitHorizontal,
    SplitVertical,
    Reconnect,
    Retry,
    EditConnection,
    CopyDiagnostics,
    Close,
}

impl PaneAction {
    pub(crate) const fn layout_priority(self) -> u8 {
        match self {
            Self::ResetDisplay => 4,
            Self::Reconnect | Self::Retry | Self::Close => 3,
            Self::SplitHorizontal | Self::SplitVertical => 2,
            Self::EditConnection | Self::CopyDiagnostics => 1,
        }
    }

    pub fn command(self, pane: PaneId, session: Option<SessionId>) -> Option<UiCommand> {
        match self {
            Self::ResetDisplay => session.map(|session| UiCommand::Session {
                session,
                command: SessionUiCommand::ResetDisplay,
            }),
            Self::SplitHorizontal => Some(UiCommand::Split {
                pane,
                axis: SplitAxis::Horizontal,
            }),
            Self::SplitVertical => Some(UiCommand::Split {
                pane,
                axis: SplitAxis::Vertical,
            }),
            Self::Reconnect | Self::Retry => Some(UiCommand::RetryPane(pane)),
            Self::Close => Some(UiCommand::ClosePane(pane)),
            Self::EditConnection | Self::CopyDiagnostics => None,
        }
    }
}

pub(crate) fn page(
    has_launch_target: bool,
    has_session: bool,
    state: SessionState,
) -> PanePageKind {
    if !has_launch_target {
        return PanePageKind::Unavailable;
    }
    if !has_session {
        return PanePageKind::Status;
    }
    match state {
        SessionState::Connected => PanePageKind::Terminal,
        SessionState::Exited => PanePageKind::Status,
        SessionState::Failed | SessionState::Crashed => PanePageKind::Error,
        SessionState::Created
        | SessionState::Connecting
        | SessionState::AwaitingHostKey
        | SessionState::AwaitingAuthentication
        | SessionState::Reconnecting
        | SessionState::Closing => PanePageKind::Pending,
    }
}

pub(crate) fn status_label(
    has_launch_target: bool,
    has_session: bool,
    state: SessionState,
) -> &'static str {
    if !has_launch_target {
        return "Session unavailable";
    }
    if !has_session {
        return "Disconnected";
    }
    match state {
        SessionState::Created => "Created",
        SessionState::Connecting => "Connecting",
        SessionState::AwaitingHostKey => "Awaiting host key",
        SessionState::AwaitingAuthentication => "Awaiting authentication",
        SessionState::Connected => "Connected",
        SessionState::Reconnecting => "Reconnecting",
        SessionState::Closing => "Closing",
        SessionState::Exited => "Exited",
        SessionState::Failed => "Failed",
        SessionState::Crashed => "Crashed",
    }
}

pub(crate) fn actions(
    page: PanePageKind,
    has_connection: bool,
    has_recovery_notice: bool,
) -> Vec<PaneAction> {
    match page {
        PanePageKind::Error | PanePageKind::Status => {
            let mut actions = vec![PaneAction::Retry];
            if has_connection {
                actions.push(PaneAction::EditConnection);
            }
            actions.push(PaneAction::CopyDiagnostics);
            if has_recovery_notice {
                actions.push(PaneAction::ResetDisplay);
            }
            actions.push(PaneAction::Close);
            actions
        }
        PanePageKind::Terminal => {
            let mut actions = Vec::with_capacity(5);
            if has_recovery_notice {
                actions.push(PaneAction::ResetDisplay);
            }
            actions.extend([
                PaneAction::SplitHorizontal,
                PaneAction::SplitVertical,
                PaneAction::Reconnect,
                PaneAction::Close,
            ]);
            actions
        }
        PanePageKind::Pending | PanePageKind::Unavailable => vec![PaneAction::Close],
    }
}
