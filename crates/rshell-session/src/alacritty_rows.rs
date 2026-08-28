use alacritty_terminal::{
    Term,
    event::EventListener,
    grid::{Dimensions, Grid},
    index::Line,
    term::cell::Cell,
};

const ANCHOR_ROWS: usize = 3;

pub(crate) struct CapacityAnchor {
    start: usize,
    maximum_shift: usize,
    identities: [usize; ANCHOR_ROWS],
}

pub(crate) fn capture<T: EventListener>(
    terminal: &Term<T>,
    history_limit: usize,
    bytes: &[u8],
) -> Option<CapacityAnchor> {
    let grid = terminal.grid();
    let history = grid.history_size();
    if history != history_limit || grid.total_lines() < ANCHOR_ROWS {
        return None;
    }
    let start = grid.total_lines() - ANCHOR_ROWS;
    let maximum_shift = maximum_shift(grid.columns(), bytes).min(start);
    Some(CapacityAnchor {
        start,
        maximum_shift,
        identities: std::array::from_fn(|offset| row_identity(grid, start + offset)),
    })
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
        let matches = identities == anchor.identities;
        if matches {
            return anchor.start - candidate;
        }
    }

    // At saturated history, rotating the grid ring does not reallocate a Row. We
    // copy addresses only within this feed window and never dereference them after
    // mutation, so equal contents cannot reuse an identity. If no retained anchor
    // is observed, reserve a disjoint stable range rather than reusing IDs.
    grid.total_lines()
}

fn maximum_shift(columns: usize, bytes: &[u8]) -> usize {
    let printable_bound = bytes.len().div_ceil(columns.max(1));
    let controls = bytes
        .iter()
        .filter(|byte| matches!(byte, b'\n' | 0x0b | 0x0c))
        .count();
    printable_bound.saturating_add(controls).saturating_add(2)
}

fn row_identity(grid: &Grid<Cell>, offset: usize) -> usize {
    let history = grid.history_size();
    let line = Line(offset as i32 - history as i32);
    std::ptr::from_ref(&grid[line]) as usize
}
