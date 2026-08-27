use std::hash::{Hash, Hasher};

use alacritty_terminal::{
    Term,
    event::EventListener,
    grid::{Dimensions, Grid},
    index::{Column, Line},
    term::cell::Cell,
};

const ANCHOR_ROWS: usize = 3;

pub(crate) struct CapacityAnchor {
    start: usize,
    maximum_shift: usize,
    hashes: [u64; ANCHOR_ROWS],
}

pub(crate) fn capture<T: EventListener>(
    terminal: &Term<T>,
    history_limit: usize,
    bytes: &[u8],
) -> Option<CapacityAnchor> {
    let grid = terminal.grid();
    let history = grid.history_size();
    if history != history_limit || history < ANCHOR_ROWS {
        return None;
    }
    let start = history - ANCHOR_ROWS;
    let maximum_shift = maximum_shift(grid.columns(), bytes).min(start);
    Some(CapacityAnchor {
        start,
        maximum_shift,
        hashes: std::array::from_fn(|offset| row_hash(grid, start + offset)),
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
        let matches = (0..ANCHOR_ROWS)
            .all(|offset| row_hash(grid, candidate + offset) == anchor.hashes[offset]);
        if matches {
            return anchor.start - candidate;
        }
    }

    // No retained row can be identified. Allocate a disjoint stable range;
    // every prior row is treated as evicted rather than reusing an identity.
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

fn row_hash(grid: &Grid<Cell>, offset: usize) -> u64 {
    let history = grid.history_size();
    let line = Line(offset as i32 - history as i32);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for column in 0..grid.columns() {
        let cell = &grid[line][Column(column)];
        cell.c.hash(&mut hasher);
        cell.flags.bits().hash(&mut hasher);
        cell.zerowidth().hash(&mut hasher);
    }
    hasher.finish()
}
