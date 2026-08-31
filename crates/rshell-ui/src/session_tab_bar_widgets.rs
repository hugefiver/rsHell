use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::{TabId, WorkspaceState};

use crate::{IconRenderRequest, ProductIcon, SessionTabBar, SessionTabBarMsg, TabOverflowModel};

pub struct SessionTabBarWidgets {
    tabs: gtk::Box,
    scroll: gtk::ScrolledWindow,
    overflow_rows: gtk::Box,
    overflow_popover: gtk::Popover,
    overflow: gtk::MenuButton,
    error: gtk::Label,
}

impl SessionTabBarWidgets {
    pub(crate) fn build(root: &gtk::Box, sender: &ComponentSender<SessionTabBar>) -> Self {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .hexpand(true)
            .child(&tabs)
            .build();
        scroll.add_css_class("tab-strip-scroll");
        row.append(&scroll);

        let overflow_rows = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let overflow_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .max_content_height(360)
            .propagate_natural_height(true)
            .child(&overflow_rows)
            .build();
        let overflow_popover = gtk::Popover::builder().child(&overflow_scroll).build();
        overflow_popover.add_css_class("tab-overflow");
        let overflow = gtk::MenuButton::new();
        overflow.add_css_class("tab-overflow");
        overflow.set_tooltip_text(Some("Show all tabs"));
        set_accessible_label(&overflow, "Show all tabs");
        overflow.set_child(Some(
            &ProductIcon::More
                .image(IconRenderRequest::for_widget(16, &overflow))
                .expect("embedded tab-overflow icon"),
        ));
        overflow.set_popover(Some(&overflow_popover));
        row.append(&overflow);

        let add = ProductIcon::NewTab
            .button(
                Some("New local terminal tab"),
                IconRenderRequest::for_widget(16, root),
            )
            .expect("embedded new-tab icon");
        add.add_css_class("tab-add");
        add.connect_clicked(send(sender, SessionTabBarMsg::NewLocalTab));
        row.append(&add);
        root.append(&row);

        let error = gtk::Label::new(None);
        error.add_css_class("pane-state-label");
        error.set_halign(gtk::Align::Start);
        error.set_visible(false);
        root.append(&error);
        install_shortcuts(root, sender);
        Self {
            tabs,
            scroll,
            overflow_rows,
            overflow_popover,
            overflow,
            error,
        }
    }

    pub(crate) fn render(
        &mut self,
        workspace: &WorkspaceState,
        active: Option<TabId>,
        error: Option<&str>,
        sender: &ComponentSender<SessionTabBar>,
    ) {
        clear_box(&self.tabs);
        clear_box(&self.overflow_rows);
        let active_index = active.and_then(|id| workspace.tabs.iter().position(|tab| tab.id == id));
        let capacity = (usize::try_from(self.scroll.width()).unwrap_or(0) / 144).clamp(4, 10);
        let visible_indices = visible_tab_indices(workspace.tabs.len(), active_index, capacity);
        let overflow = TabOverflowModel::new(workspace.tabs.len(), active_index, &visible_indices);
        let mut active_group = None;
        for (index, tab) in workspace.tabs.iter().enumerate() {
            if visible_indices.contains(&index) {
                let group = self.tab_group(tab.id, &tab.title, active == Some(tab.id), sender);
                if active == Some(tab.id) {
                    active_group = Some(group.clone());
                }
                self.tabs.append(&group);
            }
            if overflow.overflow_indices.contains(&index) {
                self.overflow_rows.append(&self.overflow_row(
                    tab.id,
                    &tab.title,
                    active == Some(tab.id),
                    sender,
                ));
            }
        }
        self.overflow
            .set_visible(!overflow.overflow_indices.is_empty());
        self.error.set_label(error.unwrap_or(""));
        self.error.set_visible(error.is_some());
        if let Some(group) = active_group {
            reveal_after_allocation(&self.scroll, &group);
        }
    }

