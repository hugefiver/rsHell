use super::{PaneFocus, Restore, policy::Identity, watch::Origin};
use crate::PanePageKind;
use gtk::prelude::*;
use relm4::gtk;
use rshell_core::{AppViewModel, PaneLaunchTarget};

pub(super) struct StartupAnchor {
    widget: gtk::glib::WeakRef<gtk::Overlay>,
    focusable: bool,
}

impl Drop for StartupAnchor {
    fn drop(&mut self) {
        if let Some(widget) = self.widget.upgrade() {
            widget.set_focusable(self.focusable);
        }
    }
}

impl PaneFocus {
    pub(crate) fn authorize_startup(&self, root: &gtk::ApplicationWindow, view: &AppViewModel) {
        let Some(identity) = initial_local(view) else {
            return;
        };
        let Some(content) = self.content.upgrade() else {
            return;
        };
        if root.is_mapped()
            || gtk::prelude::RootExt::focus(root).is_some()
            || content.root().as_ref() != Some(root.upcast_ref())
            || !content.is_sensitive()
            || self.state.borrow().current.as_ref().is_none_or(|c| {
                c.identity != identity
                    || !matches!(c.page, PanePageKind::Pending | PanePageKind::Terminal)
            })
        {
            return;
        }
        let anchor = StartupAnchor {
            widget: content.downgrade(),
            focusable: content.is_focusable(),
        };
        content.set_focusable(true);
        gtk::prelude::GtkWindowExt::set_focus(root, Some(&content));
        if gtk::prelude::RootExt::focus(root).as_ref() != Some(content.upcast_ref()) {
            return;
        }
        self.state.borrow_mut().policy.activate(identity);
        self.install(
            Origin {
                root: root.upcast_ref::<gtk::Window>().downgrade(),
                widget: content.upcast_ref::<gtk::Widget>().downgrade(),
                tab_action: false,
            },
            Restore::Canvas,
        );
        if let Some(request) = self.state.borrow_mut().request.as_mut() {
            request.startup = Some(anchor);
        }
        self.watch_readiness();
    }

    pub(super) fn check_startup_origin(&self, content: &gtk::Overlay) -> Option<bool> {
        let origin = {
            let state = self.state.borrow();
            let request = state.request.as_ref()?;
            request.startup.as_ref()?;
            request.origin.clone()
        };
        let valid = origin.root.upgrade().is_some_and(|root| {
            content.root().as_ref() == Some(root.upcast_ref())
                && content.is_sensitive()
                // Startup allows the unmapped constructor phase, but never descendants.
                && gtk::prelude::RootExt::focus(&root).as_ref() == Some(content.upcast_ref())
        });
        Some(valid)
    }
}

fn initial_local(view: &AppViewModel) -> Option<Identity> {
    let tab = view.workspace.active_tab()?;
    let pane = tab.active_pane;
    if view.pane_launches.get(&pane) != Some(&PaneLaunchTarget::Local) {
        return None;
    }
    let session = tab.pane_tree.session_id(pane).ok()??;
    Some(Identity {
        tab: tab.id,
        pane,
        session,
    })
}
