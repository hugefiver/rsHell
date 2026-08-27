use std::sync::Arc;

use rshell_core::{
    KeyCode, KeyModifiers, ResolvedTerminalProfile, TerminalMouseEvent, TerminalSize,
};
use wezterm_term::{
    KeyCode as WezKeyCode, KeyModifiers as WezKeyModifiers, Terminal, TerminalConfiguration,
    TerminalSize as WezTermSize, color::ColorPalette,
};

use crate::{
    EngineError, ViewportBounds,
    wezterm_input::{map_key, map_key_modifiers, map_mouse},
    wezterm_writer::SharedWriter,
};

#[derive(Debug)]
struct RshellTerminalConfig {
    settings: ResolvedTerminalProfile,
    mouse_reporting_allowed: bool,
}

impl TerminalConfiguration for RshellTerminalConfig {
    fn scrollback_size(&self) -> usize {
        self.settings.scrollback_lines
    }

    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }

    fn enable_csi_u_key_encoding(&self) -> bool {
        self.settings.enable_csi_u
    }

    fn enable_kitty_keyboard(&self) -> bool {
        self.settings.enable_kitty_keyboard
    }

    fn enq_answerback(&self) -> String {
        self.settings.answerback.clone()
    }
}

pub(crate) struct WezTermAdapter {
    terminal: Terminal,
    config: Arc<RshellTerminalConfig>,
    outbound: SharedWriter,
    size: TerminalSize,
}

impl WezTermAdapter {
    pub(crate) fn new(settings: &ResolvedTerminalProfile, size: TerminalSize) -> Self {
        let config = Arc::new(RshellTerminalConfig {
            settings: settings.clone(),
            mouse_reporting_allowed: settings.mouse_reporting,
        });
        let outbound = SharedWriter::default();
        let terminal = make_terminal(size, config.clone(), outbound.clone());
        Self {
            terminal,
            config,
            outbound,
            size,
        }
    }

    pub(crate) fn terminal(&self) -> &Terminal {
        &self.terminal
    }

    pub(crate) fn size(&self) -> TerminalSize {
        self.size
    }

    pub(crate) fn mouse_reporting_allowed(&self) -> bool {
        self.config.mouse_reporting_allowed
    }

    pub(crate) fn encode_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Result<Vec<u8>, EngineError> {
        let key = map_key(code)?;
        let modifiers = map_key_modifiers(modifiers)?;
        self.terminal
            .key_down(key, modifiers)
            .map_err(|_| EngineError::UnsupportedInput("wezterm key encoding failed"))?;
        self.drain_with_barrier()
    }

    pub(crate) fn encode_mouse(
        &mut self,
        event: TerminalMouseEvent,
    ) -> Result<Vec<u8>, EngineError> {
        let event = map_mouse(event, self.size)?;
        self.terminal
            .mouse_event(event)
            .map_err(|_| EngineError::UnsupportedMouse("wezterm mouse encoding failed"))?;
        self.drain_with_barrier()
    }

    pub(crate) fn viewport_bounds(&self) -> ViewportBounds {
        let screen = self.terminal.screen();
        let rows = screen.scrollback_rows();
        let first_stable_row = stable_row(screen.phys_to_stable_row_index(0));
        let end_stable_row = stable_row(screen.phys_to_stable_row_index(rows));
        ViewportBounds {
            first_stable_row,
            bottom_top_stable_row: end_stable_row
                .saturating_sub(i64::from(self.size.rows))
                .max(first_stable_row),
        }
    }

    pub(crate) fn input(&mut self, bytes: &[u8]) -> Result<Vec<u8>, EngineError> {
        self.terminal.advance_bytes(bytes);
        self.drain_with_barrier()
    }

    pub(crate) fn resize(&mut self, size: TerminalSize) {
        self.terminal.resize(to_backend_size(size));
        self.size = size;
    }

    pub(crate) fn clear_scrollback(&mut self) {
        self.terminal.erase_scrollback();
    }

    pub(crate) fn reset(&mut self) {
        self.outbound.take();
        self.terminal = make_terminal(self.size, self.config.clone(), self.outbound.clone());
    }

    fn drain_with_barrier(&mut self) -> Result<Vec<u8>, EngineError> {
        // The pinned terminal forwards writer bytes on a background thread. A
        // synthetic key provides an ordered drain marker; its bytes are removed
        // before the payload can reach the transport.
        const BARRIER: char = '\u{10ffff}';
        self.terminal
            .key_down(WezKeyCode::Char(BARRIER), WezKeyModifiers::NONE)
            .map_err(|_| EngineError::UnsupportedInput("wezterm writer barrier failed"))?;
        let mut encoded = [0; 4];
        Ok(self
            .outbound
            .take_through(BARRIER.encode_utf8(&mut encoded).as_bytes()))
    }
}

fn make_terminal(
    size: TerminalSize,
    config: Arc<RshellTerminalConfig>,
    outbound: SharedWriter,
) -> Terminal {
    let mut terminal = Terminal::new(
        to_backend_size(size),
        config,
        "rsHell",
        env!("CARGO_PKG_VERSION"),
        Box::new(outbound),
    );
    terminal.advance_bytes(b"\x1b]0;rsHell\x07");
    terminal
}

fn to_backend_size(size: TerminalSize) -> WezTermSize {
    WezTermSize {
        rows: usize::from(size.rows),
        cols: usize::from(size.cols),
        pixel_width: size.pixel_width as usize,
        pixel_height: size.pixel_height as usize,
        dpi: size.dpi,
    }
}

fn stable_row(index: isize) -> i64 {
    i64::try_from(index).unwrap_or(if index.is_negative() {
        i64::MIN
    } else {
        i64::MAX
    })
}
