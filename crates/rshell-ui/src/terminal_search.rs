use rshell_core::SearchMatch;

#[derive(Debug, Default)]
pub(crate) struct TerminalSearchState {
    open: bool,
    matches: Vec<SearchMatch>,
    current: Option<usize>,
}

impl TerminalSearchState {
    pub(crate) fn open(&mut self) {
        self.open = true;
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn apply(&mut self, matches: Vec<SearchMatch>) {
        self.current = (!matches.is_empty()).then_some(0);
        self.matches = matches;
    }

    pub(crate) fn current(&self) -> Option<SearchMatch> {
        self.current
            .and_then(|index| self.matches.get(index).copied())
    }

    pub(crate) fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    pub(crate) fn current_index(&self) -> Option<usize> {
        self.current
    }

    pub(crate) fn navigate(&mut self, previous: bool) -> Option<SearchMatch> {
        if self.matches.is_empty() {
            return None;
        }
        let current = self.current.unwrap_or(0);
        let next = if previous {
            current.checked_sub(1).unwrap_or(self.matches.len() - 1)
        } else {
            (current + 1) % self.matches.len()
        };
        self.current = Some(next);
        Some(self.matches[next])
    }
}

pub(crate) fn search_index(
    matches: &[SearchMatch],
    row: i64,
    column: u16,
    width: u8,
) -> Option<usize> {
    let end = column.saturating_add(u16::from(width));
    matches.iter().position(|found| {
        if row < found.start.stable_row || row > found.end.stable_row {
            return false;
        }
        if found.start.stable_row == found.end.stable_row {
            return found.start.column < end && column < found.end.column;
        }
        (row != found.start.stable_row || found.start.column < end)
            && (row != found.end.stable_row || column < found.end.column)
    })
}
