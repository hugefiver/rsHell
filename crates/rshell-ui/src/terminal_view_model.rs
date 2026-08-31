use std::{fmt, sync::Arc};

use rshell_core::{
    PaneId, RenderFrame, ResolvedTerminalProfile, SearchMatch, SearchQuery, SelectionRange,
    SessionId, SessionUiCommand, SessionUiEvent, TerminalInput, TerminalSettingsV1, TerminalSize,
    UiCommand,
};
use rshell_platform::ClipboardPolicy;

use crate::{
    MeasuredFontMetrics,
    terminal_input::{PhysicalAltState, TerminalViewError},
    terminal_search::TerminalSearchState,
};

pub use crate::terminal_frame::FrameUpdate;

#[derive(PartialEq, Eq)]
pub enum TerminalClipboardAction {
    Write(String),
}

impl fmt::Debug for TerminalClipboardAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(_) => formatter.write_str("Write([REDACTED])"),
        }
    }
}

pub struct TerminalViewModel {
    pub(crate) pane: PaneId,
    pub(crate) session: SessionId,
    pub(crate) profile: ResolvedTerminalProfile,
    pub(crate) alt: PhysicalAltState,
    pub(crate) measured: MeasuredFontMetrics,
    pub(crate) last_logical_allocation: Option<(i32, i32)>,
    pub(crate) last_emitted_size: Option<TerminalSize>,
    pub(crate) last_geometry_delivered: bool,
    pub(crate) frame: Option<Arc<RenderFrame>>,
    pub(crate) search: TerminalSearchState,
    clipboard: Option<TerminalClipboardAction>,
}

impl TerminalViewModel {
    pub fn new(session: SessionId, measured: MeasuredFontMetrics) -> Self {
        Self::with_profile(
            PaneId::new(),
            session,
            TerminalSettingsV1::default().resolve(&Default::default()),
            measured,
        )
    }

    pub fn with_profile(
        pane: PaneId,
        session: SessionId,
        profile: ResolvedTerminalProfile,
        measured: MeasuredFontMetrics,
    ) -> Self {
        Self {
            pane,
            session,
            profile,
            alt: PhysicalAltState::default(),
            measured,
            last_logical_allocation: None,
            last_emitted_size: None,
            last_geometry_delivered: false,
            frame: None,
            search: TerminalSearchState::default(),
            clipboard: None,
        }
    }

    pub fn apply_frame(&mut self, frame: Arc<RenderFrame>) -> FrameUpdate {
        if self
            .frame
            .as_ref()
            .is_some_and(|current| frame.generation <= current.generation)
        {
            return FrameUpdate::rejected();
        }
        let update = FrameUpdate::accepted_from(self.frame.as_deref(), &frame);
        self.frame = Some(frame);
        update
    }

    pub fn frame(&self) -> Option<&Arc<RenderFrame>> {
        self.frame.as_ref()
    }

    pub fn committed_text(&self, text: &str) -> Result<UiCommand, TerminalViewError> {
        if text.contains('\0') {
            return Err(TerminalViewError::InvalidText);
        }
        Ok(
            self.command(SessionUiCommand::Input(TerminalInput::CommittedText(
                text.to_owned(),
            ))),
        )
    }

    pub fn paste(&self, text: &str) -> Result<UiCommand, TerminalViewError> {
        let text =
            ClipboardPolicy::normalize_text(text).map_err(|_| TerminalViewError::InvalidText)?;
        Ok(self.command(SessionUiCommand::paste(text)))
    }

    pub fn search(
        &self,
        needle: &str,
        case_sensitive: bool,
        regex: bool,
    ) -> Result<UiCommand, TerminalViewError> {
        if needle.contains('\0') {
            return Err(TerminalViewError::InvalidText);
        }
        Ok(self.command(SessionUiCommand::Search(SearchQuery {
            needle: needle.to_owned(),
            case_sensitive,
            regex,
        })))
    }

    pub fn apply_search_results(&mut self, matches: Vec<SearchMatch>) {
        self.search.apply(matches);
    }

    pub fn search_is_open(&self) -> bool {
        self.search.is_open()
    }

    pub fn current_search_match(&self) -> Option<SearchMatch> {
        self.search.current()
    }

    pub fn search_matches(&self) -> &[SearchMatch] {
        self.search.matches()
    }

    pub fn current_search_index(&self) -> Option<usize> {
        self.search.current_index()
    }

    pub fn copy(&self) -> UiCommand {
        self.command(SessionUiCommand::CopySelection)
    }

    pub(crate) fn reports_mouse(&self) -> bool {
        self.profile.mouse_reporting
            && self
                .frame
                .as_ref()
                .is_some_and(|frame| frame.mouse_reporting)
    }

    pub fn apply_session_event(&mut self, event: SessionUiEvent) {
        match event {
            SessionUiEvent::Frame(frame) => {
                self.apply_frame(frame);
            }
            SessionUiEvent::Search(matches) => self.apply_search_results(matches),
            SessionUiEvent::Copy(text) => {
                self.clipboard = Some(TerminalClipboardAction::Write(text));
            }
            _ => {}
        }
    }

    pub fn take_clipboard_action(&mut self) -> Option<TerminalClipboardAction> {
        self.clipboard.take()
    }

    pub(crate) fn navigate_search(&mut self, previous: bool) -> Option<UiCommand> {
        let found = self.search.navigate(previous)?;
        Some(self.command(SessionUiCommand::Select(SelectionRange {
            start: found.start,
            end: found.end,
            rectangular: false,
        })))
    }

    pub(crate) fn command(&self, command: SessionUiCommand) -> UiCommand {
        UiCommand::Session {
            session: self.session,
            command,
        }
    }
}
