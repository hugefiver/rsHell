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
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.add_css_class("dialog-body");
        body.append(&section("Trust/auth message"));
        let summary = gtk::Label::new(None);
        summary.set_halign(gtk::Align::Start);
        summary.set_wrap(true);
        body.append(&summary);
        body.append(&section("Required inputs"));
        let prompts = gtk::Box::new(gtk::Orientation::Vertical, 8);
        prompts.add_css_class("interaction-prompts");
        body.append(&prompts);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&body)
            .build();
        root.append(&scroll);
        let error = gtk::Label::new(None);
        error.add_css_class("interaction-error");
        error.add_css_class("dialog-error");
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        root.append(&error);
        let footer = gtk::Box::new(gtk::Orientation::Vertical, 8);
        footer.add_css_class("dialog-footer");
        footer.append(&section("Actions"));
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        footer.append(&actions);
        root.append(&footer);
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

    pub fn park_focus(&self) {
        if let Some(root) = self.title.root()
            && let Ok(window) = root.downcast::<gtk::Window>()
        {
            gtk::prelude::GtkWindowExt::set_focus(&window, gtk::Widget::NONE);
        }
        self.title.set_focusable(true);
        self.title.grab_focus();
    }
}

fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dialog-section");
    label.set_halign(gtk::Align::Start);
    label
}
