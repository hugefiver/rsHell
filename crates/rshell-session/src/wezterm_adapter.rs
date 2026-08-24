use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

use rshell_core::{ResolvedTerminalProfile, TerminalSize};
use wezterm_term::{
    Terminal, TerminalConfiguration, TerminalSize as WezTermSize, color::ColorPalette,
};

#[derive(Debug)]
struct RshellTerminalConfig {
    settings: ResolvedTerminalProfile,
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

    pub(crate) fn input(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.terminal.advance_bytes(bytes);
        self.outbound.take()
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
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl SharedWriter {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|error| error.into_inner()))
    }
}

impl Write for SharedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
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
