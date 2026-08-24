use std::collections::BTreeMap;

use gtk::{gdk, prelude::*};
use relm4::{
    ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent, gtk,
};
use rshell_core::{
    AppViewModel, ConnectionId, PaneId, SessionId, SessionUiEvent, TabId, UiCommand, UiPortError,
};
use rshell_platform::ClipboardPolicy;

use crate::{
    PaneAction, PaneHostInit, PaneHostModel, TerminalView, TerminalViewMsg, TerminalViewOutput,
    pane_host_render::render_projection,
    pane_host_terminals::{detach_terminals, send_active_terminal, sync_terminals},
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
    CommandRejected(UiPortError),
}

#[derive(Debug)]
pub enum PaneHostOutput {
    Command(Box<UiCommand>),
    EditConnection(ConnectionId),
    ActiveTab(TabId),
    ClipboardWritten { bytes: usize },
    Error(&'static str),
}

pub struct PaneHost {
    model: PaneHostModel,
    terminals: BTreeMap<SessionId, Controller<TerminalView>>,
    content: gtk::Box,
    clipboard: gdk::Clipboard,
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
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
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
            content,
            clipboard: root.display().clipboard(),
        };
        sync_terminals(&mut model.model, &mut model.terminals, &sender);
        model.render(&sender);
        ComponentParts {
            model,
            widgets: PaneHostWidgets { status },
        }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            PaneHostMsg::SetViewModel(view_model) => {
                detach_terminals(&self.terminals);
                clear_box(&self.content);
                self.model.replace_view_model(*view_model);
                sync_terminals(&mut self.model, &mut self.terminals, &sender);
            }
            PaneHostMsg::ActivateTab(tab) => {
                if self.model.activate_tab(tab) {
                    let _ = sender.output(PaneHostOutput::ActiveTab(tab));
                }
            }
            PaneHostMsg::ActivatePane(pane) => {
                self.model.activate_pane(pane);
            }
            PaneHostMsg::Action { pane, action } => self.handle_action(pane, action, &sender),
            PaneHostMsg::Connect { connection } => self.connect_active(connection, &sender),
            PaneHostMsg::ActiveTerminal(message) => {
                send_active_terminal(&self.model, &self.terminals, message, &sender)
            }
            PaneHostMsg::SessionEvent { session, event } => {
                if self.model.apply_session_event(session, event.clone()) {
                    sync_terminals(&mut self.model, &mut self.terminals, &sender);
                    if let Some(terminal) = self.terminals.get(&session) {
                        match event {
                            SessionUiEvent::Frame(frame) => {
                                terminal.emit(TerminalViewMsg::ApplyFrame(frame));
                            }
                            event => terminal.emit(TerminalViewMsg::SessionEvent(event)),
                        }
                    }
                }
            }
            PaneHostMsg::Terminal(_, TerminalViewOutput::Command(command)) => {
                let _ = sender.output(PaneHostOutput::Command(command));
            }
            PaneHostMsg::Terminal(_, TerminalViewOutput::Error(_)) => {
                let _ = sender.output(PaneHostOutput::Error("terminal input was rejected"));
            }
            PaneHostMsg::Terminal(_, TerminalViewOutput::ClipboardWritten { bytes }) => {
                let _ = sender.output(PaneHostOutput::ClipboardWritten { bytes });
            }
            PaneHostMsg::CommandRejected(error) => self.model.command_rejected(error),
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        self.render(&sender);
        widgets.status.set_label(self.model.status().unwrap_or(""));
        widgets.status.set_visible(self.model.status().is_some());
    }
}

impl PaneHost {
    fn connect_active(&self, connection: ConnectionId, sender: &ComponentSender<Self>) {
        let Some(tab) = self.model.active_tab() else {
            let _ = sender.output(PaneHostOutput::Error("no active tab"));
            return;
        };
        let Some(pane) = self.model.active_pane(tab) else {
            let _ = sender.output(PaneHostOutput::Error("no active pane"));
            return;
        };
        let _ = sender.output(PaneHostOutput::Command(Box::new(UiCommand::Connect {
            pane,
            connection,
        })));
    }

    fn render(&self, sender: &ComponentSender<Self>) {
        detach_terminals(&self.terminals);
        clear_box(&self.content);
        let Some(tab) = self.model.active_tab() else {
            let empty = gtk::Label::new(Some("No terminal tabs"));
            empty.add_css_class("pane-state-label");
            self.content.append(&empty);
            return;
        };
        if let Some(projection) = self.model.projection(tab) {
            let active = self.model.active_pane(tab);
            self.content.append(&render_projection(
                &projection,
                active,
                &self.terminals,
                sender,
            ));
        }
    }

    fn handle_action(
        &mut self,
        pane_id: PaneId,
        action: PaneAction,
        sender: &ComponentSender<Self>,
    ) {
        let Some(pane) = self.model.pane(pane_id) else {
            return;
        };
        match action {
            PaneAction::EditConnection => {
                if let Some(connection) = pane.connection_id() {
                    let _ = sender.output(PaneHostOutput::EditConnection(connection));
                }
            }
            PaneAction::CopyDiagnostics => {
                let Some(diagnostics) = pane.diagnostics() else {
                    return;
                };
                match ClipboardPolicy::normalize_text(&diagnostics) {
                    Ok(diagnostics) => self.clipboard.set_text(&diagnostics),
                    Err(_) => {
                        let _ =
                            sender.output(PaneHostOutput::Error("diagnostics copy was rejected"));
                    }
                }
            }
            other => {
                if let Some(command) = other.command(pane_id, pane.session()) {
                    let _ = sender.output(PaneHostOutput::Command(Box::new(command)));
                }
            }
        }
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
