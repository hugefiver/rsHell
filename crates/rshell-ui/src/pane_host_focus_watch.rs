use super::PaneFocus;
use gtk::prelude::*;
use relm4::gtk;
use rshell_core::TabId;

impl PaneFocus {
    pub(super) fn check_origin(&self) -> bool {
        let Some(content) = self.content.upgrade() else {
            self.invalidate();
            return false;
        };
        if let Some(valid) = self.check_startup_origin(&content) {
            if !valid {
                self.invalidate();
            }
            return valid;
        }
        let snapshot = self
            .state
            .borrow()
            .request
            .as_ref()
            .map(|r| (r.origin.clone(), r.fallback.clone()));
        let Some((origin, fallback)) = snapshot else {
            return false;
        };
        let Some(focus) = origin.current_focus(&content) else {
            self.invalidate();
            return false;
        };
        let (ui_tab, bound_tab) = self.tab_source();
        if let Some(replacement) = tab_replacement(&origin, focus.as_ref(), ui_tab, bound_tab) {
            if let Some(request) = self.state.borrow_mut().request.as_mut() {
                request.origin.widget = replacement;
            }
            return true;
        }
        let valid = origin.owns_focus(focus.as_ref())
            || fallback.is_some_and(|old| match old {
                Some(old) => old.upgrade().is_some_and(|w| focus.as_ref() == Some(&w)),
                None => focus.is_none(),
            });
        let reparenting = self.state.borrow().mechanical;
        let mechanical =
            (reparenting || self.owned_layout_loss()) && mechanical_focus(focus.as_ref(), &content);
        let fallback = Some(focus.as_ref().map(|w| w.downgrade()));
        if mechanical
            && !reparenting
            && let Some(request) = self.state.borrow_mut().request.as_mut()
        {
            request.fallback = fallback;
        }
        if !valid && !mechanical {
            self.invalidate();
        }
        valid || mechanical
    }
}

#[derive(Clone)]
pub(super) struct Origin {
    pub root: gtk::glib::WeakRef<gtk::Window>,
    pub widget: gtk::glib::WeakRef<gtk::Widget>,
    pub tab_action: bool,
}

pub(super) fn owns(widget: &gtk::Widget, owner: &gtk::Widget) -> bool {
    widget == owner || widget.is_ancestor(owner)
}

pub(super) fn mechanical_focus(focus: Option<&gtk::Widget>, content: &gtk::Overlay) -> bool {
    focus.is_none_or(|widget| {
        widget.is::<gtk::Paned>()
            && (owns(widget, content.upcast_ref()) || owns(content.upcast_ref(), widget))
    })
}

pub(super) fn origin(content: &gtk::Overlay, new: bool) -> Option<Origin> {
    if !content.is_mapped() || !content.is_sensitive() {
        return None;
    }
    let root = content.root()?.downcast::<gtk::Window>().ok()?;
    let mut focus = gtk::prelude::RootExt::focus(&root)?;
    // An entry's focused GtkText belongs to the entry domain, not only its leaf.
    if let Some(parent) = focus.parent()
        && (parent.is::<gtk::Entry>() || parent.is::<gtk::SearchEntry>())
    {
        focus = parent;
    }
    let eligible = focus.has_css_class("terminal-canvas")
        || focus.has_css_class("terminal-search")
        || if new {
            focus.has_css_class("tab-add")
                || focus.tooltip_text().as_deref() == Some("New local terminal tab")
        } else {
            focus.has_css_class("tab-button")
                || focus.has_css_class("tab-overflow-row")
                || focus.has_css_class("tab-add")
                || focus.tooltip_text().as_deref() == Some("New local terminal tab")
        };
    eligible.then(|| Origin {
        root: root.downgrade(),
        widget: focus.downgrade(),
        tab_action: !new
            && (focus.has_css_class("tab-button") || focus.has_css_class("tab-overflow-row")),
    })
}

pub(super) fn tab_replacement(
    origin: &Origin,
    focus: Option<&gtk::Widget>,
    ui_tab: Option<TabId>,
    bound_tab: Option<TabId>,
) -> Option<gtk::glib::WeakRef<gtk::Widget>> {
    if !origin.tab_action || bound_tab.is_none() || ui_tab != bound_tab {
        return None;
    }
    let root = origin.root.upgrade()?;
    if origin
        .widget
        .upgrade()
        .is_some_and(|old| old.is_mapped() && old.root().as_ref() == Some(root.upcast_ref()))
    {
        return None;
    }
    let focus = focus?;
    (focus.is_mapped()
        && focus.is_sensitive()
        && focus.root().as_ref() == Some(root.upcast_ref())
        && focus.has_css_class("active-tab")
        && (focus.has_css_class("tab-button") || focus.has_css_class("tab-overflow-row")))
    .then(|| focus.downgrade())
}

impl Origin {
    pub fn current_focus(&self, content: &gtk::Overlay) -> Option<Option<gtk::Widget>> {
        let root = self.root.upgrade()?;
        if !root.is_mapped() || content.root().as_ref() != Some(root.upcast_ref()) {
            return None;
        }
        Some(gtk::prelude::RootExt::focus(&root))
    }
    pub fn owns_focus(&self, focus: Option<&gtk::Widget>) -> bool {
        self.widget
            .upgrade()
            .is_some_and(|owner| focus.is_some_and(|w| owns(w, &owner)))
    }
}

#[derive(Default)]
pub(super) struct Connections(
    Vec<(
        gtk::glib::WeakRef<gtk::glib::Object>,
        gtk::glib::SignalHandlerId,
    )>,
);
impl Connections {
    pub fn push(
        &mut self,
        object: &impl IsA<gtk::glib::Object>,
        handler: gtk::glib::SignalHandlerId,
    ) {
        self.0.push((object.as_ref().downgrade(), handler));
    }
}
impl Drop for Connections {
    fn drop(&mut self) {
        for (object, handler) in self.0.drain(..) {
            if let Some(object) = object.upgrade() {
                object.disconnect(handler);
            }
        }
    }
}

pub(super) fn descendant(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.has_css_class(class) {
            return Some(widget);
        }
        if let Some(found) = descendant(&widget, class) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

pub(super) fn action(root: &gtk::Widget, tooltip: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(widget) = child {
        if widget.is::<gtk::Button>()
            && widget.is_mapped()
            && widget.is_sensitive()
            && widget.tooltip_text().as_deref() == Some(tooltip)
        {
            return Some(widget);
        }
        if let Some(found) = action(&widget, tooltip) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}
