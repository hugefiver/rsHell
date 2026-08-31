use gtk::prelude::*;
use relm4::{ComponentController, gtk};

use crate::{
    ConnectionEditorMsg, ImportDialogMsg, InteractionDialogMsg, MainWindow, ModalKind,
    ModalRequest, SettingsWindowMsg,
};

impl MainWindow {
    pub(crate) fn resize_window(&mut self, width: i32) {
        let realized = self.smoke.as_ref().and_then(|_| {
            self.shell
                .overlay
                .root()
                .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
                .and_then(|window| crate::main_window_smoke_resize::window_surface_size(&window))
        });
        let (layout_width, height) = realized.unwrap_or((width, self.shell.overlay.height()));
        self.apply_shell_layout(layout_width);
        self.modal.resize(layout_width);
        self.observe_smoke_window_allocation(layout_width, height);
    }

    pub(crate) fn handle_modal(&mut self, request: ModalRequest) {
        match request {
            ModalRequest::Open { kind, trigger } => {
                let surface = self.modal_surface(kind);
                self.modal.open(kind, &surface, &trigger);
                self.restore_sidebar_selection();
                match kind {
                    ModalKind::Settings => self.send_settings(SettingsWindowMsg::Open),
                    ModalKind::Import => self.send_import(ImportDialogMsg::Open),
                    ModalKind::ConnectionEditor | ModalKind::Interaction => {}
                }
            }
            ModalRequest::Close(kind) => self.modal.close(kind),
        }
    }

    pub(crate) fn open_editor(&mut self, message: ConnectionEditorMsg) {
        self.open_with_current_focus(ModalKind::ConnectionEditor);
        self.send_editor(message);
    }

    pub(crate) fn open_interaction(&mut self, message: InteractionDialogMsg) {
        self.open_with_current_focus(ModalKind::Interaction);
        self.send_interaction(message);
    }

    pub(crate) fn open_settings(&mut self) {
        self.open_with_current_focus(ModalKind::Settings);
    }

    pub(crate) fn open_import(&mut self) {
        self.open_with_current_focus(ModalKind::Import);
    }

    fn open_with_current_focus(&mut self, kind: ModalKind) {
        let trigger = self.modal_trigger();
        if self.smoke_state.visual_checkpoint
            == crate::main_window_smoke_visual::VisualCheckpointPhase::Opening
        {
            self.smoke_state.visual_focus_trigger = Some(trigger.clone());
        }
        self.handle_modal(ModalRequest::Open { kind, trigger });
    }

    fn modal_surface(&self, kind: ModalKind) -> gtk::Widget {
        match kind {
            ModalKind::ConnectionEditor => self.editor.widget().clone().upcast(),
            ModalKind::Settings => self.dialogs.settings.widget().clone().upcast(),
            ModalKind::Import => self.dialogs.import.widget().clone().upcast(),
            ModalKind::Interaction => self.dialogs.interaction.widget().clone().upcast(),
        }
    }

    fn modal_trigger(&self) -> gtk::Widget {
        self.shell
            .overlay
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
            .or_else(|| find_terminal(self.shell.background.upcast_ref()))
            .unwrap_or_else(|| self.shell.terminal_workspace.clone().upcast())
    }
}

fn find_terminal(root: &gtk::Widget) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(current) = child {
        if current.has_css_class("terminal-canvas") {
            return Some(current);
        }
        if let Some(found) = find_terminal(&current) {
            return Some(found);
        }
        child = current.next_sibling();
    }
    None
}

impl Drop for MainWindow {
    fn drop(&mut self) {
        for forwarder in &self.live_forwarders {
            forwarder.abort();
        }
    }
}
