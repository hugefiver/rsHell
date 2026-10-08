use std::{cell::Cell, collections::BTreeMap};

#[path = "pane_host_focus.rs"]
mod focus;

use gtk::{gdk, prelude::*};
use relm4::{ComponentParts, ComponentSender, Controller, SimpleComponent, gtk};
use rshell_core::{
    AppViewModel, ConnectionId, PaneId, SessionId, SessionUiEvent, TabId, UiCommand, UiPortError,
};

use crate::{
    PaneAction, PaneHostInit, PaneHostModel, TerminalView, TerminalViewMsg, TerminalViewOutput,
    pane_host_commands::{connect_active, handle_action},
    pane_host_geometry::{PaneHostGeometryAck, forward_terminal_command},
    pane_host_refresh::{active_terminals_changed, projection_changed, session_is_active},
    pane_host_terminals::{
        rendered_session, send_active_terminal, send_terminal_message, sync_terminals,
    },
};

#[derive(Debug)]
pub enum PaneHostMsg {
    SetViewModel(Box<AppViewModel>),
    ActivateTab(TabId),
    ActivatePane(PaneId),
    Connect {
        connection: ConnectionId,
    },
    ActiveTerminal(TerminalViewMsg),
    Action {
        pane: PaneId,
        action: PaneAction,
    },
    SessionEvent {
        session: SessionId,
        event: SessionUiEvent,
    },
    Terminal(SessionId, TerminalViewOutput),
    RefreshUnacknowledgedGeometry,
    CommandRejected(UiPortError),
}

