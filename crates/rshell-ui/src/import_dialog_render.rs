use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::ImportSourceKind;

use crate::{
    IconRenderRequest, ImportDialog, ImportDialogMsg, ProductIcon,
    import_dialog_widgets::ImportDialogWidgets,
};

pub fn render_import(
    model: &ImportDialog,
    widgets: &mut ImportDialogWidgets,
    sender: &ComponentSender<ImportDialog>,
) {
    widgets.source.set_label(match model.view.source() {
        Some(ImportSourceKind::LegacyRshellJson) => "Preview source: legacy rsHell JSON",
        Some(ImportSourceKind::OpenSshConfig) => "Preview source: OpenSSH config",
        None => "Choose an import source to build a preview",
    });
    let groups = model
        .view
        .groups()
        .iter()
        .map(|group| group.name.as_str())
        .collect::<Vec<_>>();
    let group_summary = if groups.is_empty() {
        "No groups in preview".to_owned()
    } else {
        format!("Groups: {}", groups.join(", "))
    };
    widgets.groups.set_label(&group_summary);
    clear(&widgets.candidates);
    for candidate in model.view.candidates() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("import-candidate");
        let select = gtk::CheckButton::new();
        select.set_active(model.view.is_selected(candidate.id));
        select.set_sensitive(candidate.selectable && !model.view.is_pending());
        select.update_property(&[gtk::accessible::Property::Label(&format!(
            "Import {}",
            candidate.name
        ))]);
        let id = candidate.id;
        let input = sender.input_sender().clone();
        select.connect_toggled(move |toggle| {
            let _ = input.send(ImportDialogMsg::Toggle(id, toggle.is_active()));
        });
        row.append(&select);
        let details = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let name = gtk::Label::new(Some(&candidate.name));
        name.set_halign(gtk::Align::Start);
        name.add_css_class("heading");
        details.append(&name);
        let endpoint = gtk::Label::new(Some(&format!(
            "{}@{}:{}",
            candidate.username, candidate.host, candidate.port
        )));
        endpoint.set_halign(gtk::Align::Start);
        endpoint.add_css_class("dim-label");
        details.append(&endpoint);
        if candidate
            .warnings
            .iter()
            .any(|warning| warning.code.to_ascii_lowercase().contains("proxyjump"))
        {
            let marker = gtk::Label::new(Some("Uses system OpenSSH config"));
            marker.set_halign(gtk::Align::Start);
            details.append(&marker);
        }
        row.append(&details);
        let icon_request = IconRenderRequest::for_widget(16, &row);
        if candidate.has_secret {
            let secret = ProductIcon::SecretPresent
                .image(icon_request)
                .expect("embedded secret-present icon");
            secret.set_tooltip_text(Some("Secret is present; value is never displayed"));
            secret.update_property(&[gtk::accessible::Property::Label("Secret present")]);
            row.append(&secret);
        }
        if !candidate.selectable {
            let warning = ProductIcon::Warning
                .image(icon_request)
                .expect("embedded warning icon");
            warning.set_tooltip_text(Some("This candidate cannot be imported"));
            warning.update_property(&[gtk::accessible::Property::Label(
                "Candidate cannot be imported",
            )]);
            row.append(&warning);
        }
        widgets.candidates.append(&row);
    }
    clear(&widgets.warnings);
    for warning in model.view.visible_warnings() {
        let label = gtk::Label::new(Some(&warning.message));
        label.set_halign(gtk::Align::Start);
        label.set_wrap(true);
        label.add_css_class("warning");
        widgets.warnings.append(&label);
    }
    if let Some(report) = model.view.report() {
        widgets.result.set_label(&format!(
            "Imported {} groups and {} connections; skipped {}",
            report.imported_groups, report.imported_connections, report.skipped_candidates
        ));
    } else {
        widgets.result.set_label("");
    }
    widgets.error.set_label(model.view.error().unwrap_or(""));
    widgets.error.set_visible(model.view.error().is_some());
    widgets.retry.set_visible(model.view.error().is_some());
    widgets
        .retry
        .set_sensitive(!model.selecting && !model.view.is_pending());
    widgets.commit.set_sensitive(model.view.can_commit());
    widgets
        .legacy
        .set_sensitive(!model.selecting && !model.view.is_pending());
    widgets
        .openssh
        .set_sensitive(!model.selecting && !model.view.is_pending());
    widgets.root.set_visible(model.visible);
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
