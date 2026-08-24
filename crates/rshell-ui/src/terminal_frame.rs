use std::collections::BTreeSet;

use rshell_core::RenderFrame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameUpdate {
    accepted: bool,
    dirty_rows: Vec<usize>,
}

impl FrameUpdate {
    pub(crate) fn rejected() -> Self {
        Self {
            accepted: false,
            dirty_rows: Vec::new(),
        }
    }

    pub(crate) fn accepted_from(previous: Option<&RenderFrame>, frame: &RenderFrame) -> Self {
        let dirty_rows = dirty_frame_rows(previous, frame);
        Self {
            accepted: true,
            dirty_rows,
        }
    }

    pub fn accepted(&self) -> bool {
        self.accepted
    }

    pub fn dirty_rows(&self) -> &[usize] {
        &self.dirty_rows
    }
}

pub(crate) fn dirty_frame_rows(previous: Option<&RenderFrame>, frame: &RenderFrame) -> Vec<usize> {
    let Some(previous) = previous else {
        return (0..frame.rows.len()).collect();
    };
    let row_count = previous.rows.len().max(frame.rows.len());
    if previous.size != frame.size || previous.viewport_top != frame.viewport_top {
        return (0..row_count).collect();
    }

    let mut dirty = (0..row_count)
        .filter(|index| previous.rows.get(*index) != frame.rows.get(*index))
        .collect::<BTreeSet<_>>();
    if previous.cursor != frame.cursor {
        add_cursor_row(previous, &mut dirty);
        add_cursor_row(frame, &mut dirty);
    }
    dirty.into_iter().collect()
}

fn add_cursor_row(frame: &RenderFrame, dirty: &mut BTreeSet<usize>) {
    let Some(cursor) = frame.cursor.filter(|cursor| cursor.visible) else {
        return;
    };
    if let Some(index) = frame
        .rows
        .iter()
        .position(|row| row.stable_row == cursor.position.stable_row)
    {
        dirty.insert(index);
    }
}
