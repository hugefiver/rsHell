use crate::connection::{
    ConnectionBackend, ConnectionProfile, ConnectionRepository, ConnectionStore,
};
use crate::terminal::{launch_local_session, launch_session, SessionPhase, TerminalSessionHandle};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::thread;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq)]
enum SplitLayout {
    Single,
    HSplit,
    VSplit,
    TopBottom3,
    Grid,
}

struct TerminalPane {
    name: String,
    handle: TerminalSessionHandle,
}

struct TerminalGroup {
    layout: SplitLayout,
    panes: Vec<TerminalPane>,
    active_pane: usize,
}

impl TerminalGroup {
    fn can_split(&self) -> bool {
        self.panes.len() < 4
    }

    fn add_pane(&mut self, pane: TerminalPane, horizontal: bool) {
        self.panes.push(pane);
        self.layout = match self.panes.len() {
            1 => SplitLayout::Single,
            2 => {
                if horizontal {
                    SplitLayout::HSplit
                } else {
                    SplitLayout::VSplit
                }
            }
            3 => SplitLayout::TopBottom3,
            _ => SplitLayout::Grid,
        };
        self.active_pane = self.panes.len() - 1;
    }

    fn remove_pane(&mut self, index: usize) {
        if index >= self.panes.len() {
            return;
        }
        self.panes[index].handle.shutdown();
        self.panes.remove(index);
        self.layout = match self.panes.len() {
            0 | 1 => SplitLayout::Single,
            2 => match self.layout {
                SplitLayout::VSplit => SplitLayout::VSplit,
                _ => SplitLayout::HSplit,
            },
            3 => SplitLayout::TopBottom3,
            _ => SplitLayout::Grid,
        };
        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len().saturating_sub(1);
        }
    }
}

pub struct ShellXApp {
    repository: ConnectionRepository,
    store: ConnectionStore,
    selected_connection_id: Option<Uuid>,
    draft: ConnectionDraft,
    groups: Vec<TerminalGroup>,
    selected_group: Option<usize>,
    toast: String,
    sidebar_visible: bool,
    editor_visible: bool,
    connections_dirty: Cell<bool>,
    groups_dirty: Cell<bool>,
    terminal_dirty: Cell<bool>,
    draft_dirty: Cell<bool>,
    updating_draft: Cell<bool>,
}

#[derive(Debug)]
pub enum AppMsg {
    SelectConnection(Uuid),
    NewConnection,
    SaveDraft,
    DeleteSelected,
    ToggleSidebar,
    ToggleEditor,
    DraftNameChanged(String),
    DraftFolderChanged(String),
    DraftHostChanged(String),
    DraftPortChanged(u16),
    DraftUserChanged(String),
    DraftPasswordChanged(String),
    DraftIdentityChanged(String),
    DraftCommandChanged(String),
    DraftNoteChanged(String),
    DraftBackendChanged(ConnectionBackend),
    DraftAcceptNewHostChanged(bool),
    LaunchSelected,
    NewLocalTab,
    SelectGroup(usize),
    CloseGroup(usize),
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    FocusPane(usize),
    PaneKeyPress(usize, gdk::Key, gdk::ModifierType),
    SessionLaunched(String, TerminalSessionHandle),
    SessionFailed(String),
    RefreshSessions,
    ShutdownAll,
}

#[derive(Default, Clone)]
struct ConnectionDraft {
    id: Option<Uuid>,
    name: String,
    folder: String,
    host: String,
    port: u16,
    user: String,
    password: String,
    identity_file: String,
    remote_command: String,
    note: String,
    backend: ConnectionBackend,
    accept_new_host: bool,
}

impl ConnectionDraft {
    fn from_profile(store: &ConnectionStore, profile: &ConnectionProfile) -> Self {
        Self {
            id: Some(profile.id),
            name: profile.name.clone(),
            folder: store
                .folder_name(profile.folder_id)
                .unwrap_or_default()
                .to_string(),
            host: profile.host.clone(),
            port: profile.port,
            user: profile.user.clone(),
            password: profile.password.clone(),
            identity_file: profile.identity_file.clone(),
            remote_command: profile.remote_command.clone(),
            note: profile.note.clone(),
            backend: profile.backend,
            accept_new_host: profile.accept_new_host,
        }
    }

    fn empty() -> Self {
        Self {
            port: 22,
            backend: ConnectionBackend::SystemOpenSsh,
            accept_new_host: true,
            ..Default::default()
        }
    }

    fn into_profile(self, store: &mut ConnectionStore) -> ConnectionProfile {
        let folder_id = store.ensure_folder_named(&self.folder);
        let mut profile = ConnectionProfile::new(
            if self.name.trim().is_empty() {
                "New connection"
            } else {
                self.name.trim()
            },
            self.host.trim(),
        );
        profile.id = self.id.unwrap_or_else(Uuid::new_v4);
        profile.folder_id = folder_id;
        profile.port = self.port;
        profile.user = self.user;
        profile.password = self.password;
        profile.identity_file = self.identity_file;
        profile.remote_command = self.remote_command;
        profile.note = self.note;
        profile.backend = self.backend;
        profile.accept_new_host = self.accept_new_host;
        profile
    }
}

pub struct AppWidgets {
    sidebar_revealer: gtk::Revealer,
    connection_list: gtk::ListBox,
    editor_dialog: gtk::Window,
    draft_name: gtk::Entry,
    draft_folder: gtk::Entry,
    draft_host: gtk::Entry,
    draft_port: gtk::SpinButton,
    draft_user: gtk::Entry,
    draft_password: gtk::PasswordEntry,
    draft_identity: gtk::Entry,
    draft_command: gtk::Entry,
    draft_note: gtk::TextView,
    accept_new_host: gtk::CheckButton,
    backend_system: gtk::CheckButton,
    backend_wezterm: gtk::CheckButton,
    connect_btn: gtk::Button,
    split_h_btn: gtk::Button,
    split_v_btn: gtk::Button,
    close_pane_btn: gtk::Button,
    tab_bar: gtk::Box,
    terminal_container: gtk::Box,
    pane_views: Vec<gtk::TextView>,
    pane_sizes: Vec<(u16, u16)>,
    status_label: gtk::Label,
    toast_label: gtk::Label,
}

