#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabOverflowModel {
    pub active_index: Option<usize>,
    pub overflow_indices: Vec<usize>,
    tab_count: usize,
}

impl TabOverflowModel {
    pub fn new(tab_count: usize, active_index: Option<usize>, visible_indices: &[usize]) -> Self {
        let active_index = active_index.filter(|index| *index < tab_count);
        let overflow_indices = (0..tab_count)
            .filter(|index| active_index == Some(*index) || !visible_indices.contains(index))
            .collect();
        Self {
            active_index,
            overflow_indices,
            tab_count,
        }
    }

    pub fn cycle(&self, delta: i32) -> Option<usize> {
        if self.tab_count == 0 {
            return None;
        }
        let current = self.active_index.unwrap_or(0) as i64;
        let count = self.tab_count as i64;
        Some((current + i64::from(delta)).rem_euclid(count) as usize)
    }
}
