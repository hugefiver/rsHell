use crate::PaneAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneActionLayout {
    pub visible: Vec<PaneAction>,
    pub overflow: Vec<PaneAction>,
}

impl PaneActionLayout {
    pub fn for_width(actions: &[PaneAction], width: i32) -> Self {
        let capacity = match width {
            ..=159 => 0,
            160..=239 => 1,
            240..=479 => 5,
            _ => actions.len(),
        }
        .min(actions.len());
        if width >= 480 {
            return Self {
                visible: actions.to_vec(),
                overflow: Vec::new(),
            };
        }

        let mut ranked = actions.iter().copied().enumerate().collect::<Vec<_>>();
        ranked.sort_by_key(|(index, action)| (std::cmp::Reverse(action.layout_priority()), *index));
        let selected = ranked
            .into_iter()
            .filter(|(_, action)| width >= 480 || action.layout_priority() > 1)
            .take(capacity)
            .map(|(index, _)| index)
            .collect::<std::collections::BTreeSet<_>>();
        let (visible, overflow) = actions
            .iter()
            .copied()
            .enumerate()
            .partition::<Vec<_>, _>(|(index, _)| selected.contains(index));
        Self {
            visible: visible.into_iter().map(|(_, action)| action).collect(),
            overflow: overflow.into_iter().map(|(_, action)| action).collect(),
        }
    }
}
