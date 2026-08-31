use gtk::prelude::*;
use relm4::{ComponentController, gtk};
use rshell_core::{PaneTree, SessionState, SplitAxis};

use crate::{
    ConnectionSidebarMsg, MainWindow, ModalKind, PaneAction, SessionTabBarMsg,
    SmokeVisualCheckpoint, SmokeVisualState,
};

impl MainWindow {
    pub(crate) fn begin_smoke_checkpoint(&mut self, checkpoint: &SmokeVisualCheckpoint) {
        self.smoke_state.visual_focus_trigger = self
            .shell
            .overlay
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root));
        match checkpoint.state {
            SmokeVisualState::Editor => {
                self.send_sidebar(ConnectionSidebarMsg::CreateConnection);
            }
            SmokeVisualState::Settings => self.open_settings(),
            SmokeVisualState::Import => self.open_import(),
            SmokeVisualState::TwentyTabs
            | SmokeVisualState::HostKey
            | SmokeVisualState::Authentication => {}
            _ => self.send_tab(SessionTabBarMsg::RevealActive),
        }
    }

    pub(crate) fn advance_smoke_checkpoint(
        &mut self,
        state: SmokeVisualState,
    ) -> Result<bool, &'static str> {
        if matches!(
            state,
            SmokeVisualState::Single
                | SmokeVisualState::HSplit
                | SmokeVisualState::VSplit
                | SmokeVisualState::TopBottom3
                | SmokeVisualState::Grid
        ) {
            return self.advance_smoke_pane_shape(state);
        }
        if state == SmokeVisualState::TwentyTabs {
            let count = self.view_model.workspace.tabs.len();
            if count < 20 {
                if self.smoke_state.visual_stage_count == Some(count) {
                    return Ok(false);
                }
                self.smoke_state.visual_stage_count = Some(count);
                self.send_tab(SessionTabBarMsg::NewLocalTab);
                return Ok(false);
            }
            self.smoke_state.visual_stage_count = None;
        }
        if state != SmokeVisualState::TwentyTabs {
            self.smoke_state.visual_stage_count = None;
        }
        Ok(self.smoke_checkpoint_ready(state))
    }

    fn smoke_checkpoint_ready(&self, state: SmokeVisualState) -> bool {
        let active = self.view_model.workspace.active_tab();
        match state {
            SmokeVisualState::Empty => self.view_model.catalog.connections.is_empty(),
            SmokeVisualState::Connected => self
                .view_model
                .session_states
                .values()
                .any(|state| *state == SessionState::Connected),
            SmokeVisualState::TwentyTabs => self.view_model.workspace.tabs.len() >= 20,
            SmokeVisualState::Single => {
                active.is_some_and(|tab| tab.pane_tree.pane_ids().len() == 1)
            }
            SmokeVisualState::HSplit => active
                .is_some_and(|tab| root_split(&tab.pane_tree) == Some((SplitAxis::Horizontal, 2))),
            SmokeVisualState::VSplit => active
                .is_some_and(|tab| root_split(&tab.pane_tree) == Some((SplitAxis::Vertical, 2))),
            SmokeVisualState::TopBottom3 => active.is_some_and(|tab| {
                tab.pane_tree.pane_ids().len() == 3 && nested_axes(&tab.pane_tree)
            }),
            SmokeVisualState::Grid => active.is_some_and(|tab| tab.pane_tree.pane_ids().len() >= 4),
            SmokeVisualState::Editor => {
                self.smoke_state.editor_open && self.editor.widget().is_mapped()
            }
            SmokeVisualState::Settings => {
                self.modal.open_kind() == Some(ModalKind::Settings)
                    && self.dialogs.settings.widget().is_mapped()
            }
            SmokeVisualState::Import => {
                self.modal.open_kind() == Some(ModalKind::Import)
                    && self.dialogs.import.widget().is_mapped()
            }
            SmokeVisualState::HostKey => {
                self.modal.open_kind() == Some(ModalKind::Interaction)
                    && self
                        .view_model
                        .session_states
                        .values()
                        .any(|state| *state == SessionState::AwaitingHostKey)
            }
            SmokeVisualState::Authentication => {
                self.modal.open_kind() == Some(ModalKind::Interaction)
                    && self
                        .view_model
                        .session_states
                        .values()
                        .any(|state| *state == SessionState::AwaitingAuthentication)
            }
            SmokeVisualState::Failure => self
                .view_model
                .session_states
                .values()
                .any(|state| matches!(state, SessionState::Failed | SessionState::Crashed)),
            SmokeVisualState::Recovery => !self.view_model.display_recovery.is_empty(),
        }
    }

    fn advance_smoke_pane_shape(&mut self, state: SmokeVisualState) -> Result<bool, &'static str> {
        let Some(active) = self.view_model.workspace.active_tab() else {
            self.send_tab(SessionTabBarMsg::NewLocalTab);
            return Ok(false);
        };
        let count = active.pane_tree.pane_ids().len();
        let ready = match state {
            SmokeVisualState::Single => count == 1,
            SmokeVisualState::HSplit => {
                root_split(&active.pane_tree) == Some((SplitAxis::Horizontal, 2))
            }
            SmokeVisualState::VSplit => {
                root_split(&active.pane_tree) == Some((SplitAxis::Vertical, 2))
            }
            SmokeVisualState::TopBottom3 => count == 3 && nested_axes(&active.pane_tree),
            SmokeVisualState::Grid => count == 4,
            _ => unreachable!("pane shape filtered"),
        };
        if ready {
            self.smoke_state.visual_stage_count = None;
            return Ok(true);
        }
        if self.smoke_state.visual_stage_count == Some(count) {
            return Ok(false);
        }
        self.smoke_state.visual_stage_count = Some(count);
        let action = match (state, count) {
            (SmokeVisualState::HSplit, 1) => Some(PaneAction::SplitHorizontal),
            (SmokeVisualState::VSplit, 1) => Some(PaneAction::SplitVertical),
            (SmokeVisualState::TopBottom3, 1) => Some(PaneAction::SplitVertical),
            (SmokeVisualState::TopBottom3, 2) => Some(PaneAction::SplitHorizontal),
            (SmokeVisualState::Grid, 1 | 3) => Some(PaneAction::SplitHorizontal),
            (SmokeVisualState::Grid, 2) => Some(PaneAction::SplitVertical),
            (_, 2..) => Some(PaneAction::Close),
            _ => None,
        };
        if let Some(action) = action {
            self.send_active_pane_action(action)?;
        } else {
            self.send_tab(SessionTabBarMsg::NewLocalTab);
        }
        Ok(false)
    }

    pub(crate) fn smoke_checkpoint_surface(&self, state: SmokeVisualState) -> Option<gtk::Widget> {
        match state {
            SmokeVisualState::Editor => Some(self.editor.widget().clone().upcast()),
            SmokeVisualState::Settings => Some(self.dialogs.settings.widget().clone().upcast()),
            SmokeVisualState::Import => Some(self.dialogs.import.widget().clone().upcast()),
            SmokeVisualState::HostKey | SmokeVisualState::Authentication => {
                Some(self.dialogs.interaction.widget().clone().upcast())
            }
            _ => None,
        }
    }
}

