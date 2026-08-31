use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::{TabId, UiCommand, UiPortError, WorkspaceState};

use crate::{TabOverflowModel, session_tab_bar_widgets::SessionTabBarWidgets};

#[derive(Debug, Clone)]
pub struct SessionTabBarInit {
    pub workspace: WorkspaceState,
}

#[derive(Debug, Clone)]
pub enum SessionTabBarMsg {
    SetWorkspace(WorkspaceState),
    Activate(TabId),
    ActivateFromOverflow(TabId),
    Cycle(i32),
    RevealActive,
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
        let active = init.workspace.active_tab;
        let model = Self {
            workspace: init.workspace,
            active,
            error: None,
        };
        let mut widgets = SessionTabBarWidgets::build(&root, &sender);
        widgets.render(
            &model.workspace,
            model.active,
            model.error.as_deref(),
            &sender,
        );
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
            SessionTabBarMsg::Activate(tab) | SessionTabBarMsg::ActivateFromOverflow(tab) => {
                self.activate(tab, &sender)
            }
            SessionTabBarMsg::Cycle(delta) => {
                let active_index = self
                    .active
                    .and_then(|active| self.workspace.tabs.iter().position(|tab| tab.id == active));
                let model = TabOverflowModel::new(self.workspace.tabs.len(), active_index, &[]);
                if let Some(index) = model.cycle(delta) {
                    self.activate(self.workspace.tabs[index].id, &sender);
                }
            }
            SessionTabBarMsg::RevealActive => {}
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
        widgets.render(&self.workspace, self.active, self.error.as_deref(), &sender);
    }
}

impl SessionTabBar {
    fn activate(&mut self, tab: TabId, sender: &ComponentSender<Self>) {
        if self.workspace.tab(tab).is_ok() {
            self.active = Some(tab);
            let _ = sender.output(SessionTabBarOutput::ActivateTab(tab));
        }
    }
}
