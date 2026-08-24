use std::{collections::BTreeSet, fmt, path::PathBuf, sync::Arc};

use crate::{
    connection::{CatalogMutation, ConnectionCatalog, ConnectionId, PaneId, SessionId},
    render::{
        ExitStatus, RenderFrame, SearchMatch, SearchQuery, SelectionRange, SessionFailure,
        SessionState, TerminalInput, TerminalMouseEvent, TerminalSize,
    },
    terminal::{AppSettings, TerminalProfile},
    workspace::{SplitAxis, WorkspaceState},
};

use super::{
    AppFailure, ImportCandidateId, ImportPreviewId, ImportPreviewView, ImportReportView,
    ImportSourceKind, InteractionId, InteractionRequest, InteractionResponse, SecretUpdate,
};

// Keep payloads direct so the stable UI protocol matches its public shape.
#[allow(clippy::large_enum_variant)]
pub enum UiCommand {
    ApplyCatalog {
        mutation: CatalogMutation,
        secret: SecretUpdate,
    },
    SearchConnections(String),
    NewLocalTab,
    StartLocal {
        pane: PaneId,
    },
    Connect {
        pane: PaneId,
        connection: ConnectionId,
    },
    Split {
        pane: PaneId,
        axis: SplitAxis,
    },
    ClosePane(PaneId),
    CloseTab(uuid::Uuid),
    RetryPane(PaneId),
    Session {
        session: SessionId,
        command: SessionUiCommand,
    },
    SaveTerminalProfile(TerminalProfile),
    SaveSettings(AppSettings),
    PreviewImport {
        source: ImportSourceKind,
        path: PathBuf,
    },
    CommitImport {
        preview: ImportPreviewId,
        selected: BTreeSet<ImportCandidateId>,
    },
    CancelImport {
        preview: ImportPreviewId,
    },
    Respond {
        session: SessionId,
        interaction: InteractionId,
        response: InteractionResponse,
    },
    Shutdown,
}

impl UiCommand {
    pub fn secret_update(&self) -> Option<&SecretUpdate> {
        match self {
            Self::ApplyCatalog { secret, .. } => Some(secret),
            _ => None,
        }
    }
}

impl fmt::Debug for UiCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApplyCatalog { mutation, secret } => formatter
                .debug_struct("ApplyCatalog")
                .field("mutation", mutation)
                .field("secret", secret)
                .finish(),
            Self::SearchConnections(query) => formatter
                .debug_tuple("SearchConnections")
                .field(query)
                .finish(),
            Self::NewLocalTab => formatter.write_str("NewLocalTab"),
            Self::StartLocal { pane } => formatter
                .debug_struct("StartLocal")
                .field("pane", pane)
                .finish(),
            Self::Connect { pane, connection } => formatter
                .debug_struct("Connect")
                .field("pane", pane)
                .field("connection", connection)
                .finish(),
            Self::Split { pane, axis } => formatter
                .debug_struct("Split")
                .field("pane", pane)
                .field("axis", axis)
                .finish(),
            Self::ClosePane(pane) => formatter.debug_tuple("ClosePane").field(pane).finish(),
            Self::CloseTab(tab) => formatter.debug_tuple("CloseTab").field(tab).finish(),
            Self::RetryPane(pane) => formatter.debug_tuple("RetryPane").field(pane).finish(),
            Self::Session { session, command } => formatter
                .debug_struct("Session")
                .field("session", session)
                .field("command", command)
                .finish(),
            Self::SaveTerminalProfile(profile) => formatter
                .debug_tuple("SaveTerminalProfile")
                .field(profile)
                .finish(),
            Self::SaveSettings(settings) => formatter
                .debug_tuple("SaveSettings")
                .field(settings)
                .finish(),
            Self::PreviewImport { source, path: _ } => formatter
                .debug_struct("PreviewImport")
                .field("source", source)
                .field("path", &"[REDACTED]")
                .finish(),
            Self::CommitImport { preview, selected } => formatter
                .debug_struct("CommitImport")
                .field("preview", preview)
                .field("selected", selected)
                .finish(),
            Self::CancelImport { preview } => formatter
                .debug_struct("CancelImport")
                .field("preview", preview)
                .finish(),
            Self::Respond {
                session,
                interaction,
                response,
            } => formatter
                .debug_struct("Respond")
                .field("session", session)
                .field("interaction", interaction)
                .field("response", response)
                .finish(),
            Self::Shutdown => formatter.write_str("Shutdown"),
        }
    }
}

