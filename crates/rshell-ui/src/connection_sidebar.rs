use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::{
    ConnectionCatalog, ConnectionGroup, ConnectionId, ConnectionProfile, GroupId, UiCommand,
    UiPortError,
};

use crate::{
    SidebarAction, SidebarRow, SidebarViewModel, connection_sidebar_selection as selection,
    connection_sidebar_widgets::ConnectionSidebarWidgets,
};

#[derive(Debug, Clone)]
pub struct ConnectionSidebarInit {
    pub catalog: ConnectionCatalog,
}

#[derive(Debug, Clone)]
pub enum ConnectionSidebarMsg {
    SetCatalog(ConnectionCatalog),
    Search(String),
    Select(usize),
    SelectConnection(ConnectionId),
    Activate(usize),
    CreateConnection,
    CreateGroup,
    EditSelected,
    DuplicateSelected,
    RequestDelete,
    CancelDelete,
    ConfirmDelete,
    Action(SidebarAction),
    CommandRejected(UiPortError),
}

#[derive(Debug)]
pub enum ConnectionSidebarOutput {
    Command(UiCommand),
    Connect(ConnectionId),
    OpenCreate(Option<GroupId>),
    OpenEdit(ConnectionProfile),
    SelectionChanged(Option<ConnectionId>),
}

impl ConnectionSidebarOutput {
    pub(crate) fn closes_navigation_drawer(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
}

pub struct ConnectionSidebar {
    view: SidebarViewModel,
    selected: Option<usize>,
    confirm_delete: bool,
    error: Option<String>,
}

impl SimpleComponent for ConnectionSidebar {
    type Init = ConnectionSidebarInit;
    type Input = ConnectionSidebarMsg;
    type Output = ConnectionSidebarOutput;
    type Root = gtk::Box;
    type Widgets = ConnectionSidebarWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("sidebar");
        root.set_width_request(232);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            view: SidebarViewModel::new(init.catalog),
            selected: None,
            confirm_delete: false,
            error: None,
        };
        let mut widgets = ConnectionSidebarWidgets::build(&root, &sender);
        model.render(&mut widgets);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ConnectionSidebarMsg::SetCatalog(catalog) => {
                self.replace_catalog(catalog);
                let _ = sender.output(ConnectionSidebarOutput::SelectionChanged(
                    self.selected_connection().map(|(id, _)| id),
                ));
            }
            ConnectionSidebarMsg::Search(query) => {
                self.view.set_query(query.clone());
                self.selected = None;
                let _ = sender.output(ConnectionSidebarOutput::SelectionChanged(None));
                let _ = sender.output(ConnectionSidebarOutput::Command(
                    SidebarAction::Search(query).into_command(),
                ));
            }
            ConnectionSidebarMsg::Select(index) => {
                self.selected = Some(index);
                let _ = sender.output(ConnectionSidebarOutput::SelectionChanged(
                    self.selected_connection().map(|(id, _)| id),
                ));
            }
            ConnectionSidebarMsg::SelectConnection(connection) => {
                self.selected = self.view.rows().iter().position(
                    |row| matches!(row, SidebarRow::Connection { id, .. } if *id == connection),
                );
                let _ = sender.output(ConnectionSidebarOutput::SelectionChanged(
                    self.selected_connection().map(|(id, _)| id),
                ));
            }
            ConnectionSidebarMsg::Activate(index) => self.connect_index(index, &sender),
            ConnectionSidebarMsg::CreateConnection => {
                let group = self.selected_group();
                let _ = sender.output(ConnectionSidebarOutput::OpenCreate(group));
            }
            ConnectionSidebarMsg::CreateGroup => self.output_action(
                SidebarAction::CreateGroup(ConnectionGroup::default()),
                &sender,
            ),
            ConnectionSidebarMsg::EditSelected => {
                if let Some(index) = self.selected {
                    self.open_index(index, &sender);
                }
            }
            ConnectionSidebarMsg::DuplicateSelected => {
                if let Some((id, group)) = self.selected_connection() {
                    self.output_action(
                        SidebarAction::Duplicate {
                            source: id,
                            destination: group,
                        },
                        &sender,
                    );
                }
            }
            ConnectionSidebarMsg::RequestDelete => self.confirm_delete = self.selected.is_some(),
            ConnectionSidebarMsg::CancelDelete => self.confirm_delete = false,
            ConnectionSidebarMsg::ConfirmDelete => {
                self.confirm_delete = false;
                if let Some(action) = self.delete_action() {
                    self.output_action(action, &sender);
                }
            }
            ConnectionSidebarMsg::Action(action) => self.output_action(action, &sender),
            ConnectionSidebarMsg::CommandRejected(error) => self.error = Some(error.to_string()),
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        self.render(widgets);
    }
}

