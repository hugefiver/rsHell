use std::rc::Rc;

use gtk::prelude::*;
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_platform::{FileSelectionRequest, FileSelectionService};

pub use crate::import_dialog_message::{
    ImportDialogInit, ImportDialogMsg, ImportDialogOutput, ImportDialogState,
};
use crate::{
    ImportViewModel, import_dialog_render::render_import,
    import_dialog_widgets::ImportDialogWidgets,
};

pub struct ImportDialog {
    pub(crate) view: ImportViewModel,
    pub(crate) file_selection: Rc<dyn FileSelectionService>,
    pub(crate) visible: bool,
    pub(crate) selecting: bool,
    pub(crate) selection_generation: u64,
    pub(crate) active_selection: Option<u64>,
    pub(crate) closed_notified: bool,
    pub(crate) revision: u64,
}

impl SimpleComponent for ImportDialog {
    type Init = ImportDialogInit;
    type Input = ImportDialogMsg;
    type Output = ImportDialogOutput;
    type Root = gtk::Box;
    type Widgets = ImportDialogWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        root.add_css_class("import-dialog");
        root.add_css_class("content-dialog");
        root.set_width_request(620);
        root.set_halign(gtk::Align::Center);
        root.set_valign(gtk::Align::Center);
        root.set_visible(false);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            view: ImportViewModel::empty(),
            file_selection: init.file_selection,
            visible: false,
            selecting: false,
            selection_generation: 0,
            active_selection: None,
            closed_notified: false,
            revision: 0,
        };
        let mut widgets = ImportDialogWidgets::build(&root, &sender);
        attach_keys(&root, &sender);
        render_import(&model, &mut widgets, &sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            ImportDialogMsg::Open => {
                self.invalidate_selection();
                self.visible = true;
                self.closed_notified = false;
            }
            ImportDialogMsg::Choose(source) => {
                if self.selecting {
                    return;
                }
                let token = self.next_selection();
                self.selecting = true;
                let request = match source {
                    rshell_core::ImportSourceKind::LegacyRshellJson => {
                        FileSelectionRequest::legacy_import()
                    }
                    rshell_core::ImportSourceKind::OpenSshConfig => {
                        FileSelectionRequest::open_ssh_import()
                    }
                };
                let input = sender.input_sender().clone();
                self.file_selection.select_file(
                    request,
                    Box::new(move |result| {
                        let _ = input.send(ImportDialogMsg::FileSelected(token, source, result));
                    }),
                );
            }
            ImportDialogMsg::FileSelected(token, source, result) => {
                if self.active_selection != Some(token) || !self.visible {
                    return;
                }
                self.active_selection = None;
                self.selecting = false;
                match result {
                    Ok(Some(path)) => {
                        self.view.remember_source(source, path);
                        self.output(self.view.preview_command(), &sender);
                    }
                    Ok(None) => {}
                    Err(_) => self.view.failed("file selection failed"),
                }
            }
            ImportDialogMsg::PreviewPath(source, path) => {
                self.invalidate_selection();
                self.visible = true;
                self.closed_notified = false;
                self.view.remember_source(source, path);
                self.output(self.view.preview_command(), &sender);
            }
            ImportDialogMsg::Preview(preview) => {
                self.invalidate_selection();
                self.visible = true;
                self.view.accept_preview(preview);
            }
            ImportDialogMsg::Toggle(id, selected) => self.view.set_selected(id, selected),
            ImportDialogMsg::Commit => {
                let command = self.view.begin_commit();
                self.output(command, &sender);
            }
            ImportDialogMsg::Retry => {
                let command = self.view.retry_command();
                self.output(command, &sender);
            }
            ImportDialogMsg::Close => {
                self.invalidate_selection();
                let command = self.view.cancel_command();
                self.output(command, &sender);
                self.visible = false;
                if !self.closed_notified {
                    self.closed_notified = true;
                    let _ = sender.output(ImportDialogOutput::Closed);
                }
            }
            ImportDialogMsg::Completed(report) => {
                self.invalidate_selection();
                self.view.completed(report);
            }
            ImportDialogMsg::Cancelled(preview) => {
                self.view.cancelled(preview);
                self.invalidate_selection();
                let notify = self.visible && !self.closed_notified;
                self.visible = false;
                if notify {
                    self.closed_notified = true;
                    let _ = sender.output(ImportDialogOutput::Closed);
                }
            }
            ImportDialogMsg::OperationFailed(failure) => {
                if failure.context == "import preview expired" {
                    self.view.expired();
                } else {
                    self.view.failed(failure.context);
                }
            }
            ImportDialogMsg::CommandRejected(error) => {
                self.view.failed(if error.to_string().is_empty() {
                    "command could not be sent"
                } else {
                    "command queue unavailable"
                });
            }
        }
        self.revision = self.revision.saturating_add(1);
        let _ = sender.output(ImportDialogOutput::StateChanged(
            crate::import_dialog_message::ImportDialogState {
                visible: self.visible,
                pending: self.view.is_pending(),
                preview_ready: self.view.preview_id().is_some(),
                has_error: self.view.error().is_some(),
                revision: self.revision,
            },
        ));
    }

    fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>) {
        render_import(self, widgets, &sender);
    }
}

impl ImportDialog {
    fn output(&self, command: Option<rshell_core::UiCommand>, sender: &ComponentSender<Self>) {
        if let Some(command) = command {
            let _ = sender.output(ImportDialogOutput::Command(Box::new(command)));
        }
    }

    fn next_selection(&mut self) -> u64 {
        self.selection_generation = self.selection_generation.saturating_add(1);
        self.active_selection = Some(self.selection_generation);
        self.selection_generation
    }

    fn invalidate_selection(&mut self) {
        self.selection_generation = self.selection_generation.saturating_add(1);
        self.active_selection = None;
        self.selecting = false;
    }
}

fn attach_keys(root: &gtk::Box, sender: &ComponentSender<ImportDialog>) {
    let keys = gtk::EventControllerKey::new();
    let input = sender.input_sender().clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            let _ = input.send(ImportDialogMsg::Close);
            return gtk::glib::Propagation::Stop;
        }
        if key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter {
            let _ = input.send(ImportDialogMsg::Commit);
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    root.add_controller(keys);
}
