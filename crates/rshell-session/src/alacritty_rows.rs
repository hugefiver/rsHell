use alacritty_terminal::{
    Term,
    event::EventListener,
    grid::{Dimensions, Grid},
    index::Line,
    term::cell::Cell,
};

const ANCHOR_ROWS: usize = 3;

#[derive(Clone, Default)]
pub(crate) struct ScrollTracker {
    state: ScanState,
}

#[derive(Clone, Copy, Default)]
enum ScanState {
    #[default]
    Ground,
    Escape,
    Csi(CsiState),
}

#[derive(Clone, Copy, Default)]
struct CsiState {
    value: usize,
    first_complete: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Prefix {
    pub(crate) length: usize,
    pub(crate) maximum_shift: usize,
}

pub(crate) struct CapacityAnchor {
    start: usize,
    maximum_shift: usize,
    identities: [usize; ANCHOR_ROWS],
    oldest_history_identity: Option<usize>,
}

pub(crate) fn capture<T: EventListener>(
    terminal: &Term<T>,
    maximum_shift: usize,
) -> Option<CapacityAnchor> {
    let grid = terminal.grid();
    if grid.total_lines() < ANCHOR_ROWS {
        return None;
    }
    let start = grid.total_lines() - ANCHOR_ROWS;
    Some(CapacityAnchor {
        start,
        maximum_shift: maximum_shift.min(start),
        identities: std::array::from_fn(|offset| row_identity(grid, start + offset)),
        oldest_history_identity: (grid.history_size() != 0).then(|| row_identity(grid, 0)),
    })
}

impl ScrollTracker {
    pub(crate) fn bounded_prefix(
        &mut self,
        bytes: &[u8],
        columns: usize,
        screen_lines: usize,
        maximum: usize,
    ) -> Option<Prefix> {
        let mut bound = Bound::default();
        let mut previous = 0;
        for (index, byte) in bytes.iter().enumerate() {
            let state = self.state;
            bound.add(self.observe(*byte, screen_lines));
            let maximum_shift = bound.shift(columns);
            if maximum_shift > maximum {
                self.state = state;
                return (index != 0).then_some(Prefix {
                    length: index,
                    maximum_shift: previous,
                });
            }
            previous = maximum_shift;
        }
        Some(Prefix {
            length: bytes.len(),
            maximum_shift: previous,
        })
    }

    pub(crate) fn consume(&mut self, bytes: &[u8], screen_lines: usize) {
        for byte in bytes {
            self.observe(*byte, screen_lines);
        }
    }

    fn observe(&mut self, byte: u8, screen_lines: usize) -> Effect {
        match std::mem::take(&mut self.state) {
            ScanState::Ground => match byte {
                0x1b => {
                    self.state = ScanState::Escape;
                    Effect::default()
                }
                0x9b => {
                    self.state = ScanState::Csi(CsiState::default());
                    Effect::default()
                }
                0x84 | 0x85 => Effect::semantic(1),
                b'\n' | 0x0b | 0x0c => Effect::control(),
                0x20..=0x7e | 0x80..=0xff => Effect::printable(),
                _ => Effect::default(),
            },
            ScanState::Escape => match byte {
                b'[' => {
                    self.state = ScanState::Csi(CsiState::default());
                    Effect::default()
                }
                b'D' | b'E' => Effect::semantic(1),
                0x1b => {
                    self.state = ScanState::Escape;
                    Effect::default()
                }
                _ => Effect::default(),
            },
            ScanState::Csi(mut csi) => {
                if (0x40..=0x7e).contains(&byte) {
                    return match byte {
                        b'S' | b'M' => Effect::semantic(csi.parameter()),
                        b'J' => Effect::semantic(screen_lines),
                        _ => Effect::default(),
                    };
                }
                if byte.is_ascii_digit() && !csi.first_complete {
                    csi.value = csi
                        .value
                        .saturating_mul(10)
                        .saturating_add(usize::from(byte - b'0'));
                } else if byte == b';' {
                    csi.first_complete = true;
                } else if byte == 0x1b {
                    self.state = ScanState::Escape;
                    return Effect::default();
                }
                self.state = ScanState::Csi(csi);
                Effect::default()
            }
        }
    }
}

#[derive(Default)]
struct Bound {
    printable: usize,
    controls: usize,
    semantic: usize,
}

impl Bound {
    fn add(&mut self, effect: Effect) {
        self.printable = self.printable.saturating_add(effect.printable);
        self.controls = self.controls.saturating_add(effect.controls);
        self.semantic = self.semantic.saturating_add(effect.semantic);
    }

    fn shift(&self, columns: usize) -> usize {
        self.printable
            .div_ceil(columns.max(1))
            .saturating_add(self.controls)
            .saturating_add(self.semantic)
    }
}

#[derive(Default)]
struct Effect {
    printable: usize,
    controls: usize,
    semantic: usize,
}

impl Effect {
    fn printable() -> Self {
        Self {
            printable: 1,
            ..Self::default()
        }
    }

    fn control() -> Self {
        Self {
            controls: 1,
            ..Self::default()
        }
    }

    fn semantic(lines: usize) -> Self {
        Self {
            semantic: lines,
            ..Self::default()
        }
    }
}

impl CsiState {
    fn parameter(&self) -> usize {
        self.value.max(1)
    }
}

pub(crate) fn bounded_prefix<T: EventListener>(
    tracker: &mut ScrollTracker,
    bytes: &[u8],
    terminal: &Term<T>,
    history_limit: usize,
) -> Option<Prefix> {
    let grid = terminal.grid();
    let room = history_limit.saturating_sub(grid.history_size());
    let maximum = if room == 0 {
        grid.total_lines().saturating_sub(ANCHOR_ROWS)
    } else {
        room
    };
    tracker.bounded_prefix(bytes, grid.columns(), grid.screen_lines(), maximum)
}

pub(crate) fn completed_shift<T: EventListener>(
    terminal: &Term<T>,
    history_limit: usize,
    anchor: Option<CapacityAnchor>,
) -> usize {
    let Some(anchor) = anchor else {
        return 0;
    };
    let grid = terminal.grid();
    if grid.history_size() != history_limit {
        return 0;
    }
    let lower = anchor.start.saturating_sub(anchor.maximum_shift);
    for candidate in (lower..=anchor.start).rev() {
        let identities = std::array::from_fn(|offset| row_identity(grid, candidate + offset));
        if identities == anchor.identities {
            return anchor.start - candidate;
        }
    }

    if anchor
        .oldest_history_identity
        .is_some_and(|identity| row_identity(grid, 0) == identity)
    {
        return 0;
    }

    // The bounded window cannot evict an anchored active row and reuse its slot.
    // Addresses are copied before feeding and compared only in this window. If no
    // retained row is observable, reserve a disjoint stable range rather than reuse IDs.
    grid.total_lines()
}

fn row_identity(grid: &Grid<Cell>, offset: usize) -> usize {
    let history = grid.history_size();
    let line = Line(offset as i32 - history as i32);
    std::ptr::from_ref(&grid[line]) as usize
}
