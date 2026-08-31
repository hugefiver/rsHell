use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{
    ConnectionSidebar, ConnectionSidebarMsg, IconRenderRequest, ProductIcon, SidebarRow,
    connection_sidebar_row::sidebar_row,
};

pub struct ConnectionSidebarWidgets {
    list: gtk::ListBox,
    row_selected_handler: gtk::glib::SignalHandlerId,
    rows: Vec<SidebarRow>,
    edit: gtk::Button,
    duplicate: gtk::Button,
    delete: gtk::Button,
    confirmation: gtk::Revealer,
    confirm: gtk::Button,
    confirming: bool,
    empty: gtk::Label,
    error: gtk::Label,
}

impl ConnectionSidebarWidgets {
    pub(crate) fn build(root: &gtk::Box, sender: &ComponentSender<ConnectionSidebar>) -> Self {
        let title = gtk::Label::new(Some("Connections"));
        title.set_halign(gtk::Align::Start);
        title.add_css_class("sidebar-header");
        root.append(&title);

        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search connections")
            .tooltip_text("Search by name, host, user, or tag")
            .build();
        search.add_css_class("connection-search");
        search.update_property(&[gtk::accessible::Property::Label("Search connections")]);
        search.set_margin_start(6);
        search.set_margin_end(6);
        search.connect_search_changed({
            let sender = sender.clone();
            move |entry| sender.input(ConnectionSidebarMsg::Search(entry.text().into()))
        });
        root.append(&search);

        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        toolbar.add_css_class("sidebar-toolbar");
        let icon_request = IconRenderRequest::for_widget(16, root);
        let create = action_button(
            ProductIcon::AddConnection,
            Some("New"),
            "Create a connection",
            icon_request,
        );
        let create_group =
            action_button(ProductIcon::AddGroup, None, "Create a group", icon_request);
        let edit = action_button(ProductIcon::Edit, None, "Edit selection", icon_request);
        let duplicate = action_button(
            ProductIcon::Duplicate,
            None,
            "Duplicate connection",
            icon_request,
        );
        let delete = action_button(ProductIcon::Delete, None, "Delete selection", icon_request);
        for button in [&create, &create_group, &edit, &duplicate, &delete] {
            toolbar.append(button);
        }
        root.append(&toolbar);
        create.connect_clicked(send(sender, ConnectionSidebarMsg::CreateConnection));
        create_group.connect_clicked(send(sender, ConnectionSidebarMsg::CreateGroup));
        edit.connect_clicked(send(sender, ConnectionSidebarMsg::EditSelected));
        duplicate.connect_clicked(send(sender, ConnectionSidebarMsg::DuplicateSelected));
        delete.connect_clicked(send(sender, ConnectionSidebarMsg::RequestDelete));

        let list = gtk::ListBox::new();
        list.add_css_class("connection-list");
        list.set_selection_mode(gtk::SelectionMode::Single);
        let row_selected_handler = list.connect_row_selected({
            let sender = sender.clone();
            move |_, row| {
                if let Some(row) = row {
                    sender.input(ConnectionSidebarMsg::Select(row.index() as usize));
                }
            }
        });
        list.connect_row_activated({
            let sender = sender.clone();
            move |_, row| sender.input(ConnectionSidebarMsg::Activate(row.index() as usize))
        });
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();
        root.append(&scroll);

        let empty = gtk::Label::new(None);
        empty.add_css_class("sidebar-empty");
        empty.add_css_class("dim-label");
        empty.set_wrap(true);
        empty.set_margin_start(10);
        empty.set_margin_end(10);
        empty.set_visible(false);
        root.append(&empty);

        let (confirmation, confirm) = delete_confirmation(sender);
        root.append(&confirmation);
        let error = gtk::Label::new(None);
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        error.add_css_class("dim-label");
        root.append(&error);
        Self {
            list,
            row_selected_handler,
            rows: Vec::new(),
            edit,
            duplicate,
            delete,
            confirmation,
            confirm,
            confirming: false,
            empty,
            error,
        }
    }

    pub(crate) fn render(
        &mut self,
        rows: Vec<SidebarRow>,
        selected: Option<usize>,
        connection_selected: bool,
        confirm_delete: bool,
        error: Option<&str>,
        empty_text: Option<&str>,
    ) {
        if self.rows != rows {
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
            for row in &rows {
                self.list.append(&sidebar_row(row));
            }
            self.rows = rows;
        }
        let current = self.list.selected_row().map(|row| row.index() as usize);
        if current != selected {
            let row = selected.and_then(|index| self.list.row_at_index(index as i32));
            self.list.block_signal(&self.row_selected_handler);
            self.list.select_row(row.as_ref());
            self.list.unblock_signal(&self.row_selected_handler);
        }
        for index in 0..self.rows.len() {
            if let Some(row) = self.list.row_at_index(index as i32) {
                if selected == Some(index) {
                    row.add_css_class("navigation-selected");
                } else {
                    row.remove_css_class("navigation-selected");
                }
            }
        }
        self.edit.set_sensitive(selected.is_some());
        self.edit.set_tooltip_text(Some(if selected.is_some() {
            "Edit selection"
        } else {
            "Select a connection or group to edit"
        }));
        self.duplicate.set_sensitive(connection_selected);
        self.duplicate
            .set_tooltip_text(Some(if connection_selected {
                "Duplicate connection"
            } else {
                "Select a connection to duplicate"
            }));
        self.delete.set_sensitive(selected.is_some());
        self.delete.set_tooltip_text(Some(if selected.is_some() {
            "Delete selection"
        } else {
            "Select a connection or group to delete"
        }));
        self.confirmation.set_reveal_child(confirm_delete);
        if confirm_delete && !self.confirming {
            self.confirm.grab_focus();
        }
        self.confirming = confirm_delete;
        self.error.set_label(error.unwrap_or(""));
        self.error.set_visible(error.is_some());
        self.empty.set_label(empty_text.unwrap_or(""));
        self.empty.set_visible(empty_text.is_some());
    }
}

fn action_button(
    icon: ProductIcon,
    text: Option<&str>,
    tooltip: &str,
    request: IconRenderRequest,
) -> gtk::Button {
    let button = icon
        .button(Some(tooltip), request)
        .expect("embedded sidebar icon");
    if let Some(text) = text {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        content.append(&icon.image(request).expect("embedded sidebar icon"));
        let label = gtk::Label::new(Some(text));
        label.add_css_class("navigation-action-label");
        content.append(&label);
        button.set_child(Some(&content));
    }
    button
}

fn delete_confirmation(
    sender: &ComponentSender<ConnectionSidebar>,
) -> (gtk::Revealer, gtk::Button) {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 4);
    panel.set_margin_start(6);
    panel.set_margin_end(6);
    let prompt = gtk::Label::new(Some("Delete the selected item?"));
    prompt.set_halign(gtk::Align::Start);
    panel.append(&prompt);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let confirm = gtk::Button::with_label("Delete");
    confirm.add_css_class("destructive-action");
    actions.append(&cancel);
    actions.append(&confirm);
    panel.append(&actions);
    cancel.connect_clicked(send(sender, ConnectionSidebarMsg::CancelDelete));
    confirm.connect_clicked(send(sender, ConnectionSidebarMsg::ConfirmDelete));
    (gtk::Revealer::builder().child(&panel).build(), confirm)
}

fn send(
    sender: &ComponentSender<ConnectionSidebar>,
    message: ConnectionSidebarMsg,
) -> impl Fn(&gtk::Button) + 'static {
    let sender = sender.clone();
    move |_| sender.input(message.clone())
}
