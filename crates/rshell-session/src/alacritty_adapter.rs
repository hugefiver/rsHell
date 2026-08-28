use alacritty_terminal::{
    Term,
    grid::Dimensions,
    index::Line,
    term::{Config, TermMode},
    vte::ansi::Processor,
};
use rshell_core::{
    KeyCode, KeyModifiers, ResolvedTerminalProfile, TerminalMouseEvent, TerminalSize,
};

use crate::{
    EngineError, ViewportBounds, alacritty_event::EventSink, alacritty_feed, alacritty_key,
    alacritty_mouse, alacritty_tracker,
};

pub(crate) struct AlacrittyAdapter {
    terminal: Term<EventSink>,
    processor: Processor,
    events: EventSink,
    settings: ResolvedTerminalProfile,
    size: TerminalSize,
    primary_origin: i64,
    scroll_tracker: alacritty_tracker::ScrollTracker,
}

impl AlacrittyAdapter {
    pub(crate) fn new(settings: &ResolvedTerminalProfile, size: TerminalSize) -> Self {
        let events = EventSink::new(size);
        let terminal = make_terminal(settings, size, events.clone());
        Self {
            terminal,
            processor: Processor::new(),
            events,
            settings: settings.clone(),
            size,
            primary_origin: 0,
            scroll_tracker: alacritty_tracker::ScrollTracker::new(
                usize::from(size.cols),
                usize::from(size.rows),
            ),
        }
    }

    pub(crate) fn terminal(&self) -> &Term<EventSink> {
        &self.terminal
    }

    pub(crate) fn size(&self) -> TerminalSize {
        self.size
    }

    pub(crate) fn title(&self) -> String {
        self.events.title()
    }

    pub(crate) fn mouse_reporting_active(&self) -> bool {
        self.settings.mouse_reporting && self.terminal.mode().intersects(TermMode::MOUSE_MODE)
    }

    pub(crate) fn alternate_screen(&self) -> bool {
        self.terminal.mode().contains(TermMode::ALT_SCREEN)
    }

    pub(crate) fn origin(&self) -> i64 {
        if self.alternate_screen() {
            0
        } else {
            self.primary_origin
        }
    }

    pub(crate) fn stable_row(&self, line: Line) -> i64 {
        self.origin().saturating_add(i64::from(line.0))
    }

    pub(crate) fn backend_line(&self, stable_row: i64) -> Option<Line> {
        let line = stable_row.checked_sub(self.origin())?;
        let line = i32::try_from(line).ok()?;
        let line = Line(line);
        (line >= self.terminal.grid().topmost_line()
            && line <= self.terminal.grid().bottommost_line())
        .then_some(line)
    }

    pub(crate) fn encode_key(
        &self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<Vec<u8>, EngineError> {
        alacritty_key::encode(
            code,
            modifiers,
            *self.terminal.mode(),
            self.settings.enable_csi_u,
        )
    }

    pub(crate) fn encode_mouse(&self, event: TerminalMouseEvent) -> Result<Vec<u8>, EngineError> {
        alacritty_mouse::encode(event, *self.terminal.mode())
    }

    pub(crate) fn viewport_bounds(&self) -> ViewportBounds {
        let history = self.terminal.grid().history_size() as i64;
        let first_stable_row = self.origin().saturating_sub(history);
        ViewportBounds {
            first_stable_row,
            bottom_top_stable_row: self.origin(),
        }
    }

    pub(crate) fn input(&mut self, bytes: &[u8]) -> Vec<u8> {
        alacritty_feed::advance(
            &mut self.processor,
            &mut self.terminal,
            &self.events,
            &self.settings,
            &mut self.primary_origin,
            &mut self.scroll_tracker,
            bytes,
        )
    }

    pub(crate) fn resize(&mut self, size: TerminalSize) {
        let old_history = self.terminal.grid().history_size();
        self.terminal.resize(GridSize::from(size));
        let history = self.terminal.grid().history_size();
        if !self.alternate_screen() && history > old_history {
            self.primary_origin = self
                .primary_origin
                .saturating_add((history - old_history) as i64);
        }
        self.events.resize(size);
        self.size = size;
        self.scroll_tracker
            .resize(usize::from(size.cols), usize::from(size.rows));
    }

    pub(crate) fn clear_scrollback(&mut self) {
        self.terminal.grid_mut().clear_history();
    }

    pub(crate) fn reset(&mut self) {
        self.events.take_outbound();
        self.events = EventSink::new(self.size);
        self.terminal = make_terminal(&self.settings, self.size, self.events.clone());
        self.processor = Processor::new();
        self.primary_origin = 0;
        self.scroll_tracker = alacritty_tracker::ScrollTracker::new(
            usize::from(self.size.cols),
            usize::from(self.size.rows),
        );
    }
}

fn make_terminal(
    settings: &ResolvedTerminalProfile,
    size: TerminalSize,
    events: EventSink,
) -> Term<EventSink> {
    let config = Config {
        scrolling_history: settings.scrollback_lines,
        kitty_keyboard: settings.enable_kitty_keyboard,
        ..Config::default()
    };
    Term::new(config, &GridSize::from(size), events)
}

#[derive(Clone, Copy)]
struct GridSize {
    columns: usize,
    lines: usize,
}

impl From<TerminalSize> for GridSize {
    fn from(size: TerminalSize) -> Self {
        Self {
            columns: usize::from(size.cols),
            lines: usize::from(size.rows),
        }
    }
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}
