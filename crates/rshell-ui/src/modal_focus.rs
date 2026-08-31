use gtk::prelude::*;
use relm4::gtk;

pub struct ModalFocusSession {
    trigger: gtk::glib::WeakRef<gtk::Widget>,
    fallback: gtk::glib::WeakRef<gtk::Widget>,
    first: gtk::glib::WeakRef<gtk::Widget>,
    last: gtk::glib::WeakRef<gtk::Widget>,
    surface: gtk::glib::WeakRef<gtk::Widget>,
}

impl ModalFocusSession {
    pub(crate) fn new(
        trigger: &gtk::Widget,
        fallback: &gtk::Widget,
        surface: &gtk::Widget,
    ) -> Self {
        let session = Self {
            trigger: trigger.downgrade(),
            fallback: fallback.downgrade(),
            first: gtk::glib::WeakRef::new(),
            last: gtk::glib::WeakRef::new(),
            surface: surface.downgrade(),
        };
        session.refresh_targets();
        session
    }

    pub fn contain_tab(&self, backwards: bool) -> gtk::glib::Propagation {
        self.refresh_targets();
        let Some(first) = self.first.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        let Some(last) = self.last.upgrade() else {
            return gtk::glib::Propagation::Proceed;
        };
        let focused = first
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root));
        let boundary = if backwards { &first } else { &last };
        if !focused
            .as_ref()
            .is_some_and(|focused| is_same_or_descendant(focused, boundary))
        {
            return gtk::glib::Propagation::Proceed;
        }
        if backwards {
            last.grab_focus();
        } else {
            first.grab_focus();
        }
        gtk::glib::Propagation::Stop
    }

    pub fn restore(self) {
        let target = self
            .trigger
            .upgrade()
            .filter(is_live_focus_target)
            .or_else(|| self.fallback.upgrade().filter(is_live_focus_target));
        if let Some(target) = target {
            target.grab_focus();
            gtk::glib::idle_add_local_once(move || {
                if is_live_focus_target(&target) {
                    target.grab_focus();
                }
            });
        }
    }

    pub(crate) fn focus_first(&self) {
        self.refresh_targets();
        if let Some(first) = self.first.upgrade() {
            first.grab_focus();
        }
    }

    fn refresh_targets(&self) {
        let Some(surface) = self.surface.upgrade() else {
            return;
        };
        let targets = focusable_descendants(&surface);
        let first =
            find_css_descendant(&surface, "modal-focus-first").or_else(|| targets.first().cloned());
        let last =
            find_css_descendant(&surface, "modal-focus-last").or_else(|| targets.last().cloned());
        self.first.set(first.as_ref());
        self.last.set(last.as_ref());
    }
}

impl Clone for ModalFocusSession {
    fn clone(&self) -> Self {
        Self {
            trigger: clone_weak(&self.trigger),
            fallback: clone_weak(&self.fallback),
            first: clone_weak(&self.first),
            last: clone_weak(&self.last),
            surface: clone_weak(&self.surface),
        }
    }
}

fn focusable_descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    fn collect(widget: &gtk::Widget, targets: &mut Vec<gtk::Widget>) {
        if is_native_control(widget) {
            targets.push(widget.clone());
            return;
        }
        if let Ok(scroll) = widget.clone().downcast::<gtk::ScrolledWindow>() {
            if let Some(child) = scroll.child() {
                collect(&child, targets);
            }
            return;
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            collect(&current, targets);
            child = current.next_sibling();
        }
    }
    let mut targets = Vec::new();
    collect(root, &mut targets);
    targets
}

fn is_native_control(widget: &gtk::Widget) -> bool {
    widget.is_mapped()
        && widget.is_sensitive()
        && (widget.is::<gtk::Button>()
            || widget.is::<gtk::CheckButton>()
            || widget.is::<gtk::DropDown>()
            || widget.is::<gtk::Entry>()
            || widget.is::<gtk::PasswordEntry>()
            || widget.is::<gtk::SpinButton>()
            || widget.is::<gtk::TextView>())
}

fn find_css_descendant(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(current) = child {
        if current.is_mapped() && current.is_sensitive() && current.has_css_class(class) {
            return Some(current);
        }
        if let Some(found) = find_css_descendant(&current, class) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn is_same_or_descendant(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget == *ancestor {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn is_live_focus_target(widget: &gtk::Widget) -> bool {
    widget.root().is_some() && widget.is_mapped() && widget.is_sensitive() && widget.is_focusable()
}

fn clone_weak(source: &gtk::glib::WeakRef<gtk::Widget>) -> gtk::glib::WeakRef<gtk::Widget> {
    let weak = gtk::glib::WeakRef::new();
    if let Some(widget) = source.upgrade() {
        weak.set(Some(&widget));
    }
    weak
}
