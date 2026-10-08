use super::{
    PaneFocus, Restore,
    policy::selected,
    watch::{Connections, Origin, descendant, owns},
};
use crate::{
    PaneHost, PanePageKind, pane_host_layout::request_layout_frame,
    pane_host_render::render_projection, pane_host_terminals::detach_terminals,
};
use gtk::prelude::*;
use relm4::{ComponentController, ComponentSender, gtk};

impl PaneFocus {
    pub(in crate::pane_host) fn activate(&self, host: &PaneHost) {
        self.invalidate();
        if let Some((identity, page)) = selected(&host.model)
            && matches!(page, PanePageKind::Pending | PanePageKind::Terminal)
        {
            self.state.borrow_mut().policy.activate(identity);
            let focused = host
                .content
                .root()
                .and_then(|r| r.downcast::<gtk::Window>().ok())
                .and_then(|r| gtk::prelude::RootExt::focus(&r).map(|w| (r, w)))
                .filter(|(_, w)| {
                    host.terminals
                        .get(&identity.session)
                        .is_some_and(|t| owns(w, t.widget().upcast_ref()))
                });
            if let Some((root, widget)) = focused {
                self.install(
                    Origin {
                        root: root.downgrade(),
                        widget: widget.downgrade(),
                        tab_action: false,
                    },
                    Restore::Descendant(widget.downgrade()),
                );
            } else {
                self.start(false, Restore::Canvas);
            }
        }
    }
    fn prepare_reparent(&self) {
        let origin = self
            .state
            .borrow()
            .request
            .as_ref()
            .and_then(|r| matches!(r.restore, Restore::Action(_)).then(|| r.origin.clone()));
        if let Some(origin) = origin
            && let Some(content) = self.content.upgrade()
            && let Some(focus) = origin.current_focus(&content)
            && origin.owns_focus(focus.as_ref())
            && let Some(root) = origin.root.upgrade()
        {
            gtk::prelude::GtkWindowExt::set_focus(&root, gtk::Widget::NONE);
        }
    }
    pub(in crate::pane_host) fn capture_owned(&self, host: &PaneHost) {
        self.state.borrow_mut().mechanical = true;
        let active = self.state.borrow().policy.active();
        if active || !host.content.is_sensitive() {
            return;
        }
        let Some((identity, _)) = selected(&host.model) else {
            return;
        };
        if self.state.borrow().rendered != Some(identity) {
            return;
        }
        let Some(root) = host
            .content
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
        else {
            return;
        };
        let Some(focused) = gtk::prelude::RootExt::focus(&root) else {
            return;
        };
        let terminal = host
            .terminals
            .get(&identity.session)
            .map(relm4::ComponentController::widget);
        let restore = if terminal.is_some_and(|t| owns(&focused, t.upcast_ref())) {
            Restore::Descendant(focused.downgrade())
        } else if focused.is::<gtk::Button>()
            && descendant(host.content.upcast_ref(), "active-pane")
                .is_some_and(|pane| owns(&focused, &pane))
            && let Some(tooltip) = focused.tooltip_text()
        {
            Restore::Action(tooltip.into())
        } else {
            return;
        };
        self.state.borrow_mut().policy.activate(identity);
        self.install(
            Origin {
                root: root.downgrade(),
                widget: focused.downgrade(),
                tab_action: false,
            },
            restore,
        );
    }

    pub(super) fn observe(&self, origin: Origin) {
        let generation = self.state.borrow().policy.generation;
        let mut connections = Connections::default();
        if let Some(root) = origin.root.upgrade() {
            let check = self.callback(generation, false);
            connections.push(
                &root,
                root.connect_notify_local(Some("focus-widget"), move |_, _| check()),
            );
            let cancel = self.callback(generation, true);
            connections.push(&root, root.connect_unmap(move |_| cancel()));
        }
        if let Some(content) = self.content.upgrade() {
            let check = self.callback(generation, false);
            connections.push(
                &content,
                content.connect_notify_local(Some("root"), move |_, _| check()),
            );
        }
        if let Some(request) = self.state.borrow_mut().request.as_mut() {
            request.connections = connections;
        }
    }

    fn callback(&self, generation: u64, cancel: bool) -> impl Fn() + 'static {
        let state = std::rc::Rc::downgrade(&self.state);
        let content = self.content.clone();
        move || {
            let Some(state) = state.upgrade() else {
                return;
            };
            let focus = PaneFocus {
                state,
                content: content.clone(),
            };
            if focus.state.borrow().policy.generation != generation {
                return;
            }
            if cancel {
                focus.invalidate();
            } else {
                focus.check_origin();
            }
        }
    }

    fn finish_reparent(&self) {
        self.state.borrow_mut().mechanical = false;
        if let Some(content) = self.content.upgrade() {
            let origin = self
                .state
                .borrow()
                .request
                .as_ref()
                .map(|r| r.origin.clone());
            if let Some(origin) = origin
                && let Some(focused) = origin.current_focus(&content)
                && !origin.owns_focus(focused.as_ref())
                && super::watch::mechanical_focus(focused.as_ref(), &content)
                && let Some(request) = self.state.borrow_mut().request.as_mut()
            {
                request.fallback = Some(focused.map(|w| w.downgrade()));
            }
        }
        let generation = self.state.borrow().policy.generation;
        self.schedule_layout(generation);
    }
}

impl PaneHost {
    pub(in crate::pane_host) fn render(&self, sender: &ComponentSender<Self>) {
        self.focus.state.borrow_mut().mechanical = true;
        self.focus.prepare_reparent();
        detach_terminals(&self.terminals);
        self.content.set_child(gtk::Widget::NONE);
        if let Some(tab) = self.model.active_tab() {
            if let Some(projection) = self.model.projection(tab) {
                let active = self.model.active_pane(tab);
                let projection = render_projection(&projection, active, &self.terminals, sender);
                self.content.set_child(Some(&projection));
                request_layout_frame(&projection);
            }
        } else {
            let empty = gtk::Label::new(Some("No terminal tabs"));
            empty.add_css_class("pane-state-label");
            self.content.set_child(Some(&empty));
            self.focus.state.borrow_mut().rendered = None;
            self.geometry.schedule(&self.content, sender);
            self.focus.finish_reparent();
            return;
        }
        self.focus.state.borrow_mut().rendered = selected(&self.model).map(|(id, _)| id);
        request_layout_frame(&self.content);
        self.geometry.schedule(&self.content, sender);
        if let Some(root) = self.content.root() {
            root.queue_resize();
            root.queue_draw();
            if let Ok(window) = root.downcast::<gtk::Window>()
                && let Some(surface) = window.surface()
            {
                surface.queue_render();
            }
        }
        self.focus.watch_readiness();
        self.focus.finish_reparent();
    }
}
