use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{MainWindow, ShellLayout, ShellLayoutMode, navigation_drawer::NavigationDrawer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellChildOwner {
    Detached,
    WorkspacePanedStart,
    DrawerOverlay,
}

pub struct MainWindowShell {
    pub overlay: gtk::Overlay,
    pub background: gtk::Box,
    pub command_status: gtk::Label,
    pub navigation_host: gtk::Box,
    pub terminal_workspace: gtk::Box,
    pub workspace_paned: gtk::Paned,
    pub drawer_overlay: gtk::Overlay,
    navigation: NavigationDrawer,
    sidebar_owner: ShellChildOwner,
    current: ShellLayout,
    stored_navigation_width: i32,
    presented_navigation_width: i32,
}

impl MainWindowShell {
    pub(crate) fn new(
        command_bar: &gtk::Box,
        sidebar: &gtk::Widget,
        tab_bar: &gtk::Widget,
        pane_host: &gtk::Widget,
        sender: &ComponentSender<MainWindow>,
    ) -> Self {
        let command_status = gtk::Label::new(Some("Ready"));
        command_status.add_css_class("command-status");
        command_status.set_halign(gtk::Align::End);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        command_bar.append(&spacer);
        command_bar.append(&command_status);

        let terminal_workspace = gtk::Box::new(gtk::Orientation::Vertical, 0);
        terminal_workspace.set_hexpand(true);
        terminal_workspace.set_vexpand(true);
        terminal_workspace.append(tab_bar);
        terminal_workspace.append(pane_host);
        let workspace = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        workspace.set_hexpand(true);
        workspace.set_vexpand(true);
        workspace.append(&terminal_workspace);

        let workspace_paned = gtk::Paned::new(gtk::Orientation::Horizontal);
        workspace_paned.set_start_child(Some(sidebar));
        workspace_paned.set_end_child(Some(&workspace));
        workspace_paned.set_position(260);
        workspace_paned.set_resize_start_child(true);
        workspace_paned.set_shrink_start_child(true);
        workspace_paned.set_hexpand(true);
        workspace_paned.set_vexpand(true);
        let drawer_overlay = gtk::Overlay::new();
        drawer_overlay.set_child(Some(&workspace_paned));
        drawer_overlay.set_hexpand(true);
        drawer_overlay.set_vexpand(true);
        let navigation = NavigationDrawer::new(sidebar, sender);
        let navigation_host = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        navigation_host.set_hexpand(true);
        navigation_host.set_vexpand(true);
        navigation_host.append(navigation.rail());
        navigation_host.append(&drawer_overlay);

        let background = gtk::Box::new(gtk::Orientation::Vertical, 0);
        background.add_css_class("modal-background");
        background.append(command_bar);
        background.append(&navigation_host);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&background));
        Self {
            overlay,
            background,
            command_status,
            navigation_host,
            terminal_workspace,
            workspace_paned,
            drawer_overlay,
            navigation,
            sidebar_owner: ShellChildOwner::WorkspacePanedStart,
            current: ShellLayout::for_width(1_360),
            stored_navigation_width: 260,
            presented_navigation_width: 260,
        }
    }

    fn detach_sidebar(&mut self, sidebar: &gtk::Widget) -> bool {
        match self.sidebar_owner {
            ShellChildOwner::Detached => return true,
            ShellChildOwner::WorkspacePanedStart => {
                if self.workspace_paned.start_child().as_ref() != Some(sidebar) {
                    return false;
                }
                self.workspace_paned.set_start_child(gtk::Widget::NONE);
            }
            ShellChildOwner::DrawerOverlay => {
                if sidebar.parent().as_ref() != Some(self.drawer_overlay.upcast_ref()) {
                    return false;
                }
                self.drawer_overlay.remove_overlay(sidebar);
            }
        }
        self.sidebar_owner = ShellChildOwner::Detached;
        true
    }

    fn attach_sidebar(&mut self, owner: ShellChildOwner, sidebar: &gtk::Widget) -> bool {
        if self.sidebar_owner != ShellChildOwner::Detached || sidebar.parent().is_some() {
            return false;
        }
        match owner {
            ShellChildOwner::Detached => return false,
            ShellChildOwner::WorkspacePanedStart => {
                self.workspace_paned.set_start_child(Some(sidebar));
            }
            ShellChildOwner::DrawerOverlay => self.drawer_overlay.add_overlay(sidebar),
        }
        self.sidebar_owner = owner;
        true
    }

    pub fn apply(&mut self, layout: ShellLayout, sidebar: &gtk::Widget) {
        let focused = sidebar
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
            .filter(|focused| is_within(focused, sidebar));
        if self.current.mode != ShellLayoutMode::Compact {
            let position = self.workspace_paned.position();
            if position > 0 && position != self.presented_navigation_width {
                self.stored_navigation_width = position;
            }
        }
        let owner = if layout.sidebar_overlay {
            ShellChildOwner::DrawerOverlay
        } else {
            ShellChildOwner::WorkspacePanedStart
        };
        if self.sidebar_owner != owner
            && (!self.detach_sidebar(sidebar) || !self.attach_sidebar(owner, sidebar))
        {
            return;
        }

        self.apply_mode_classes(layout.mode);
        self.set_action_text_visible(layout.text_global_actions);
        self.navigation.set_compact(
            layout.mode == ShellLayoutMode::Compact,
            focused.is_some(),
            sidebar,
        );
        if layout.mode == ShellLayoutMode::Compact {
            sidebar.set_halign(gtk::Align::Start);
        } else {
            sidebar.set_halign(gtk::Align::Fill);
            let width = if layout.mode == ShellLayoutMode::Wide {
                self.stored_navigation_width.min(layout.navigation_width)
            } else {
                self.stored_navigation_width
            };
            sidebar.set_width_request(width);
            self.workspace_paned.set_position(width);
            self.presented_navigation_width = width;
        }
        self.current = layout;
        if let Some(focused) = focused {
            gtk::glib::idle_add_local_once(move || {
                focused.grab_focus();
            });
        }
    }

    pub fn set_status(&self, text: &str) {
        self.command_status.set_label(text);
    }

    pub fn layout(&self) -> ShellLayout {
        self.current
    }

    pub(crate) fn toggle_navigation_drawer(&mut self, sidebar: &gtk::Widget) {
        if self.current.mode == ShellLayoutMode::Compact {
            self.navigation.toggle(sidebar);
        }
    }

    pub(crate) fn close_navigation_drawer(&mut self, sidebar: &gtk::Widget) {
        self.navigation.close(sidebar);
    }

    fn apply_mode_classes(&self, mode: ShellLayoutMode) {
        for class in ["shell-compact", "shell-standard", "shell-wide"] {
            self.background.remove_css_class(class);
        }
        self.background.add_css_class(match mode {
            ShellLayoutMode::Compact => "shell-compact",
            ShellLayoutMode::Standard => "shell-standard",
            ShellLayoutMode::Wide => "shell-wide",
        });
    }

    fn set_action_text_visible(&self, visible: bool) {
        visit_children(self.background.upcast_ref(), &mut |widget| {
            if widget.has_css_class("global-action-label")
                || widget.has_css_class("navigation-action-label")
            {
                widget.set_visible(visible);
            }
        });
    }
}

fn visit_children(widget: &gtk::Widget, visit: &mut impl FnMut(&gtk::Widget)) {
    let mut child = widget.first_child();
    while let Some(current) = child {
        visit(&current);
        visit_children(&current, visit);
        child = current.next_sibling();
    }
}

fn is_within(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget == *ancestor {
            return true;
        }
        current = widget.parent();
    }
    false
}
