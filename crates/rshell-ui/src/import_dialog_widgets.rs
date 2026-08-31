use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{ImportDialog, ImportDialogMsg};
use rshell_core::ImportSourceKind;

pub struct ImportDialogWidgets {
    pub root: gtk::Box,
    pub source: gtk::Label,
    pub groups: gtk::Label,
    pub candidates: gtk::Box,
    pub warnings: gtk::Box,
    pub result: gtk::Label,
    pub error: gtk::Label,
    pub legacy: gtk::Button,
    pub openssh: gtk::Button,
    pub retry: gtk::Button,
    pub commit: gtk::Button,
}

impl ImportDialogWidgets {
    pub fn build(root: &gtk::Box, sender: &ComponentSender<ImportDialog>) -> Self {
        let title = gtk::Label::new(Some("Import connections"));
        title.add_css_class("title-2");
        title.add_css_class("dialog-header");
        title.set_halign(gtk::Align::Start);
        root.append(&title);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("dialog-body");
        body.append(&section("Source"));
        let source_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let legacy = gtk::Button::with_label("Legacy rsHell JSON");
        legacy.add_css_class("modal-focus-first");
        let openssh = gtk::Button::with_label("OpenSSH config");
        legacy.set_tooltip_text(Some("Choose a legacy connections JSON file"));
        openssh.set_tooltip_text(Some("Choose an OpenSSH configuration file"));
        source_actions.append(&legacy);
        source_actions.append(&openssh);
        body.append(&source_actions);

        body.append(&section("Preview"));
        let source = gtk::Label::new(None);
        source.set_halign(gtk::Align::Start);
        source.add_css_class("dim-label");
        body.append(&source);
        let groups = gtk::Label::new(None);
        groups.set_halign(gtk::Align::Start);
        groups.set_wrap(true);
        body.append(&groups);

        let candidates = gtk::Box::new(gtk::Orientation::Vertical, 4);
        candidates.add_css_class("import-candidates");
        body.append(&candidates);
        let warnings = gtk::Box::new(gtk::Orientation::Vertical, 4);
        warnings.add_css_class("import-warnings");
        body.append(&warnings);
        body.append(&section("Result"));
        let result = gtk::Label::new(None);
        result.set_halign(gtk::Align::Start);
        result.add_css_class("import-result");
        body.append(&result);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&body)
            .build();
        root.append(&scroll);
        let error = gtk::Label::new(None);
        error.set_halign(gtk::Align::Start);
        error.set_wrap(true);
        error.add_css_class("import-error");
        error.add_css_class("dialog-error");
        root.append(&error);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.add_css_class("dialog-footer");
        actions.set_halign(gtk::Align::End);
        let close = gtk::Button::with_label("Cancel");
        close.add_css_class("modal-focus-last");
        let retry = gtk::Button::with_label("Preview again");
        let commit = gtk::Button::with_label("Import selected");
        commit.add_css_class("suggested-action");
        actions.append(&retry);
        actions.append(&commit);
        actions.append(&close);
        root.append(&actions);
        connect(&legacy, sender, || {
            ImportDialogMsg::Choose(ImportSourceKind::LegacyRshellJson)
        });
        connect(&openssh, sender, || {
            ImportDialogMsg::Choose(ImportSourceKind::OpenSshConfig)
        });
        connect(&close, sender, || ImportDialogMsg::Close);
        connect(&retry, sender, || ImportDialogMsg::Retry);
        connect(&commit, sender, || ImportDialogMsg::Commit);
        Self {
            root: root.clone(),
            source,
            groups,
            candidates,
            warnings,
            result,
            error,
            legacy,
            openssh,
            retry,
            commit,
        }
    }
}

fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dialog-section");
    label.set_halign(gtk::Align::Start);
    label
}

fn connect(
    button: &gtk::Button,
    sender: &ComponentSender<ImportDialog>,
    message: impl Fn() -> ImportDialogMsg + 'static,
) {
    let input = sender.input_sender().clone();
    button.connect_clicked(move |_| {
        let _ = input.send(message());
    });
}