#[derive(Debug)]
pub enum PaneHostOutput {
    Command(Box<UiCommand>),
    EditConnection(ConnectionId),
    ActiveTab(TabId),
    RenderedSession(Option<SessionId>),
    GeometryReady(SessionId),
    ClipboardWritten { bytes: usize },
    Error(&'static str),
}

pub struct PaneHost {
    model: PaneHostModel,
    terminals: BTreeMap<SessionId, Controller<TerminalView>>,
    content: gtk::Overlay,
    clipboard: gdk::Clipboard,
    geometry: PaneHostGeometryAck,
    render_dirty: Cell<bool>,
    focus: focus::PaneFocus,
}

pub struct PaneHostWidgets {
    status: gtk::Label,
}

impl SimpleComponent for PaneHost {
    type Init = PaneHostInit;
    type Input = PaneHostMsg;
    type Output = PaneHostOutput;
    type Root = gtk::Box;
    type Widgets = PaneHostWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("pane-host");
        root.set_hexpand(true);
        root.set_vexpand(true);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let content = gtk::Overlay::new();
        content.set_hexpand(true);
        content.set_vexpand(true);
        let status = gtk::Label::new(None);
        status.add_css_class("pane-state-label");
        status.set_halign(gtk::Align::Start);
        status.set_visible(false);
        root.append(&content);
        root.append(&status);
        let mut model = Self {
            model: init.into_model(),
            terminals: BTreeMap::new(),
            focus: focus::PaneFocus::new(&content),
            content,
            clipboard: root.display().clipboard(),
            geometry: PaneHostGeometryAck::default(),
            render_dirty: Cell::new(false),
        };
        model.sync_terminal_controllers(&sender);
        model.render(&sender);
        let _ = sender.output(PaneHostOutput::RenderedSession(rendered_session(
            &model.model,
            &model.terminals,
        )));
        ComponentParts {
            model,
            widgets: PaneHostWidgets { status },
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            PaneHostMsg::SetViewModel(view_model) => {
                let render = projection_changed(self.model.view_model(), view_model.as_ref());
                let sync = active_terminals_changed(self.model.view_model(), view_model.as_ref());
                if render {
                    self.focus.capture_owned(self);
                    self.render_dirty.set(true);
                }
                self.model.replace_view_model(*view_model);
                if sync {
                    self.sync_terminal_controllers(&sender);
                }
                self.focus.synchronize(self);
            }
            PaneHostMsg::ActivateTab(tab) => {
                let changed = self.model.active_tab() != Some(tab);
                if self.model.activate_tab(tab) {
                    if changed {
                        self.focus.activate(self);
                        self.focus.capture_owned(self);
                        self.render_dirty.set(true);
                        self.sync_terminal_controllers(&sender);
                    }
                    let _ = sender.output(PaneHostOutput::ActiveTab(tab));
                }
            }
            PaneHostMsg::ActivatePane(pane) => {
                let changed = self
                    .model
                    .active_tab()
                    .and_then(|t| self.model.active_pane(t))
                    != Some(pane);
                if changed && self.model.activate_pane(pane) {
                    self.focus.activate(self);
                    self.focus.capture_owned(self);
                    self.focus.synchronize(self);
                    self.render_dirty.set(true);
                }
            }
            PaneHostMsg::Action { pane, action } => {
                handle_action(&self.model, &self.clipboard, pane, action, &sender)
            }
            PaneHostMsg::Connect { connection } => connect_active(&self.model, connection, &sender),
            PaneHostMsg::ActiveTerminal(message) => {
                send_active_terminal(&self.model, &mut self.terminals, message, &sender)
            }
            PaneHostMsg::SessionEvent { session, event } => {
                let view = self.model.view_model();
                let active = if self.model.active_tab() == view.workspace.active_tab {
                    session_is_active(view, session)
                } else {
                    self.model.active_tab().is_some_and(|tab| {
                        view.workspace
                            .tab(tab)
                            .is_ok_and(|tab| tab.pane_tree.session_ids().contains(&session))
                    })
                };
                if active && !matches!(event, SessionUiEvent::Frame(_)) {
                    self.focus.capture_owned(self);
                }
                if self.model.apply_session_event(session, event.clone()) {
                    if active && !matches!(event, SessionUiEvent::Frame(_)) {
                        self.render_dirty.set(true);
                    }
                    if active {
                        self.sync_terminal_controllers(&sender);
                    }
                    if active
                        && !matches!(event, SessionUiEvent::Frame(_))
                        && let Some(terminal) = self.terminals.get(&session)
                        && !send_terminal_message(
                            terminal,
                            TerminalViewMsg::SessionEvent(event.clone()),
                        )
                    {
                        self.terminals.remove(&session);
                        self.sync_terminal_controllers(&sender);
                        if let Some(replacement) = self.terminals.get(&session) {
                            let _ = send_terminal_message(
                                replacement,
                                TerminalViewMsg::SessionEvent(event),
                            );
                        }
                    }
                }
            }
            PaneHostMsg::Terminal(source, TerminalViewOutput::Command(command)) => {
                if forward_terminal_command(
                    source,
                    command,
                    &self.geometry,
                    &mut self.terminals,
                    &self.content,
                    &self.model,
                    &sender,
                ) {
                    self.sync_terminal_controllers(&sender);
                }
            }
            PaneHostMsg::RefreshUnacknowledgedGeometry => {
                self.geometry.refresh(&mut self.terminals);
                self.geometry.schedule(&self.content, &sender);
            }
            PaneHostMsg::Terminal(_, TerminalViewOutput::Error(_)) => {
                let _ = sender.output(PaneHostOutput::Error("terminal input was rejected"));
            }
            PaneHostMsg::Terminal(_, TerminalViewOutput::ClipboardWritten { bytes }) => {
                let _ = sender.output(PaneHostOutput::ClipboardWritten { bytes });
            }
            PaneHostMsg::CommandRejected(error) => {
                self.focus.cancel();
                self.model.command_rejected(error);
            }
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        if self.render_dirty.replace(false) {
            self.render(&sender);
            let _ = sender.output(PaneHostOutput::RenderedSession(rendered_session(
                &self.model,
                &self.terminals,
            )));
        }
        widgets.status.set_label(self.model.status().unwrap_or(""));
        widgets.status.set_visible(self.model.status().is_some());
    }
}

impl PaneHost {
    fn sync_terminal_controllers(&mut self, sender: &ComponentSender<Self>) {
        let replaced = sync_terminals(&mut self.model, &mut self.terminals, &self.content, sender);
        self.geometry
            .synchronize(self.terminals.keys().copied(), &replaced);
        self.geometry.schedule(&self.content, sender);
        self.focus.synchronize(self);
    }
}