impl ShellXApp {
    fn selected_profile(&self) -> Option<&ConnectionProfile> {
        self.selected_connection_id
            .and_then(|id| self.store.connection(id))
    }

    fn load_draft_from_selection(&mut self) {
        if let Some(profile) = self.selected_profile() {
            self.draft = ConnectionDraft::from_profile(&self.store, profile);
        }
    }

    fn save_store(&mut self) {
        match self.repository.save(&self.store) {
            Ok(()) => {
                self.toast = format!("Saved {} connections", self.store.connections.len());
            }
            Err(e) => {
                self.toast = format!("Save failed: {e:#}");
            }
        }
    }

    fn selected_group(&self) -> Option<&TerminalGroup> {
        self.selected_group.and_then(|i| self.groups.get(i))
    }

    fn selected_group_mut(&mut self) -> Option<&mut TerminalGroup> {
        self.selected_group.and_then(|i| self.groups.get_mut(i))
    }

    fn live_count(&self) -> usize {
        self.groups
            .iter()
            .flat_map(|g| &g.panes)
            .filter(|p| {
                matches!(
                    p.handle.snapshot().phase,
                    SessionPhase::Connecting | SessionPhase::Connected | SessionPhase::Attention
                )
            })
            .count()
    }

    fn status_text(&self) -> String {
        if let Some(group) = self.selected_group() {
            if let Some(pane) = group.panes.get(group.active_pane) {
                let snap = pane.handle.snapshot();
                return format!(
                    "{}  ·  {}  ·  {}  ·  Panes: {}",
                    snap.phase.label(),
                    pane.name,
                    snap.status_line,
                    group.panes.len()
                );
            }
        }
        format!(
            "Sessions: {}  ·  Live: {}",
            self.groups.len(),
            self.live_count()
        )
    }
}

