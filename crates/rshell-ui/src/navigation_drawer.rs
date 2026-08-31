use gtk::prelude::*;
use relm4::{ComponentController, ComponentSender, gtk};

use crate::{ConnectionSidebarMsg, IconRenderRequest, MainWindow, MainWindowMsg, ProductIcon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationAction {
    Toggle,
    Close,
    NewConnection,
    NewGroup,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NavigationDrawerState {
    pub open: bool,
}

pub(crate) struct NavigationDrawer {
    state: NavigationDrawerState,
    rail: gtk::Box,
    navigation: gtk::Button,
}

impl NavigationDrawer {
    pub(crate) fn new(sidebar: &gtk::Widget, sender: &ComponentSender<MainWindow>) -> Self {
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 4);
        rail.add_css_class("compact-nav-rail");
        rail.set_width_request(48);
        rail.set_visible(false);
        let request = IconRenderRequest::for_widget(16, &rail);
        let navigation = ProductIcon::Navigation
            .button(Some("Navigation"), request)
            .expect("embedded navigation icon");
        let create = ProductIcon::AddConnection
            .button(Some("New connection"), request)
            .expect("embedded new-connection icon");
        let group = ProductIcon::AddGroup
            .button(Some("New group"), request)
            .expect("embedded new-group icon");
        navigation.connect_clicked(send(sender, NavigationAction::Toggle));
        create.connect_clicked(send(sender, NavigationAction::NewConnection));
        group.connect_clicked(send(sender, NavigationAction::NewGroup));
        rail.append(&navigation);
        rail.append(&create);
        rail.append(&group);
        install_escape(sidebar, sender);
        Self {
            state: NavigationDrawerState::default(),
            rail,
            navigation,
        }
    }

    pub(crate) fn rail(&self) -> &gtk::Box {
        &self.rail
    }

    pub(crate) fn set_compact(
        &mut self,
        compact: bool,
        preserve_sidebar_focus: bool,
        sidebar: &gtk::Widget,
    ) {
        self.rail.set_visible(compact);
        if compact {
            self.state.open = self.state.open || preserve_sidebar_focus;
            sidebar.add_css_class("navigation-drawer");
            sidebar.set_width_request(280);
        } else {
            self.state.close();
            sidebar.remove_css_class("navigation-drawer");
        }
        self.sync_sidebar(sidebar);
    }

    pub(crate) fn toggle(&mut self, sidebar: &gtk::Widget) {
        self.state.toggle();
        self.sync_sidebar(sidebar);
        if self.state.open {
            focus_search(sidebar);
        }
    }

    pub(crate) fn close(&mut self, sidebar: &gtk::Widget) {
        if !self.state.open {
            return;
        }
        self.state.close();
        self.sync_sidebar(sidebar);
        let navigation = self.navigation.clone();
        gtk::glib::idle_add_local_once(move || {
            navigation.grab_focus();
        });
    }

    fn sync_sidebar(&self, sidebar: &gtk::Widget) {
        let compact = self.rail.is_visible();
        sidebar.set_visible(!compact || self.state.open);
        if compact && self.state.open {
            sidebar.add_css_class("navigation-drawer-open");
        } else {
            sidebar.remove_css_class("navigation-drawer-open");
        }
    }
}

fn install_escape(sidebar: &gtk::Widget, sender: &ComponentSender<MainWindow>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sidebar_root = sidebar.clone();
    let sidebar_state = sidebar_root.clone();
    let input = sender.input_sender().clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape && sidebar_state.has_css_class("navigation-drawer-open") {
            let _ = input.send(MainWindowMsg::Navigation(NavigationAction::Close));
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    sidebar_root.add_controller(keys);
}

fn focus_search(sidebar: &gtk::Widget) {
    if let Some(search) = find_css(sidebar, "connection-search") {
        gtk::glib::idle_add_local_once(move || {
            search.grab_focus();
        });
    }
}

fn find_css(widget: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    let mut child = widget.first_child();
    while let Some(current) = child {
        if current.has_css_class(class) {
            return Some(current);
        }
        if let Some(found) = find_css(&current, class) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

fn send(
    sender: &ComponentSender<MainWindow>,
    action: NavigationAction,
) -> impl Fn(&gtk::Button) + 'static {
    let sender = sender.clone();
    move |_| sender.input(MainWindowMsg::Navigation(action))
}

impl MainWindow {
    pub(crate) fn handle_navigation(&mut self, action: NavigationAction) {
        match action {
            NavigationAction::Toggle => self
                .shell
                .toggle_navigation_drawer(self.sidebar.widget().upcast_ref()),
            NavigationAction::Close => self
                .shell
                .close_navigation_drawer(self.sidebar.widget().upcast_ref()),
            NavigationAction::NewConnection => {
                self.send_sidebar(ConnectionSidebarMsg::CreateConnection)
            }
            NavigationAction::NewGroup => self.send_sidebar(ConnectionSidebarMsg::CreateGroup),
        }
    }
}

impl NavigationDrawerState {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}
