use super::{
    PaneFocus, Restore,
    watch::{Connections, descendant},
};
use crate::PanePageKind;
use gtk::prelude::*;
use relm4::gtk;
use rshell_core::TabId;

impl PaneFocus {
    fn ready_callback(&self, generation: u64) -> impl Fn() + 'static {
        let state = std::rc::Rc::downgrade(&self.state);
        let content = self.content.clone();
        move || {
            if let Some(state) = state.upgrade() {
                let focus = PaneFocus {
                    state,
                    content: content.clone(),
                };
                focus.try_focus(generation);
                focus.schedule_layout(generation);
            }
        }
    }

    pub(super) fn watch_readiness(&self) {
        let old = self
            .state
            .borrow_mut()
            .request
            .as_mut()
            .map(|r| std::mem::take(&mut r.readiness));
        drop(old);
        let actions = self
            .state
            .borrow_mut()
            .request
            .as_mut()
            .and_then(|r| r.actions.take());
        drop(actions);
        let (generation, terminal) = {
            let state = self.state.borrow();
            if state.request.is_none() {
                return;
            }
            (
                state.policy.generation,
                state.current.as_ref().and_then(|c| c.terminal.clone()),
            )
        };
        let Some(terminal) = terminal.and_then(|w| w.upgrade()) else {
            return;
        };
        let mut connections = Connections::default();
        let ready = self.ready_callback(generation);
        connections.push(&terminal, terminal.connect_map(move |_| ready()));
        if let Some(canvas) = descendant(&terminal, "terminal-canvas")
            .and_then(|c| c.downcast::<gtk::DrawingArea>().ok())
        {
            let ready = self.ready_callback(generation);
            connections.push(&canvas, canvas.connect_resize(move |_, _, _| ready()));
        }
        let restored = self
            .state
            .borrow()
            .request
            .as_ref()
            .and_then(|r| match &r.restore {
                Restore::Descendant(weak) => Some(weak.clone()),
                _ => None,
            });
        if let Some(restored) = restored.and_then(|w| w.upgrade()) {
            let ready = self.ready_callback(generation);
            connections.push(&restored, restored.connect_map(move |_| ready()));
        }
        let action_restore = self
            .state
            .borrow()
            .request
            .as_ref()
            .is_some_and(|r| matches!(r.restore, Restore::Action(_)));
        let actions = action_restore
            .then(|| {
                self.content
                    .upgrade()
                    .and_then(|c| descendant(c.upcast_ref(), "active-pane"))
                    .and_then(|p| descendant(&p, "pane-action-region"))
                    .map(|r| r.observe_children())
            })
            .flatten();
        if let Some(actions) = &actions {
            let ready = self.ready_callback(generation);
            connections.push(
                actions,
                actions.connect_items_changed(move |_, _, _, _| ready()),
            );
        }
        if let Some(request) = self.state.borrow_mut().request.as_mut() {
            request.readiness = connections;
            request.actions = actions;
        }
    }

    pub(crate) fn set_ui_tab(&self, tab: Option<TabId>) {
        let changed_target = {
            let mut state = self.state.borrow_mut();
            state.ui_tab = tab;
            state.policy.observed() && state.policy.bound().is_some_and(|id| Some(id.tab) != tab)
        };
        if changed_target {
            self.invalidate();
        }
    }

    pub(super) fn tab_source(&self) -> (Option<TabId>, Option<TabId>) {
        let state = self.state.borrow();
        (
            state.ui_tab,
            state
                .policy
                .bound()
                .filter(|id| state.current.as_ref().is_some_and(|c| c.identity == *id))
                .map(|id| id.tab),
        )
    }

    pub(super) fn owned_layout_loss(&self) -> bool {
        let state = self.state.borrow();
        state.request.as_ref().is_some_and(|r| {
            r.layout_pending && matches!(r.restore, Restore::Descendant(_) | Restore::Action(_))
        }) && state
            .current
            .as_ref()
            .is_some_and(|c| state.policy.bound() == Some(c.identity))
    }

    pub(super) fn schedule_layout(&self, generation: u64) {
        let eligible = {
            let state = self.state.borrow();
            state.policy.generation == generation
                && state.request.as_ref().is_some_and(|r| !r.layout_pending)
                && state.current.as_ref().is_some_and(|c| {
                    c.page == PanePageKind::Terminal && state.policy.bound() == Some(c.identity)
                })
        };
        if !eligible {
            return;
        }
        let Some(content) = self.content.upgrade() else {
            self.invalidate();
            return;
        };
        if !content.is_mapped() {
            return;
        }
        let Some(clock) = content.frame_clock() else {
            return;
        };
        let state = std::rc::Rc::downgrade(&self.state);
        let weak_content = self.content.clone();
        let id = clock.connect_after_paint(move |_| {
            let Some(state) = state.upgrade() else {
                return;
            };
            let focus = PaneFocus {
                state,
                content: weak_content.clone(),
            };
            let connection = {
                let mut state = focus.state.borrow_mut();
                if state.policy.generation != generation {
                    return;
                }
                state
                    .request
                    .as_mut()
                    .map(|r| std::mem::take(&mut r.layout))
            };
            drop(connection);
            focus.try_focus(generation);
            if let Some(request) = focus.state.borrow_mut().request.as_mut() {
                request.layout_pending = false;
            }
        });
        let mut connection = Connections::default();
        connection.push(&clock, id);
        if let Some(request) = self.state.borrow_mut().request.as_mut() {
            request.layout = connection;
            request.layout_pending = true;
        }
        clock.request_phase(gtk::gdk::FrameClockPhase::AFTER_PAINT);
    }
}
