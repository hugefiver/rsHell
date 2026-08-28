#[derive(Default)]
pub(crate) struct PrimaryRows {
    pub(crate) origin: i64,
    pub(crate) history: usize,
}

impl PrimaryRows {
    pub(crate) fn reconcile_resize(
        &mut self,
        old_history: usize,
        history: usize,
        primary_scrolls: usize,
    ) {
        let added = history.saturating_sub(old_history);
        let removed = old_history.saturating_sub(history);
        let evicted = primary_scrolls.saturating_sub(added);
        self.origin = self
            .origin
            .saturating_add(i64::try_from(added.saturating_add(evicted)).unwrap_or(i64::MAX))
            .saturating_sub(i64::try_from(removed).unwrap_or(i64::MAX));
        self.history = history;
    }

    pub(crate) fn reconcile_hidden_resize(
        &mut self,
        old_lines: usize,
        lines: usize,
        primary_scrolls: usize,
        history_limit: usize,
    ) {
        if lines < old_lines {
            let added = primary_scrolls.min(history_limit.saturating_sub(self.history));
            self.history = self.history.saturating_add(added);
            self.origin = self
                .origin
                .saturating_add(i64::try_from(primary_scrolls).unwrap_or(i64::MAX));
        } else if lines > old_lines {
            let pulled = self.history.min(lines - old_lines);
            self.history -= pulled;
            self.origin = self
                .origin
                .saturating_sub(i64::try_from(pulled).unwrap_or(i64::MAX));
        }
    }
}
