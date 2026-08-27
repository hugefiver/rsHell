use std::{fmt, sync::Arc};

use rshell_core::{
    MouseButton, MouseEventKind, PaneId, RenderFrame, ResolvedTerminalProfile, SearchMatch,
    SearchQuery, SelectionRange, SessionId, SessionUiCommand, SessionUiEvent, TerminalInput,
    TerminalMouseEvent, TerminalSettingsV1, UiCommand,
};
use rshell_platform::ClipboardPolicy;

use crate::{
    terminal_geometry::{
        PointerEvent, ViewRect, checked_pixel, logical_cell, point_to_cell, point_to_view_cell,
        terminal_size,
    },
    terminal_input::{FontMetrics, PhysicalAltState, TerminalViewError},
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
    metrics: FontMetrics,
    frame: Option<Arc<RenderFrame>>,
    pub(crate) search: TerminalSearchState,
    clipboard: Option<TerminalClipboardAction>,
}

impl TerminalViewModel {
    pub fn new(session: SessionId, metrics: FontMetrics) -> Self {
        Self::with_profile(
            PaneId::new(),
            session,
            TerminalSettingsV1::default().resolve(&Default::default()),
            metrics,
        )
    }

    pub fn with_profile(
        pane: PaneId,
        session: SessionId,
        profile: ResolvedTerminalProfile,
        metrics: FontMetrics,
    ) -> Self {
        Self {
            pane,
            session,
            profile,
            alt: PhysicalAltState::default(),
            metrics,
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

    pub fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    pub fn cursor_rect(&self) -> Option<ViewRect> {
        let frame = self.frame.as_ref()?;
        let cursor = frame.cursor.filter(|cursor| cursor.visible)?;
        let row_index = frame
            .rows
            .iter()
            .position(|row| row.stable_row == cursor.position.stable_row)?;
        let (start, width) = logical_cell(frame, cursor.position)?;
        Some(ViewRect {
            x: f64::from(start) * self.metrics.cell_width,
            y: row_index as f64 * self.metrics.cell_height,
            width: f64::from(width) * self.metrics.cell_width,
            height: self.metrics.cell_height,
        })
    }

    pub fn resize(
        &self,
        width: i32,
        height: i32,
        scale: f64,
    ) -> Result<UiCommand, TerminalViewError> {
        Ok(self.command(SessionUiCommand::Resize(terminal_size(
            width,
            height,
            scale,
            self.metrics,
        )?)))
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

    pub fn mouse(&self, event: PointerEvent) -> Result<Option<UiCommand>, TerminalViewError> {
        let frame = self.frame.as_ref().ok_or(TerminalViewError::OutOfBounds)?;
        let reports_mouse = self.profile.mouse_reporting && frame.mouse_reporting;
        if !reports_mouse && event.kind != MouseEventKind::Scroll {
            return Ok(None);
        }
        if !reports_mouse {
            return Ok((event.scroll_delta != 0)
                .then(|| self.command(SessionUiCommand::Scroll(event.scroll_delta))));
        }
        let (cell, viewport_row) = point_to_view_cell(frame, self.metrics, event.x, event.y)?;
        let button = if event.kind == MouseEventKind::Scroll {
            match event.scroll_delta.cmp(&0) {
                std::cmp::Ordering::Less => Some(MouseButton::WheelUp),
                std::cmp::Ordering::Greater => Some(MouseButton::WheelDown),
                std::cmp::Ordering::Equal => return Ok(None),
            }
        } else {
            event.button
        };
        let pixel_x = checked_pixel(event.x, event.scale)?;
        let pixel_y = checked_pixel(event.y, event.scale)?;
        Ok(Some(self.command(SessionUiCommand::Mouse(
            TerminalMouseEvent {
                kind: event.kind,
                button,
                cell,
                viewport_row,
                pixel_x,
                pixel_y,
                modifiers: event.modifiers,
            },
        ))))
    }

    pub fn selection(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        rectangular: bool,
    ) -> Result<UiCommand, TerminalViewError> {
        let frame = self.frame.as_ref().ok_or(TerminalViewError::OutOfBounds)?;
        let start = point_to_cell(frame, self.metrics, start_x, start_y)?;
        let end = point_to_cell(frame, self.metrics, end_x, end_y)?;
        Ok(self.command(SessionUiCommand::Select(SelectionRange {
            start,
            end,
            rectangular,
        })))
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