impl ConnectionSidebar {
    fn replace_catalog(&mut self, catalog: ConnectionCatalog) {
        let selected = selection::selected_row(&self.view, self.selected);
        self.view.set_catalog(catalog);
        self.selected = selection::index_for_identity(&self.view, selected);
        self.confirm_delete = false;
        self.error = None;
    }

    fn render(&self, widgets: &mut ConnectionSidebarWidgets) {
        let rows = self.view.rows();
        let empty_text = if rows.is_empty() && self.view.query().trim().is_empty() {
            Some("No connections yet")
        } else if rows.is_empty() {
            Some("No connections match the search")
        } else {
            None
        };
        widgets.render(
            rows,
            self.selected,
            self.selected_connection().is_some(),
            self.confirm_delete,
            self.error.as_deref(),
            empty_text,
        );
    }

    fn open_index(&self, index: usize, sender: &ComponentSender<Self>) {
        if let Some(SidebarRow::Connection { id, .. }) = self.view.rows().get(index)
            && let Some(profile) = self.view.catalog().connections.get(id)
        {
            let _ = sender.output(ConnectionSidebarOutput::OpenEdit(profile.clone()));
        }
    }

    fn connect_index(&self, index: usize, sender: &ComponentSender<Self>) {
        if let Some(SidebarRow::Connection { id, .. }) = self.view.rows().get(index) {
            let _ = sender.output(ConnectionSidebarOutput::Connect(*id));
        }
    }

    fn selected_connection(&self) -> Option<(ConnectionId, Option<GroupId>)> {
        selection::selected_connection(&self.view, self.selected)
    }

    fn selected_group(&self) -> Option<GroupId> {
        selection::selected_group(&self.view, self.selected)
    }

    fn delete_action(&self) -> Option<SidebarAction> {
        selection::delete_action(&self.view, self.selected)
    }

    fn output_action(&self, action: SidebarAction, sender: &ComponentSender<Self>) {
        let _ = sender.output(ConnectionSidebarOutput::Command(action.into_command()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_replacement_cannot_retarget_a_stale_row_index() {
        let old = ConnectionProfile::new("Old", "old.example.test");
        let mut old_catalog = ConnectionCatalog::default();
        old_catalog.connections.insert(old.id, old);
        let mut sidebar = ConnectionSidebar {
            view: SidebarViewModel::new(old_catalog),
            selected: Some(0),
            confirm_delete: true,
            error: Some("stale".into()),
        };

        let replacement = ConnectionProfile::new("Replacement", "new.example.test");
        let mut replacement_catalog = ConnectionCatalog::default();
        replacement_catalog
            .connections
            .insert(replacement.id, replacement);
        sidebar.replace_catalog(replacement_catalog);

        assert!(sidebar.selected_connection().is_none());
        assert!(sidebar.selected_group().is_none());
        assert!(sidebar.delete_action().is_none());
        assert!(!sidebar.confirm_delete);
        assert!(sidebar.error.is_none());
    }
}
