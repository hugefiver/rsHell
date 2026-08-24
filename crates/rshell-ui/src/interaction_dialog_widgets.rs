use gtk::prelude::*;
use relm4::gtk;
use rshell_core::InteractionId;

pub struct InteractionDialogWidgets {
    pub title: gtk::Label,
    pub summary: gtk::Label,
    pub prompts: gtk::Box,
    pub actions: gtk::Box,
    pub error: gtk::Label,
    pub rendered: Option<InteractionId>,
    pub inputs: Vec<gtk::Widget>,
}

impl InteractionDialogWidgets {
    pub fn build(root: &gtk::Box) -> Self {
        let title = gtk::Label::new(None);
        title.add_css_class("title-2");
        title.add_css_class("dialog-header");
        title.set_halign(gtk::Align::Start);
        root.append(&title);
        let summary = gtk::Label::new(None);
        summary.set_halign(gtk::Align::Start);
        summary.set_wrap(true);
        root.append(&summary);
        let prompts = gtk::Box::new(gtk::Orientation::Vertical, 8);
        prompts.add_css_class("interaction-prompts");
        prompts.add_css_class("dialog-body");
        root.append(&prompts);
        let error = gtk::Label::new(None);
        error.add_css_class("interaction-error");
        error.add_css_class("dialog-error");
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        root.append(&error);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.add_css_class("dialog-footer");
        actions.set_halign(gtk::Align::End);
        root.append(&actions);
        Self {
            title,
            summary,
            prompts,
            actions,
            error,
            rendered: None,
            inputs: Vec::new(),
        }
    }

    pub fn wipe_inputs(&self) {
        for widget in &self.inputs {
            if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                entry.set_text("");
            } else if let Ok(password) = widget.clone().downcast::<gtk::PasswordEntry>() {
                password.set_text("");
            }
        }
    }
}