pub enum SessionUiCommand {
    Input(TerminalInput),
    Mouse(TerminalMouseEvent),
    Paste(secrecy::SecretString),
    Resize(TerminalSize),
    Scroll(i32),
    Search(SearchQuery),
    Select(SelectionRange),
    CopySelection,
    Respond {
        interaction: InteractionId,
        response: InteractionResponse,
    },
    Reconnect,
    Shutdown,
}

impl SessionUiCommand {
    pub fn paste(text: String) -> Self {
        Self::Paste(secrecy::SecretString::from(text))
    }

    pub fn paste_matches(&self, expected: &str) -> bool {
        use secrecy::ExposeSecret;

        matches!(self, Self::Paste(value) if value.expose_secret() == expected)
    }

    pub fn paste_len(&self) -> Option<usize> {
        use secrecy::ExposeSecret;

        match self {
            Self::Paste(value) => Some(value.expose_secret().len()),
            _ => None,
        }
    }
}

impl fmt::Debug for SessionUiCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(input) => formatter.debug_tuple("Input").field(input).finish(),
            Self::Mouse(event) => formatter.debug_tuple("Mouse").field(event).finish(),
            Self::Paste(_) => formatter.write_str("Paste([REDACTED])"),
            Self::Resize(size) => formatter.debug_tuple("Resize").field(size).finish(),
            Self::Scroll(delta) => formatter.debug_tuple("Scroll").field(delta).finish(),
            Self::Search(query) => formatter.debug_tuple("Search").field(query).finish(),
            Self::Select(range) => formatter.debug_tuple("Select").field(range).finish(),
            Self::CopySelection => formatter.write_str("CopySelection"),
            Self::Respond {
                interaction,
                response,
            } => formatter
                .debug_struct("Respond")
                .field("interaction", interaction)
                .field("response", response)
                .finish(),
            Self::Reconnect => formatter.write_str("Reconnect"),
            Self::Shutdown => formatter.write_str("Shutdown"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum SessionUiEvent {
    State(SessionState),
    Frame(Arc<RenderFrame>),
    Search(Vec<SearchMatch>),
    Copy(String),
    InteractionRequired(InteractionRequest),
    Exited(ExitStatus),
    Failed(SessionFailure),
    Crashed(String),
}

impl fmt::Debug for SessionUiEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(state) => formatter.debug_tuple("State").field(state).finish(),
            Self::Frame(frame) => formatter.debug_tuple("Frame").field(frame).finish(),
            Self::Search(matches) => formatter.debug_tuple("Search").field(matches).finish(),
            Self::Copy(_) => formatter.write_str("Copy([REDACTED])"),
            Self::InteractionRequired(request) => formatter
                .debug_tuple("InteractionRequired")
                .field(request)
                .finish(),
            Self::Exited(status) => formatter.debug_tuple("Exited").field(status).finish(),
            Self::Failed(failure) => formatter.debug_tuple("Failed").field(failure).finish(),
            Self::Crashed(_) => formatter.write_str("Crashed([REDACTED])"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    CatalogChanged(ConnectionCatalog),
    SearchResults(Vec<ConnectionId>),
    WorkspaceChanged(WorkspaceState),
    Session {
        session: SessionId,
        event: SessionUiEvent,
    },
    SettingsChanged(AppSettings),
    TerminalProfilesChanged(Vec<TerminalProfile>),
    ImportPreview(ImportPreviewView),
    ImportCompleted(ImportReportView),
    ImportCancelled(ImportPreviewId),
    InteractionRequired {
        session: SessionId,
        request: InteractionRequest,
    },
    InteractionResponded {
        session: SessionId,
        interaction: InteractionId,
    },
    OperationFailed(AppFailure),
    ShutdownComplete,
}
