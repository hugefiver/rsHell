use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::{TabId, UiCommand, UiPortError, WorkspaceState};

use crate::ProductIcon;

#[derive(Debug, Clone)]
pub struct SessionTabBarInit {
    pub workspace: WorkspaceState,
}

#[derive(Debug, Clone)]
pub enum SessionTabBarMsg {
    SetWorkspace(WorkspaceState),
    Activate(TabId),
    NewLocalTab,
    Close(TabId),
    CommandRejected(UiPortError),
}

#[derive(Debug)]
pub enum SessionTabBarOutput {
    Command(Box<UiCommand>),
    ActivateTab(TabId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTabBarAction {
    NewLocalTab,
    Close(TabId),
}

impl SessionTabBarAction {
    pub fn command(self) -> UiCommand {
        match self {
            Self::NewLocalTab => UiCommand::NewLocalTab,
            Self::Close(tab) => UiCommand::CloseTab(tab),
        }
    }
}

pub struct SessionTabBar {
    workspace: WorkspaceState,
    active: Option<TabId>,
    error: Option<String>,
}

pub struct SessionTabBarWidgets {
    tabs: gtk::Box,
    error: gtk::Label,
}

impl SimpleComponent for SessionTabBar {
    type Init = SessionTabBarInit;
    type Input = SessionTabBarMsg;
    type Output = SessionTabBarOutput;
    type Root = gtk::Box;
    type Widgets = SessionTabBarWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("tab-bar");
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let error = gtk::Label::new(None);
        error.add_css_class("pane-state-label");
        error.set_halign(gtk::Align::Start);
        error.set_visible(false);
        root.append(&tabs);
        root.append(&error);
        let active = init.workspace.active_tab;
        let model = Self {
            workspace: init.workspace,
            active,
            error: None,
        };
        let mut widgets = SessionTabBarWidgets { tabs, error };
        model.render(&mut widgets, &sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            SessionTabBarMsg::SetWorkspace(workspace) => {
                let incoming = workspace.active_tab;
                let incoming_is_new = incoming.is_some_and(|tab| self.workspace.tab(tab).is_err());
                let local_was_removed = self
                    .active
                    .is_none_or(|active| workspace.tab(active).is_err());
                if incoming_is_new || local_was_removed {
                    self.active = incoming;
                }
                self.workspace = workspace;
                self.error = None;
            }
            SessionTabBarMsg::Activate(tab) => {
                if self.workspace.tab(tab).is_ok() {
                    self.active = Some(tab);
                    let _ = sender.output(SessionTabBarOutput::ActivateTab(tab));
                }
            }
            SessionTabBarMsg::NewLocalTab => {
                let _ = sender.output(SessionTabBarOutput::Command(Box::new(
                    SessionTabBarAction::NewLocalTab.command(),
                )));
            }
            SessionTabBarMsg::Close(tab) => {
                if self.workspace.tab(tab).is_ok() {
                    let _ = sender.output(SessionTabBarOutput::Command(Box::new(
                        SessionTabBarAction::Close(tab).command(),
                    )));
                }
            }
            SessionTabBarMsg::CommandRejected(error) => self.error = Some(error.to_string()),
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        self.render(widgets, &sender);
    }
}

impl SessionTabBar {
    fn render(&self, widgets: &mut SessionTabBarWidgets, sender: &ComponentSender<Self>) {
        clear_box(&widgets.tabs);
        for tab in &self.workspace.tabs {
            let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            group.add_css_class("terminal-tab");
            let activate = gtk::Button::with_label(&tab.title);
            activate.add_css_class("tab-button");
            activate.set_tooltip_text(Some(&format!("Activate {} tab", tab.title)));
            set_accessible_label(&activate, &format!("Activate {} tab", tab.title));
            if self.active == Some(tab.id) {
                activate.add_css_class("active-tab");
            }
            let tab_id = tab.id;
            let input = sender.input_sender().clone();
            activate.connect_clicked(move |_| {
                let _ = input.send(SessionTabBarMsg::Activate(tab_id));
            });
            group.append(&activate);

            let close_label = format!("Close {} tab", tab.title);
            let close = ProductIcon::CloseTab
                .button(Some(&close_label))
                .expect("embedded close-tab icon");
            close.add_css_class("tab-close");
            set_accessible_label(&close, &close_label);
            let input = sender.input_sender().clone();
            close.connect_clicked(move |_| {
                let _ = input.send(SessionTabBarMsg::Close(tab_id));
            });
            group.append(&close);
            widgets.tabs.append(&group);
        }

        let add = ProductIcon::NewTab
            .button(Some("New local terminal tab"))
            .expect("embedded new-tab icon");
        add.add_css_class("tab-add");
        add.set_tooltip_text(Some("New local terminal tab"));
        set_accessible_label(&add, "New local terminal tab");
        let input = sender.input_sender().clone();
        add.connect_clicked(move |_| {
            let _ = input.send(SessionTabBarMsg::NewLocalTab);
        });
        widgets.tabs.append(&add);
        widgets.error.set_label(self.error.as_deref().unwrap_or(""));
        widgets.error.set_visible(self.error.is_some());
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn set_accessible_label(widget: &impl IsA<gtk::Accessible>, label: &str) {
    widget.update_property(&[gtk::accessible::Property::Label(label)]);
}