impl ShellXApp {
    fn update_impl(&mut self, message: AppMsg, sender: &ComponentSender<Self>) {
        match message {
            AppMsg::SelectConnection(id) => {
                self.selected_connection_id = Some(id);
                self.load_draft_from_selection();
                self.draft_dirty.set(true);
            }
            AppMsg::NewConnection => {
                self.selected_connection_id = None;
                self.draft = ConnectionDraft::empty();
                self.editor_visible = true;
                self.connections_dirty.set(true);
                self.draft_dirty.set(true);
            }
            AppMsg::SaveDraft => {
                let draft = std::mem::take(&mut self.draft);
                let profile = draft.into_profile(&mut self.store);
                self.selected_connection_id = Some(profile.id);
                self.store.upsert(profile.clone());
                self.draft = ConnectionDraft::from_profile(&self.store, &profile);
                self.save_store();
                self.editor_visible = false;
                self.connections_dirty.set(true);
                self.draft_dirty.set(true);
            }
            AppMsg::DeleteSelected => {
                if let Some(id) = self.selected_connection_id {
                    if let Some(removed) = self.store.remove(id) {
                        self.toast = format!("Deleted {}", removed.name);
                        self.selected_connection_id = self.store.connections.first().map(|p| p.id);
                        if self.selected_connection_id.is_some() {
                            self.load_draft_from_selection();
                        } else {
                            self.draft = ConnectionDraft::empty();
                        }
                        self.save_store();
                        self.connections_dirty.set(true);
                        self.draft_dirty.set(true);
                    }
                }
            }
            AppMsg::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            AppMsg::ToggleEditor => {
                if self.editor_visible {
                    self.editor_visible = false;
                } else {
                    self.load_draft_from_selection();
                    self.editor_visible = true;
                    self.draft_dirty.set(true);
                }
            }
            AppMsg::DraftNameChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.name = v;
                }
            }
            AppMsg::DraftFolderChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.folder = v;
                }
            }
            AppMsg::DraftHostChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.host = v;
                }
            }
            AppMsg::DraftPortChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.port = v;
                }
            }
            AppMsg::DraftUserChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.user = v;
                }
            }
            AppMsg::DraftPasswordChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.password = v;
                }
            }
            AppMsg::DraftIdentityChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.identity_file = v;
                }
            }
            AppMsg::DraftCommandChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.remote_command = v;
                }
            }
            AppMsg::DraftNoteChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.note = v;
                }
            }
            AppMsg::DraftBackendChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.backend = v;
                }
            }
            AppMsg::DraftAcceptNewHostChanged(v) => {
                if !self.updating_draft.get() {
                    self.draft.accept_new_host = v;
                }
            }
            AppMsg::LaunchSelected => {
                let Some(profile) = self.selected_profile().cloned() else {
                    self.toast = "Select a connection first".into();
                    return;
                };
                self.toast = format!("Connecting to {}...", profile.name);
                let name = profile.name.clone();
                let input_tx = sender.input_sender().clone();
                thread::spawn(move || match launch_session(&profile) {
                    Ok(handle) => {
                        let _ = input_tx.send(AppMsg::SessionLaunched(name, handle));
                    }
                    Err(e) => {
                        let _ = input_tx.send(AppMsg::SessionFailed(format!("{name}: {e:#}")));
                    }
                });
            }
            AppMsg::NewLocalTab => match launch_local_session() {
                Ok(handle) => {
                    let mut group = TerminalGroup {
                        layout: SplitLayout::Single,
                        panes: Vec::new(),
                        active_pane: 0,
                    };
                    group.panes.push(TerminalPane {
                        name: "Local Shell".into(),
                        handle,
                    });
                    self.groups.push(group);
                    self.selected_group = Some(self.groups.len() - 1);
                    self.groups_dirty.set(true);
                    self.terminal_dirty.set(true);
                }
                Err(e) => {
                    self.toast = format!("Local shell failed: {e:#}");
                }
            },
            AppMsg::SelectGroup(index) => {
                if index < self.groups.len() {
                    self.selected_group = Some(index);
                    self.terminal_dirty.set(true);
                }
            }
            AppMsg::CloseGroup(index) => {
                if index < self.groups.len() {
                    let group = &self.groups[index];
                    for pane in &group.panes {
                        pane.handle.shutdown();
                    }
                    self.groups.remove(index);
                    self.selected_group = if self.groups.is_empty() {
                        None
                    } else if let Some(sel) = self.selected_group {
                        if sel > index {
                            Some(sel - 1)
                        } else if sel >= self.groups.len() {
                            Some(self.groups.len() - 1)
                        } else {
                            Some(sel)
                        }
                    } else {
                        None
                    };
                    self.groups_dirty.set(true);
                    self.terminal_dirty.set(true);
                }
            }
            AppMsg::SplitHorizontal => {
                if let Some(group) = self.selected_group_mut() {
                    if group.can_split() {
                        match launch_local_session() {
                            Ok(handle) => {
                                group.add_pane(
                                    TerminalPane {
                                        name: "Local Shell".into(),
                                        handle,
                                    },
                                    true,
                                );
                                self.terminal_dirty.set(true);
                            }
                            Err(e) => {
                                self.toast = format!("Split failed: {e:#}");
                            }
                        }
                    } else {
                        self.toast = "Max 4 panes per tab".into();
                    }
                }
            }
            AppMsg::SplitVertical => {
                if let Some(group) = self.selected_group_mut() {
                    if group.can_split() {
                        match launch_local_session() {
                            Ok(handle) => {
                                group.add_pane(
                                    TerminalPane {
                                        name: "Local Shell".into(),
                                        handle,
                                    },
                                    false,
                                );
                                self.terminal_dirty.set(true);
                            }
                            Err(e) => {
                                self.toast = format!("Split failed: {e:#}");
                            }
                        }
                    } else {
                        self.toast = "Max 4 panes per tab".into();
                    }
                }
            }
            AppMsg::ClosePane => {
                let should_remove_group = if let Some(group) = self.selected_group_mut() {
                    if group.panes.len() > 1 {
                        let idx = group.active_pane;
                        group.remove_pane(idx);
                        self.terminal_dirty.set(true);
                        false
                    } else {
                        true
                    }
                } else {
                    false
                };
                if should_remove_group {
                    if let Some(gi) = self.selected_group {
                        self.update_impl(AppMsg::CloseGroup(gi), sender);
                    }
                }
            }
            AppMsg::FocusPane(index) => {
                if let Some(group) = self.selected_group_mut() {
                    if index < group.panes.len() {
                        group.active_pane = index;
                    }
                }
            }
            AppMsg::PaneKeyPress(pane_index, key, modifiers) => {
                if let Some(bytes) = key_to_bytes(key, modifiers) {
                    if let Some(group) = self.selected_group() {
                        if let Some(pane) = group.panes.get(pane_index) {
                            let _ = pane.handle.send_bytes(bytes);
                        }
                    }
                }
            }
            AppMsg::RefreshSessions => {
                // No-op: exists solely to trigger view refresh via the 250ms timer.
                // Relm4 calls update_view() after each update(), so receiving this
                // message causes terminal content, status bar, etc. to refresh.
            }
            AppMsg::SessionLaunched(name, handle) => {
                let mut group = TerminalGroup {
                    layout: SplitLayout::Single,
                    panes: Vec::new(),
                    active_pane: 0,
                };
                group.panes.push(TerminalPane {
                    name: name.clone(),
                    handle,
                });
                self.groups.push(group);
                self.selected_group = Some(self.groups.len() - 1);
                self.toast = format!("Connected to {name}");
                self.groups_dirty.set(true);
                self.terminal_dirty.set(true);
            }
            AppMsg::SessionFailed(msg) => {
                self.toast = format!("Failed: {msg}");
            }
            AppMsg::ShutdownAll => {
                for group in &self.groups {
                    for pane in &group.panes {
                        pane.handle.shutdown();
                    }
                }
                self.groups.clear();
                self.selected_group = None;
                self.groups_dirty.set(true);
                self.terminal_dirty.set(true);
            }
        }
    }

    fn view_impl(&self, widgets: &mut AppWidgets, sender: ComponentSender<Self>) {
        widgets.toast_label.set_label(&self.toast);
        widgets
            .sidebar_revealer
            .set_reveal_child(self.sidebar_visible);
        if self.editor_visible {
            widgets.editor_dialog.present();
        } else {
            widgets.editor_dialog.set_visible(false);
        }

        widgets
            .connect_btn
            .set_sensitive(self.selected_connection_id.is_some());
        let has_group = self.selected_group.is_some();
        let can_split = self.selected_group().map_or(false, |g| g.can_split());
        widgets.split_h_btn.set_sensitive(has_group && can_split);
        widgets.split_v_btn.set_sensitive(has_group && can_split);
        widgets.close_pane_btn.set_sensitive(has_group);

        widgets.status_label.set_label(&self.status_text());

        if self.draft_dirty.get() {
            self.updating_draft.set(true);
            widgets.draft_name.set_text(&self.draft.name);
            widgets.draft_folder.set_text(&self.draft.folder);
            widgets.draft_host.set_text(&self.draft.host);
            widgets.draft_port.set_value(self.draft.port as f64);
            widgets.draft_user.set_text(&self.draft.user);
            widgets.draft_password.set_text(&self.draft.password);
            widgets.draft_identity.set_text(&self.draft.identity_file);
            widgets.draft_command.set_text(&self.draft.remote_command);
            widgets
                .accept_new_host
                .set_active(self.draft.accept_new_host);
            widgets.backend_system.set_active(matches!(
                self.draft.backend,
                ConnectionBackend::SystemOpenSsh
            ));
            widgets
                .backend_wezterm
                .set_active(matches!(self.draft.backend, ConnectionBackend::WezTermSsh));
            widgets.draft_note.buffer().set_text(&self.draft.note);
            self.updating_draft.set(false);
            self.draft_dirty.set(false);
        }

        if self.connections_dirty.get() {
            while let Some(row) = widgets.connection_list.row_at_index(0) {
                widgets.connection_list.remove(&row);
            }

            let grouped = self.store.sorted_connections().into_iter().fold(
                BTreeMap::<String, Vec<&ConnectionProfile>>::new(),
                |mut acc, conn| {
                    acc.entry(
                        self.store
                            .folder_name(conn.folder_id)
                            .unwrap_or("Ungrouped")
                            .to_string(),
                    )
                    .or_default()
                    .push(conn);
                    acc
                },
            );

            for (folder, connections) in grouped {
                let header = gtk::Label::new(Some(&folder));
                header.set_halign(gtk::Align::Start);
                header.add_css_class("folder-header");
                widgets.connection_list.append(&header);

                for conn in connections {
                    let row = gtk::ListBoxRow::new();
                    row.set_tooltip_text(Some(&conn.id.to_string()));
                    let card = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(2)
                        .build();
                    card.add_css_class("connection-row");
                    let title = gtk::Label::new(Some(&conn.name));
                    title.set_halign(gtk::Align::Start);
                    title.add_css_class("connection-name");
                    let meta = gtk::Label::new(Some(&format!(
                        "{} · {}",
                        conn.host_label(),
                        conn.backend.label()
                    )));
                    meta.set_halign(gtk::Align::Start);
                    meta.add_css_class("connection-meta");
                    card.append(&title);
                    card.append(&meta);
                    row.set_child(Some(&card));
                    widgets.connection_list.append(&row);
                    if self.selected_connection_id == Some(conn.id) {
                        widgets.connection_list.select_row(Some(&row));
                    }
                }
            }
            self.connections_dirty.set(false);
        }

        if self.groups_dirty.get() {
            while let Some(child) = widgets.tab_bar.first_child() {
                widgets.tab_bar.remove(&child);
            }

            for (i, group) in self.groups.iter().enumerate() {
                let label_text = if let Some(first) = group.panes.first() {
                    first.name.clone()
                } else {
                    format!("Tab {}", i + 1)
                };

                let tab_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(4)
                    .build();

                let label = gtk::Label::new(Some(&label_text));
                tab_box.append(&label);

                let close_btn = gtk::Button::with_label("✕");
                close_btn.add_css_class("tab-close");
                let s = sender.clone();
                close_btn.connect_clicked(move |_| {
                    s.input(AppMsg::CloseGroup(i));
                });
                tab_box.append(&close_btn);

                let btn = gtk::Button::new();
                btn.set_child(Some(&tab_box));
                btn.add_css_class("tab-button");
                if self.selected_group == Some(i) {
                    btn.add_css_class("active-tab");
                }
                let s = sender.clone();
                btn.connect_clicked(move |_| {
                    s.input(AppMsg::SelectGroup(i));
                });
                widgets.tab_bar.append(&btn);
            }

            let add_btn = gtk::Button::with_label("+");
            add_btn.add_css_class("tab-add");
            let s = sender.clone();
            add_btn.connect_clicked(move |_| {
                s.input(AppMsg::NewLocalTab);
            });
            widgets.tab_bar.append(&add_btn);

            self.groups_dirty.set(false);
        }

        if self.terminal_dirty.get() {
            if let Some(group) = self.selected_group() {
                widgets.pane_views =
                    rebuild_terminal_panes(&widgets.terminal_container, group, &sender);
                widgets.pane_sizes = vec![(0, 0); widgets.pane_views.len()];
            } else {
                while let Some(child) = widgets.terminal_container.first_child() {
                    widgets.terminal_container.remove(&child);
                }
                let placeholder =
                    gtk::Label::new(Some("Press \"+ Terminal\" or \"Connect\" to start"));
                placeholder.set_vexpand(true);
                placeholder.set_hexpand(true);
                widgets.terminal_container.append(&placeholder);
                widgets.pane_views.clear();
                widgets.pane_sizes.clear();
            }
            self.terminal_dirty.set(false);
        }

        if let Some(group) = self.selected_group() {
            for (i, pane) in group.panes.iter().enumerate() {
                if let Some(tv) = widgets.pane_views.get(i) {
                    let (screen_text, cursor_offset) = pane.handle.screen_text_with_cursor(500);
                    let buffer = tv.buffer();
                    let current = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                    if current.as_str() != screen_text.as_str() {
                        buffer.set_text(&screen_text);
                    }
                    if let Some(offset) = cursor_offset {
                        let mut iter = buffer.iter_at_offset(offset);
                        buffer.place_cursor(&iter);
                        tv.scroll_to_iter(&mut iter, 0.0, false, 0.0, 1.0);
                    }

                    // Resize terminal to match pane allocation
                    let w = tv.allocated_width();
                    let h = tv.allocated_height();
                    if w > 0 && h > 0 {
                        let layout = tv.create_pango_layout(Some("M"));
                        let (char_w, char_h) = layout.pixel_size();
                        if char_w > 0 && char_h > 0 {
                            let cols = (w / char_w) as u16;
                            let rows = (h / char_h) as u16;
                            if cols > 0 && rows > 0 {
                                let last = widgets.pane_sizes.get(i).copied().unwrap_or((0, 0));
                                if (cols, rows) != last {
                                    let _ = pane.handle.resize(cols, rows);
                                    if i < widgets.pane_sizes.len() {
                                        widgets.pane_sizes[i] = (cols, rows);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn key_to_bytes(key: gdk::Key, modifiers: gdk::ModifierType) -> Option<Vec<u8>> {
    let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);

    if ctrl {
        if let Some(ch) = key.to_unicode() {
            let ch = ch.to_ascii_lowercase();
            if ch.is_ascii_lowercase() {
                return Some(vec![(ch as u8) - b'a' + 1]);
            }
        }
    }

    match key {
        gdk::Key::Return | gdk::Key::KP_Enter => Some(vec![b'\r']),
        gdk::Key::BackSpace => Some(vec![0x7f]),
        gdk::Key::Tab => Some(vec![b'\t']),
        gdk::Key::Escape => Some(vec![0x1b]),
        gdk::Key::Up => Some(b"\x1b[A".to_vec()),
        gdk::Key::Down => Some(b"\x1b[B".to_vec()),
        gdk::Key::Right => Some(b"\x1b[C".to_vec()),
        gdk::Key::Left => Some(b"\x1b[D".to_vec()),
        gdk::Key::Home => Some(b"\x1b[H".to_vec()),
        gdk::Key::End => Some(b"\x1b[F".to_vec()),
        gdk::Key::Delete | gdk::Key::KP_Delete => Some(b"\x1b[3~".to_vec()),
        gdk::Key::Page_Up => Some(b"\x1b[5~".to_vec()),
        gdk::Key::Page_Down => Some(b"\x1b[6~".to_vec()),
        gdk::Key::Insert => Some(b"\x1b[2~".to_vec()),
        gdk::Key::F1 => Some(b"\x1bOP".to_vec()),
        gdk::Key::F2 => Some(b"\x1bOQ".to_vec()),
        gdk::Key::F3 => Some(b"\x1bOR".to_vec()),
        gdk::Key::F4 => Some(b"\x1bOS".to_vec()),
        gdk::Key::F5 => Some(b"\x1b[15~".to_vec()),
        gdk::Key::F6 => Some(b"\x1b[17~".to_vec()),
        gdk::Key::F7 => Some(b"\x1b[18~".to_vec()),
        gdk::Key::F8 => Some(b"\x1b[19~".to_vec()),
        gdk::Key::F9 => Some(b"\x1b[20~".to_vec()),
        gdk::Key::F10 => Some(b"\x1b[21~".to_vec()),
        gdk::Key::F11 => Some(b"\x1b[23~".to_vec()),
        gdk::Key::F12 => Some(b"\x1b[24~".to_vec()),
        _ => {
            if let Some(ch) = key.to_unicode() {
                if alt {
                    let mut bytes = vec![0x1b];
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    Some(bytes)
                } else {
                    let mut buf = [0u8; 4];
                    Some(ch.encode_utf8(&mut buf).as_bytes().to_vec())
                }
            } else {
                None
            }
        }
    }
}

fn scroll_wrap(child: &gtk::TextView) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(child)
        .build()
}

fn build_pane_view(index: usize, sender: &ComponentSender<ShellXApp>) -> gtk::TextView {
    let tv = gtk::TextView::new();
    tv.set_editable(false);
    tv.set_cursor_visible(true);
    tv.set_monospace(true);
    tv.set_wrap_mode(gtk::WrapMode::None);
    tv.add_css_class("terminal-view");
    tv.set_can_focus(true);
    tv.set_focusable(true);

    let s = sender.clone();
    let kc = gtk::EventControllerKey::new();
    kc.connect_key_pressed(move |_, key, _, mods| {
        s.input(AppMsg::PaneKeyPress(index, key, mods));
        glib::Propagation::Stop
    });
    tv.add_controller(kc);

    let s = sender.clone();
    let tv_ref = tv.clone();
    let click = gtk::GestureClick::new();
    click.connect_pressed(move |_, _, _, _| {
        tv_ref.grab_focus();
        s.input(AppMsg::FocusPane(index));
    });
    tv.add_controller(click);

    tv
}

fn rebuild_terminal_panes(
    container: &gtk::Box,
    group: &TerminalGroup,
    sender: &ComponentSender<ShellXApp>,
) -> Vec<gtk::TextView> {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    if group.panes.is_empty() {
        let label = gtk::Label::new(Some("No terminal panes"));
        label.set_vexpand(true);
        label.set_hexpand(true);
        container.append(&label);
        return vec![];
    }

    let views: Vec<gtk::TextView> = (0..group.panes.len())
        .map(|i| build_pane_view(i, sender))
        .collect();

    match group.layout {
        SplitLayout::Single => {
            container.append(&scroll_wrap(&views[0]));
        }
        SplitLayout::HSplit => {
            let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
            paned.set_start_child(Some(&scroll_wrap(&views[0])));
            paned.set_end_child(Some(&scroll_wrap(&views[1])));
            paned.set_hexpand(true);
            paned.set_vexpand(true);
            container.append(&paned);
        }
        SplitLayout::VSplit => {
            let paned = gtk::Paned::new(gtk::Orientation::Vertical);
            paned.set_start_child(Some(&scroll_wrap(&views[0])));
            paned.set_end_child(Some(&scroll_wrap(&views[1])));
            paned.set_hexpand(true);
            paned.set_vexpand(true);
            container.append(&paned);
        }
        SplitLayout::TopBottom3 => {
            let outer = gtk::Paned::new(gtk::Orientation::Vertical);
            let inner = gtk::Paned::new(gtk::Orientation::Horizontal);
            inner.set_start_child(Some(&scroll_wrap(&views[1])));
            inner.set_end_child(Some(&scroll_wrap(&views[2])));
            outer.set_start_child(Some(&scroll_wrap(&views[0])));
            outer.set_end_child(Some(&inner));
            outer.set_hexpand(true);
            outer.set_vexpand(true);
            container.append(&outer);
        }
        SplitLayout::Grid => {
            let outer = gtk::Paned::new(gtk::Orientation::Vertical);
            let top = gtk::Paned::new(gtk::Orientation::Horizontal);
            let bottom = gtk::Paned::new(gtk::Orientation::Horizontal);
            top.set_start_child(Some(&scroll_wrap(&views[0])));
            top.set_end_child(Some(&scroll_wrap(&views[1])));
            bottom.set_start_child(Some(&scroll_wrap(&views[2])));
            bottom.set_end_child(Some(&scroll_wrap(&views[3])));
            outer.set_start_child(Some(&top));
            outer.set_end_child(Some(&bottom));
            outer.set_hexpand(true);
            outer.set_vexpand(true);
            container.append(&outer);
        }
    }

    if let Some(v) = views.get(group.active_pane) {
        v.grab_focus();
    }

    views
}

impl SimpleComponent for ShellXApp {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type Root = gtk::Window;
    type Widgets = AppWidgets;

    fn init_root() -> Self::Root {
        let w = gtk::Window::builder()
            .title("ShellX")
            .default_width(1280)
            .default_height(800)
            .build();
        w
    }

    fn init(
        _init: Self::Init,
        window: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        crate::theme::apply_global_css();

        let repository = ConnectionRepository::default();
        let mut store = repository.load().unwrap_or_default();
        if store.connections.is_empty() {
            let mut sample = ConnectionProfile::new("Demo host", "127.0.0.1");
            sample.note = "Sample profile".into();
            store.upsert(sample.clone());
            let _ = repository.save(&store);
        }

        let selected_connection_id = store.connections.first().map(|p| p.id);
        let draft = selected_connection_id
            .and_then(|id| store.connection(id))
            .map(|p| ConnectionDraft::from_profile(&store, p))
            .unwrap_or_else(ConnectionDraft::empty);

        let mut model = ShellXApp {
            repository,
            store,
            selected_connection_id,
            draft,
            groups: Vec::new(),
            selected_group: None,
            toast: "Ready".into(),
            sidebar_visible: true,
            editor_visible: false,
            connections_dirty: Cell::new(true),
            groups_dirty: Cell::new(true),
            terminal_dirty: Cell::new(true),
            draft_dirty: Cell::new(true),
            updating_draft: Cell::new(false),
        };

        match launch_local_session() {
            Ok(handle) => {
                let mut group = TerminalGroup {
                    layout: SplitLayout::Single,
                    panes: Vec::new(),
                    active_pane: 0,
                };
                group.panes.push(TerminalPane {
                    name: "Local Shell".into(),
                    handle,
                });
                model.groups.push(group);
                model.selected_group = Some(0);
                model.toast = "Local shell launched".into();
            }
            Err(e) => {
                model.toast = format!("Local shell failed: {e:#}");
            }
        }

        let root_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        root_vbox.add_css_class("shellx-root");

        let header_bar = gtk::HeaderBar::new();
        header_bar.add_css_class("shellx-toolbar");

        let menu = gio::Menu::new();
        menu.append(Some("Toggle Sidebar"), Some("win.toggle-sidebar"));
        menu.append(Some("New Session"), Some("win.new-session"));
        menu.append(Some("New Local Tab"), Some("win.new-local-tab"));
        let section2 = gio::Menu::new();
        section2.append(Some("About ShellX"), Some("win.about"));
        menu.append_section(None, &section2);

        let menu_btn = gtk::MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_tooltip_text(Some("Menu"));
        let popover_menu = gtk::PopoverMenu::from_model(Some(&menu));
        menu_btn.set_popover(Some(&popover_menu));

        let action_toggle_sidebar = gio::SimpleAction::new("toggle-sidebar", None);
        {
            let s = sender.clone();
            action_toggle_sidebar.connect_activate(move |_, _| {
                s.input(AppMsg::ToggleSidebar);
            });
        }

        let action_new_session = gio::SimpleAction::new("new-session", None);
        {
            let s = sender.clone();
            action_new_session.connect_activate(move |_, _| {
                s.input(AppMsg::NewConnection);
            });
        }

        let action_new_local = gio::SimpleAction::new("new-local-tab", None);
        {
            let s = sender.clone();
            action_new_local.connect_activate(move |_, _| {
                s.input(AppMsg::NewLocalTab);
            });
        }

        let action_about = gio::SimpleAction::new("about", None);
        {
            let win_ref = window.clone();
            action_about.connect_activate(move |_, _| {
                let about = gtk::AboutDialog::builder()
                    .program_name("ShellX")
                    .version("0.1.0")
                    .comments("Cross-platform SSH Terminal Manager")
                    .transient_for(&win_ref)
                    .modal(true)
                    .build();
                about.present();
            });
        }

        let actions = gio::SimpleActionGroup::new();
        actions.add_action(&action_toggle_sidebar);
        actions.add_action(&action_new_session);
        actions.add_action(&action_new_local);
        actions.add_action(&action_about);
        window.insert_action_group("win", Some(&actions));

        let connect_btn = gtk::Button::with_label("Connect");
        connect_btn.add_css_class("connect-button");

        let sep1 = gtk::Separator::new(gtk::Orientation::Vertical);
        sep1.set_margin_start(4);
        sep1.set_margin_end(4);

        let split_h_btn = gtk::Button::with_label("H-Split");
        let split_v_btn = gtk::Button::with_label("V-Split");
        let close_pane_btn = gtk::Button::with_label("Close Pane");

        let toast_label = gtk::Label::new(None);
        toast_label.add_css_class("toast-label");
        toast_label.set_halign(gtk::Align::End);
        toast_label.set_hexpand(true);

        header_bar.pack_start(&menu_btn);
        header_bar.pack_start(&connect_btn);
        header_bar.pack_start(&sep1);
        header_bar.pack_start(&split_h_btn);
        header_bar.pack_start(&split_v_btn);
        header_bar.pack_start(&close_pane_btn);
        header_bar.set_title_widget(Some(&toast_label));

        window.set_titlebar(Some(&header_bar));

        let main_paned = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .hexpand(true)
            .vexpand(true)
            .position(160)
            .shrink_start_child(false)
            .resize_start_child(false)
            .shrink_end_child(false)
            .resize_end_child(true)
            .build();

        let sidebar_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideRight)
            .reveal_child(true)
            .build();

        let sidebar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(140)
            .build();
        sidebar.add_css_class("sidebar");

        let sidebar_header = gtk::Label::new(Some("SESSIONS"));
        sidebar_header.set_halign(gtk::Align::Start);
        sidebar_header.add_css_class("sidebar-header");

        let sidebar_toolbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .build();
        sidebar_toolbar.add_css_class("sidebar-toolbar");
        let btn_new = gtk::Button::with_label("New");
        let btn_edit = gtk::Button::with_label("Edit");
        let btn_del = gtk::Button::with_label("Del");
        sidebar_toolbar.append(&btn_new);
        sidebar_toolbar.append(&btn_edit);
        sidebar_toolbar.append(&btn_del);

        let connection_list = gtk::ListBox::new();
        connection_list.set_selection_mode(gtk::SelectionMode::Single);
        connection_list.add_css_class("connection-list");

        let connection_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&connection_list)
            .build();

        let editor_dialog = gtk::Window::builder()
            .title("Session Editor")
            .modal(true)
            .transient_for(&window)
            .default_width(360)
            .default_height(460)
            .resizable(false)
            .build();
        editor_dialog.add_css_class("editor-dialog");

        let editor = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();
        editor.add_css_class("editor-group");

        let draft_name = gtk::Entry::new();
        draft_name.set_placeholder_text(Some("Session name"));
        draft_name.set_hexpand(true);
        let draft_folder = gtk::Entry::new();
        draft_folder.set_placeholder_text(Some("Group folder"));
        draft_folder.set_hexpand(true);

        let draft_host = gtk::Entry::new();
        draft_host.set_placeholder_text(Some("hostname or IP"));
        draft_host.set_hexpand(true);
        let draft_port = gtk::SpinButton::with_range(1.0, 65535.0, 1.0);
        draft_port.set_width_chars(6);

        let draft_user = gtk::Entry::new();
        draft_user.set_placeholder_text(Some("root"));
        draft_user.set_hexpand(true);
        let draft_password = gtk::PasswordEntry::new();
        draft_password.set_hexpand(true);
        let draft_identity = gtk::Entry::new();
        draft_identity.set_placeholder_text(Some("~/.ssh/id_rsa"));
        draft_identity.set_hexpand(true);
        let draft_command = gtk::Entry::new();
        draft_command.set_placeholder_text(Some("optional"));
        draft_command.set_hexpand(true);
        let draft_note = gtk::TextView::new();
        draft_note.set_wrap_mode(gtk::WrapMode::WordChar);
        draft_note.set_vexpand(false);
        let note_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(40)
            .child(&draft_note)
            .build();

        let accept_new_host = gtk::CheckButton::with_label("Accept new host keys");
        let backend_system = gtk::CheckButton::with_label("OpenSSH");
        let backend_wezterm = gtk::CheckButton::with_label("WezTerm");
        backend_wezterm.set_group(Some(&backend_system));

        let grid = gtk::Grid::builder()
            .row_spacing(3)
            .column_spacing(6)
            .build();

        let mut row = 0;
        for (label_text, widget, col_span) in [
            ("Name", draft_name.upcast_ref::<gtk::Widget>(), 3),
            ("Folder", draft_folder.upcast_ref::<gtk::Widget>(), 3),
        ] {
            let lbl = gtk::Label::new(Some(label_text));
            lbl.set_halign(gtk::Align::End);
            lbl.set_valign(gtk::Align::Center);
            grid.attach(&lbl, 0, row, 1, 1);
            grid.attach(widget, 1, row, col_span, 1);
            row += 1;
        }

        let sep1 = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep1.set_margin_top(2);
        sep1.set_margin_bottom(2);
        grid.attach(&sep1, 0, row, 4, 1);
        row += 1;

        let lbl_host = gtk::Label::new(Some("Host"));
        lbl_host.set_halign(gtk::Align::End);
        lbl_host.set_valign(gtk::Align::Center);
        grid.attach(&lbl_host, 0, row, 1, 1);
        grid.attach(&draft_host, 1, row, 1, 1);
        let lbl_port = gtk::Label::new(Some("Port"));
        lbl_port.set_halign(gtk::Align::End);
        lbl_port.set_valign(gtk::Align::Center);
        grid.attach(&lbl_port, 2, row, 1, 1);
        grid.attach(&draft_port, 3, row, 1, 1);
        row += 1;

        for (label_text, widget) in [
            ("User", draft_user.upcast_ref::<gtk::Widget>()),
            ("Password", draft_password.upcast_ref::<gtk::Widget>()),
            ("Key file", draft_identity.upcast_ref::<gtk::Widget>()),
            ("Command", draft_command.upcast_ref::<gtk::Widget>()),
        ] {
            let lbl = gtk::Label::new(Some(label_text));
            lbl.set_halign(gtk::Align::End);
            lbl.set_valign(gtk::Align::Center);
            grid.attach(&lbl, 0, row, 1, 1);
            grid.attach(widget, 1, row, 3, 1);
            row += 1;
        }

        let sep2 = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep2.set_margin_top(2);
        sep2.set_margin_bottom(2);
        grid.attach(&sep2, 0, row, 4, 1);
        row += 1;

        let backend_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();
        backend_box.append(&backend_system);
        backend_box.append(&backend_wezterm);
        let lbl_backend = gtk::Label::new(Some("Backend"));
        lbl_backend.set_halign(gtk::Align::End);
        lbl_backend.set_valign(gtk::Align::Center);
        grid.attach(&lbl_backend, 0, row, 1, 1);
        grid.attach(&backend_box, 1, row, 3, 1);
        row += 1;

        grid.attach(&accept_new_host, 1, row, 3, 1);
        row += 1;

        let lbl_notes = gtk::Label::new(Some("Notes"));
        lbl_notes.set_halign(gtk::Align::End);
        lbl_notes.set_valign(gtk::Align::Start);
        grid.attach(&lbl_notes, 0, row, 1, 1);
        grid.attach(&note_scroll, 1, row, 3, 1);

        editor.append(&grid);

        let save_draft_btn = gtk::Button::with_label("Save");
        save_draft_btn.add_css_class("connect-button");
        let cancel_draft_btn = gtk::Button::with_label("Cancel");

        let btn_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::End)
            .margin_top(4)
            .build();
        btn_row.append(&cancel_draft_btn);
        btn_row.append(&save_draft_btn);
        editor.append(&btn_row);

        let editor_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&editor)
            .build();

        editor_dialog.set_child(Some(&editor_scroll));

        sidebar.append(&sidebar_header);
        sidebar.append(&sidebar_toolbar);
        sidebar.append(&connection_scroll);
        sidebar_revealer.set_child(Some(&sidebar));

        let right_vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let tab_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(1)
            .build();
        tab_bar.add_css_class("tab-bar");

        let terminal_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        terminal_container.add_css_class("terminal-container");

        let status_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .build();
        status_bar.add_css_class("status-bar");
        let status_label = gtk::Label::new(Some("Ready"));
        status_label.set_halign(gtk::Align::Start);
        status_label.set_hexpand(true);
        status_label.add_css_class("status-label");
        status_bar.append(&status_label);

        right_vbox.append(&tab_bar);
        right_vbox.append(&terminal_container);
        right_vbox.append(&status_bar);

        main_paned.set_start_child(Some(&sidebar_revealer));
        main_paned.set_end_child(Some(&right_vbox));

        root_vbox.append(&main_paned);
        window.set_child(Some(&root_vbox));

        {
            let s = sender.clone();
            connect_btn.connect_clicked(move |_| {
                s.input(AppMsg::LaunchSelected);
            });
        }
        {
            let s = sender.clone();
            split_h_btn.connect_clicked(move |_| {
                s.input(AppMsg::SplitHorizontal);
            });
        }
        {
            let s = sender.clone();
            split_v_btn.connect_clicked(move |_| {
                s.input(AppMsg::SplitVertical);
            });
        }
        {
            let s = sender.clone();
            close_pane_btn.connect_clicked(move |_| {
                s.input(AppMsg::ClosePane);
            });
        }
        {
            let s = sender.clone();
            btn_new.connect_clicked(move |_| {
                s.input(AppMsg::NewConnection);
            });
        }
        {
            let s = sender.clone();
            btn_edit.connect_clicked(move |_| {
                s.input(AppMsg::ToggleEditor);
            });
        }
        {
            let s = sender.clone();
            btn_del.connect_clicked(move |_| {
                s.input(AppMsg::DeleteSelected);
            });
        }
        {
            let s = sender.clone();
            save_draft_btn.connect_clicked(move |_| {
                s.input(AppMsg::SaveDraft);
            });
        }
        {
            let s = sender.clone();
            cancel_draft_btn.connect_clicked(move |_| {
                s.input(AppMsg::ToggleEditor);
            });
        }
        {
            let s = sender.clone();
            editor_dialog.connect_close_request(move |_| {
                s.input(AppMsg::ToggleEditor);
                glib::Propagation::Stop
            });
        }
        {
            let s = sender.clone();
            connection_list.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    if let Some(id) = row.tooltip_text() {
                        if let Ok(id) = Uuid::parse_str(id.as_str()) {
                            s.input(AppMsg::SelectConnection(id));
                        }
                    }
                }
            });
        }
        {
            let s = sender.clone();
            connection_list.connect_row_activated(move |_, row| {
                if let Some(id) = row.tooltip_text() {
                    if let Ok(id) = Uuid::parse_str(id.as_str()) {
                        s.input(AppMsg::SelectConnection(id));
                        s.input(AppMsg::LaunchSelected);
                    }
                }
            });
        }

        {
            let s = sender.clone();
            draft_name.connect_changed(move |e| {
                s.input(AppMsg::DraftNameChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_folder.connect_changed(move |e| {
                s.input(AppMsg::DraftFolderChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_host.connect_changed(move |e| {
                s.input(AppMsg::DraftHostChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_port.connect_value_changed(move |e| {
                s.input(AppMsg::DraftPortChanged(e.value() as u16));
            });
        }
        {
            let s = sender.clone();
            draft_user.connect_changed(move |e| {
                s.input(AppMsg::DraftUserChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_password.connect_changed(move |e| {
                s.input(AppMsg::DraftPasswordChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_identity.connect_changed(move |e| {
                s.input(AppMsg::DraftIdentityChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            draft_command.connect_changed(move |e| {
                s.input(AppMsg::DraftCommandChanged(e.text().to_string()));
            });
        }
        {
            let s = sender.clone();
            accept_new_host.connect_toggled(move |b| {
                s.input(AppMsg::DraftAcceptNewHostChanged(b.is_active()));
            });
        }
        {
            let s = sender.clone();
            backend_system.connect_toggled(move |b| {
                if b.is_active() {
                    s.input(AppMsg::DraftBackendChanged(
                        ConnectionBackend::SystemOpenSsh,
                    ));
                }
            });
        }
        {
            let s = sender.clone();
            backend_wezterm.connect_toggled(move |b| {
                if b.is_active() {
                    s.input(AppMsg::DraftBackendChanged(ConnectionBackend::WezTermSsh));
                }
            });
        }
        {
            let s = sender.clone();
            let note_buf = draft_note.buffer();
            note_buf.connect_changed(move |buf| {
                let txt = buf.text(&buf.start_iter(), &buf.end_iter(), true);
                s.input(AppMsg::DraftNoteChanged(txt.to_string()));
            });
        }
        {
            let s = sender.clone();
            window.connect_close_request(move |_| {
                s.input(AppMsg::ShutdownAll);
                glib::Propagation::Proceed
            });
        }

        glib::timeout_add_local(std::time::Duration::from_millis(250), {
            let s = sender.clone();
            move || {
                s.input(AppMsg::RefreshSessions);
                glib::ControlFlow::Continue
            }
        });

        let widgets = AppWidgets {
            sidebar_revealer,
            connection_list,
            editor_dialog,
            draft_name,
            draft_folder,
            draft_host,
            draft_port,
            draft_user,
            draft_password,
            draft_identity,
            draft_command,
            draft_note,
            accept_new_host,
            backend_system,
            backend_wezterm,
            connect_btn,
            split_h_btn,
            split_v_btn,
            close_pane_btn,
            tab_bar,
            terminal_container,
            pane_views: Vec::new(),
            pane_sizes: Vec::new(),
            status_label,
            toast_label,
        };

        let mut parts = ComponentParts { model, widgets };
        relm4::SimpleComponent::update_view(&parts.model, &mut parts.widgets, sender);
        parts
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        self.update_impl(message, &sender);
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        self.view_impl(widgets, sender);
    }
}
