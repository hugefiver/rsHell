use crate::alacritty_tracker_presentation::Presentation;

#[derive(Clone, Copy)]
pub(crate) enum Window {
    Bounded { length: usize, maximum_shift: usize },
    Unsafe { length: usize },
}

#[derive(Clone, Copy)]
pub(crate) struct ScrollTracker {
    state: ScanState,
    primary: Presentation,
    alternate: Presentation,
}

#[derive(Clone, Copy, Default)]
enum ScanState {
    #[default]
    Ground,
    Escape,
    Csi(Csi),
}

#[derive(Clone, Copy, Default)]
pub(super) struct Csi {
    params: [Option<usize>; 2],
    index: usize,
}

impl ScrollTracker {
    pub(crate) fn new(columns: usize, lines: usize) -> Self {
        let presentation = Presentation::new(columns, lines);
        Self {
            state: ScanState::Ground,
            primary: presentation,
            alternate: presentation,
        }
    }

    pub(crate) fn resize(&mut self, columns: usize, lines: usize) {
        self.primary.resize(columns, lines);
        self.alternate.resize(columns, lines);
    }

    pub(crate) fn next_window(&mut self, bytes: &[u8], primary: bool, maximum: usize) -> Window {
        let mut shift = 0usize;
        let mut sequence = None;
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            let before = *self;
            let previous_shift = shift;
            if matches!(self.state, ScanState::Ground) && printable(byte) {
                let length = printable_run(&bytes[index..]);
                let effect = self.presentation(primary).print_many(length, primary);
                if shift.saturating_add(effect) <= maximum {
                    shift = shift.saturating_add(effect);
                    index += length;
                    continue;
                }
                *self = before;
            }
            if matches!(self.state, ScanState::Ground) && matches!(byte, 0x1b | 0x9b) {
                sequence = Some((index, before, shift));
            }
            shift = shift.saturating_add(self.observe(byte, primary));
            if shift <= maximum {
                if matches!(self.state, ScanState::Ground) {
                    sequence = None;
                }
                index += 1;
                continue;
            }
            if let Some((start, state, bound)) = sequence {
                if start == 0 {
                    return Window::Unsafe { length: index + 1 };
                }
                *self = state;
                return Window::Bounded {
                    length: start,
                    maximum_shift: bound,
                };
            }
            if index != 0 {
                *self = before;
                return Window::Bounded {
                    length: index,
                    maximum_shift: previous_shift,
                };
            }
            return Window::Unsafe { length: 1 };
        }
        Window::Bounded {
            length: bytes.len(),
            maximum_shift: shift,
        }
    }

    pub(crate) fn consume(&mut self, bytes: &[u8], primary: bool) {
        for byte in bytes {
            self.observe(*byte, primary);
        }
    }

    fn observe(&mut self, byte: u8, primary: bool) -> usize {
        match self.state {
            ScanState::Ground => match byte {
                0x1b => self.state = ScanState::Escape,
                0x9b => self.state = ScanState::Csi(Csi::default()),
                0x84 => return self.presentation(primary).linefeed(primary),
                0x85 => return self.presentation(primary).newline(primary),
                b'\n' | 0x0b | 0x0c => return self.presentation(primary).linefeed(primary),
                b'\r' => self.presentation(primary).carriage_return(),
                0x08 => self.presentation(primary).backspace(),
                b'\t' => return self.presentation(primary).tab(primary),
                0x20..=0x7e | 0x80..=0xff => return self.presentation(primary).print(primary),
                _ => {}
            },
            ScanState::Escape => match byte {
                b'[' => self.state = ScanState::Csi(Csi::default()),
                b'D' => {
                    self.state = ScanState::Ground;
                    return self.presentation(primary).linefeed(primary);
                }
                b'E' => {
                    self.state = ScanState::Ground;
                    return self.presentation(primary).newline(primary);
                }
                0x1b => self.state = ScanState::Escape,
                _ => self.state = ScanState::Ground,
            },
            ScanState::Csi(mut csi) => {
                if (0x40..=0x7e).contains(&byte) {
                    self.state = ScanState::Ground;
                    return self.presentation(primary).csi(byte, csi, primary);
                }
                if byte.is_ascii_digit() {
                    csi.push(byte);
                } else if byte == b';' {
                    csi.index = (csi.index + 1).min(1);
                } else if byte == 0x1b {
                    self.state = ScanState::Escape;
                    return 0;
                }
                self.state = ScanState::Csi(csi);
            }
        }
        0
    }

    fn presentation(&mut self, primary: bool) -> &mut Presentation {
        if primary {
            &mut self.primary
        } else {
            &mut self.alternate
        }
    }
}

fn printable(byte: u8) -> bool {
    matches!(byte, 0x20..=0x7e | 0xa0..=0xff)
}

fn printable_run(bytes: &[u8]) -> usize {
    bytes.iter().take_while(|byte| printable(**byte)).count()
}

impl Csi {
    pub(super) fn parameter(self, index: usize, default: usize) -> usize {
        self.params[index].unwrap_or(default).max(1)
    }

    fn push(&mut self, byte: u8) {
        let value = self.params[self.index].unwrap_or(0);
        self.params[self.index] = Some(
            value
                .saturating_mul(10)
                .saturating_add(usize::from(byte - b'0')),
        );
    }
}
