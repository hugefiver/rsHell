use crate::{PaneHost, PanePageKind};
use gtk::prelude::*;
use relm4::{ComponentController, gtk};
use rshell_core::TabId;
use std::{cell::RefCell, rc::Rc};

#[path = "pane_host_focus_completion.rs"]
mod completion;

#[path = "pane_host_focus_layout.rs"]
mod layout;
#[path = "pane_host_focus_lifecycle.rs"]
mod lifecycle;
#[path = "pane_host_focus_state.rs"]
mod policy;
#[path = "pane_host_focus_startup.rs"]
mod startup;
#[path = "pane_host_focus_watch.rs"]
mod watch;
use policy::{FocusPolicy, Identity, selected};
use watch::{Connections, Origin, descendant, owns};

#[derive(Default)]
struct State {
    policy: FocusPolicy,
    request: Option<Request>,
    current: Option<Current>,
    mechanical: bool,
    ui_tab: Option<TabId>,
    rendered: Option<Identity>,
}
struct Request {
    origin: Origin,
    fallback: Option<Option<gtk::glib::WeakRef<gtk::Widget>>>,
    restore: Restore,
    connections: Connections,
    readiness: Connections,
    layout: Connections,
    layout_pending: bool,
    actions: Option<gtk::gio::ListModel>,
    receipt: Option<gtk::glib::JoinHandle<()>>,
    startup: Option<startup::StartupAnchor>,
}
#[derive(Clone)]
enum Restore {
    Canvas,
    Descendant(gtk::glib::WeakRef<gtk::Widget>),
    Action(String),
}
#[derive(Clone)]
struct Current {
    identity: Identity,
    page: PanePageKind,
    terminal: Option<gtk::glib::WeakRef<gtk::Widget>>,
}
#[derive(Clone)]
pub(crate) struct PaneFocus {
    state: Rc<RefCell<State>>,
    content: gtk::glib::WeakRef<gtk::Overlay>,
}
impl PaneFocus {
    pub(super) fn new(content: &gtk::Overlay) -> Self {
        Self {
            state: Rc::new(RefCell::new(State::default())),
            content: content.downgrade(),
        }
    }
    pub(crate) fn cancel(&self) {
        self.state.borrow_mut().policy.cancel();
        self.clear_watch();
    }
    fn invalidate(&self) {
        self.state.borrow_mut().policy.cancel();
        self.clear_watch();
    }
    fn clear_watch(&self) {
        let request = self.state.borrow_mut().request.take();
        drop(request); // Signal disconnection must not hold our RefCell borrow.
    }
    fn start(&self, new: bool, restore: Restore) {
        let Some(content) = self.content.upgrade() else {
            self.invalidate();
            return;
        };
        let Some(origin) = watch::origin(&content, new) else {
            self.invalidate();
            return;
        };
        self.install(origin, restore);
    }
    fn install(&self, origin: Origin, restore: Restore) {
        self.clear_watch();
        self.state.borrow_mut().request = Some(Request {
            origin: origin.clone(),
            fallback: None,
            restore,
            connections: Connections::default(),
            readiness: Connections::default(),
            layout: Connections::default(),
            layout_pending: false,
            actions: None,
            receipt: None,
            startup: None,
        });
        self.observe(origin);
    }
    pub(super) fn synchronize(&self, host: &PaneHost) {
        let current = selected(&host.model).map(|(identity, page)| Current {
            identity,
            page,
            terminal: host
                .terminals
                .get(&identity.session)
                .map(|t| t.widget().upcast_ref::<gtk::Widget>().downgrade()),
        });
        {
            let mut state = self.state.borrow_mut();
            state.policy.synchronize(&host.model);
            state.current = current;
        }
        if !self.state.borrow().policy.active() {
            self.clear_watch();
        }
        self.watch_readiness();
    }
    fn try_focus(&self, generation: u64) {
        let eligible = {
            let state = self.state.borrow();
            !state.mechanical && state.policy.generation == generation
        };
        if !eligible || !self.check_origin() {
            return;
        }
        let request = {
            let state = self.state.borrow();
            let Some(current) = &state.current else {
                return;
            };
            if state.policy.bound() != Some(current.identity)
                || state.rendered != Some(current.identity)
                || current.page != PanePageKind::Terminal
            {
                return;
            }
            let Some(request) = &state.request else {
                return;
            };
            (current.clone(), request.restore.clone())
        };
        let (current, restore) = request;
        let target = match &restore {
            Restore::Action(tooltip) => self
                .content
                .upgrade()
                .and_then(|c| descendant(c.upcast_ref(), "active-pane"))
                .and_then(|pane| watch::action(&pane, tooltip)),
            restore => current
                .terminal
                .as_ref()
                .and_then(|w| w.upgrade())
                .and_then(|terminal| {
                    if let Restore::Descendant(weak) = restore
                        && let Some(widget) = weak.upgrade()
                        && owns(&widget, &terminal)
                    {
                        return Some(widget);
                    }
                    descendant(&terminal, "terminal-canvas")
                }),
        };
        let Some(target) = target else {
            return;
        };
        let Some(content) = self.content.upgrade() else {
            self.invalidate();
            return;
        };
        if !target.is_mapped()
            || !target.is_visible()
            || !target.is_sensitive()
            || target.width() <= 0
            || target.height() <= 0
            || target.root() != content.root()
        {
            return;
        }
        self.invalidate(); // Consume and disconnect before grab_focus can synchronously notify.
        target.grab_focus();
    }
}
impl PaneHost {
    pub(crate) fn prepare_new_local_focus(&self) -> PaneFocus {
        self.focus.clone()
    }
    pub(crate) fn focus_handle(&self) -> PaneFocus {
        self.focus.clone()
    }
}