pub(crate) fn press_escape(surface: &gtk::Widget) -> bool {
    let controllers = surface.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .any(|controller| {
            controller.emit_by_name::<bool>(
                "key-pressed",
                &[
                    &gtk::gdk::Key::Escape,
                    &0u32,
                    &gtk::gdk::ModifierType::empty(),
                ],
            )
        })
}

pub(crate) fn focus_restored(root: &gtk::Widget, trigger: Option<&gtk::Widget>) -> bool {
    let focused = root
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    match trigger {
        Some(trigger) if trigger.root().is_some() => {
            focused.as_ref().is_some_and(|focused| focused == trigger)
        }
        _ => focused.is_some(),
    }
}

fn root_split(tree: &PaneTree) -> Option<(SplitAxis, usize)> {
    match tree {
        PaneTree::Split { axis, .. } => Some((*axis, tree.pane_ids().len())),
        PaneTree::Leaf { .. } => None,
    }
}

fn nested_axes(tree: &PaneTree) -> bool {
    let PaneTree::Split {
        axis,
        first,
        second,
        ..
    } = tree
    else {
        return false;
    };
    let child_axis = [first.as_ref(), second.as_ref()]
        .into_iter()
        .find_map(|child| {
            if let PaneTree::Split { axis, .. } = child {
                Some(*axis)
            } else {
                None
            }
        });
    child_axis.is_some_and(|child| child != *axis)
}
