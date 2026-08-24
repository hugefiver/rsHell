use rshell_core::{AppViewModel, PaneTree, SplitAxis};

use crate::SessionPaneViewModel;

#[derive(Debug, Clone)]
pub enum PaneProjection {
    Leaf(SessionPaneViewModel),
    Split {
        axis: SplitAxis,
        ratio: f32,
        first: Box<PaneProjection>,
        second: Box<PaneProjection>,
    },
}

impl PaneProjection {
    pub fn from_app(app: &AppViewModel, tree: &PaneTree) -> Self {
        match tree {
            PaneTree::Leaf {
                pane_id,
                session_id,
            } => Self::Leaf(SessionPaneViewModel::from_leaf(app, *pane_id, *session_id)),
            PaneTree::Split {
                axis,
                ratio,
                first,
                second,
            } => Self::Split {
                axis: *axis,
                ratio: *ratio,
                first: Box::new(Self::from_app(app, first)),
                second: Box::new(Self::from_app(app, second)),
            },
        }
    }

    pub fn axes(&self) -> Vec<SplitAxis> {
        let mut axes = Vec::new();
        self.collect_axes(&mut axes);
        axes
    }

    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    fn collect_axes(&self, axes: &mut Vec<SplitAxis>) {
        if let Self::Split {
            axis,
            first,
            second,
            ..
        } = self
        {
            axes.push(*axis);
            first.collect_axes(axes);
            second.collect_axes(axes);
        }
    }
}