    fn tab_group(
        &self,
        tab: TabId,
        title: &str,
        active: bool,
        sender: &ComponentSender<SessionTabBar>,
    ) -> gtk::Box {
        let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        group.add_css_class("terminal-tab");
        let label = format!("Activate {title} tab");
        let activate = gtk::Button::with_label(title);
        activate.add_css_class("tab-button");
        activate.set_tooltip_text(Some(&label));
        set_accessible_label(&activate, &label);
        if active {
            activate.add_css_class("active-tab");
        }
        activate.connect_clicked(send(sender, SessionTabBarMsg::Activate(tab)));
        group.append(&activate);
        let close_label = format!("Close {title} tab");
        let close = ProductIcon::CloseTab
            .button(
                Some(&close_label),
                IconRenderRequest::for_widget(16, &self.tabs),
            )
            .expect("embedded close-tab icon");
        close.add_css_class("tab-close");
        close.connect_clicked(send(sender, SessionTabBarMsg::Close(tab)));
        group.append(&close);
        group
    }

    fn overflow_row(
        &self,
        tab: TabId,
        title: &str,
        active: bool,
        sender: &ComponentSender<SessionTabBar>,
    ) -> gtk::Button {
        let row = gtk::Button::with_label(title);
        row.add_css_class("tab-overflow-row");
        let label = format!("Activate {title} tab from overflow");
        row.set_tooltip_text(Some(&label));
        set_accessible_label(&row, &label);
        if active {
            row.add_css_class("active-tab");
        }
        let input = sender.input_sender().clone();
        let popover = self.overflow_popover.clone();
        row.connect_clicked(move |_| {
            let _ = input.send(SessionTabBarMsg::ActivateFromOverflow(tab));
            popover.popdown();
        });
        row
    }
}

fn visible_tab_indices(tab_count: usize, active: Option<usize>, capacity: usize) -> Vec<usize> {
    let mut visible = (0..tab_count.min(capacity)).collect::<Vec<_>>();
    if let Some(active) = active.filter(|index| *index < tab_count)
        && !visible.contains(&active)
        && !visible.is_empty()
    {
        visible.pop();
        visible.push(active);
        visible.sort_unstable();
    }
    visible
}

fn install_shortcuts(root: &gtk::Box, sender: &ComponentSender<SessionTabBar>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let input = sender.input_sender().clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        if key == gtk::gdk::Key::Tab && state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            let delta = if state.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                -1
            } else {
                1
            };
            let _ = input.send(SessionTabBarMsg::Cycle(delta));
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    root.add_controller(keys);
}

fn reveal_after_allocation(scroll: &gtk::ScrolledWindow, group: &gtk::Box) {
    let scroll = scroll.clone();
    let group = group.clone();
    gtk::glib::idle_add_local_once(move || {
        let adjustment = scroll.hadjustment();
        let start = f64::from(group.allocation().x());
        let end = start + f64::from(group.width());
        let value = adjustment.value();
        let page_end = value + adjustment.page_size();
        if start < value {
            adjustment.set_value(start);
        } else if end > page_end {
            adjustment.set_value(end - adjustment.page_size());
        }
    });
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn set_accessible_label(widget: &impl IsA<gtk::Accessible>, label: &str) {
    widget.update_property(&[gtk::accessible::Property::Label(label)]);
}

fn send(
    sender: &ComponentSender<SessionTabBar>,
    message: SessionTabBarMsg,
) -> impl Fn(&gtk::Button) + 'static {
    let sender = sender.clone();
    move |_| sender.input(message.clone())
}

#[cfg(test)]
mod tests {
    use super::visible_tab_indices;

    #[test]
    fn visible_tabs_are_bounded_and_retain_the_active_tab() {
        assert_eq!(visible_tab_indices(20, Some(19), 4), vec![0, 1, 2, 19]);
        assert_eq!(visible_tab_indices(2, Some(1), 4), vec![0, 1]);
    }
}
