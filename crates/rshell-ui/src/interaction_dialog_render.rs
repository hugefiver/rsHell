use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::{AuthPrompt, InteractionRequest};

use crate::{
    IconRenderRequest, InteractionAction, InteractionDialog, InteractionDialogMsg, ProductIcon,
    interaction_dialog_widgets::InteractionDialogWidgets,
};

pub fn render_interaction(
    model: &InteractionDialog,
    root: &gtk::Box,
    widgets: &mut InteractionDialogWidgets,
    sender: &ComponentSender<InteractionDialog>,
) {
    root.set_visible(model.visible);
    let Some(view) = &model.view else {
        return;
    };
    if widgets.rendered != Some(view.interaction_id()) {
        clear(&widgets.prompts);
        clear(&widgets.actions);
        widgets.inputs.clear();
        match view.request() {
            InteractionRequest::HostKey(prompt) => {
                widgets.title.set_label(if prompt.changed {
                    "Host key changed"
                } else {
                    "Verify host key"
                });
                widgets.summary.set_label(&format!(
                    "{}\n{}",
                    view.endpoint().unwrap_or(""),
                    view.fingerprint().unwrap_or("")
                ));
                if prompt.changed {
                    widgets.summary.add_css_class("danger");
                } else {
                    widgets.summary.remove_css_class("danger");
                }
            }
            InteractionRequest::Password(prompt) => {
                widgets.title.set_label("Password required");
                widgets.summary.set_label(&prompt.label);
                add_prompt(&widgets.prompts, &mut widgets.inputs, prompt, 0, sender);
            }
            InteractionRequest::PrivateKeyPassphrase(prompt) => {
                widgets.title.set_label("Private key passphrase required");
                widgets.summary.set_label(&prompt.label);
                add_prompt(&widgets.prompts, &mut widgets.inputs, prompt, 0, sender);
            }
            InteractionRequest::KeyboardInteractive(prompt) => {
                widgets.title.set_label(&prompt.name);
                widgets.summary.set_label(&prompt.instruction);
                for (index, prompt) in prompt.prompts.iter().enumerate() {
                    add_prompt(&widgets.prompts, &mut widgets.inputs, prompt, index, sender);
                }
            }
        }
        for action in view.actions() {
            let button = action_button(*action);
            let input = sender.input_sender().clone();
            let action = *action;
            button.connect_clicked(move |_| {
                let _ = input.send(InteractionDialogMsg::Action(action));
            });
            widgets.actions.append(&button);
        }
        let first = widgets
            .inputs
            .first()
            .cloned()
            .or_else(|| widgets.actions.first_child());
        if let Some(first) = first.as_ref() {
            first.add_css_class("modal-focus-first");
        }
        if let Some(last) = widgets.actions.last_child() {
            last.add_css_class("modal-focus-last");
        }
        widgets.rendered = Some(view.interaction_id());
        if model.visible
            && let Some(first) = first
        {
            first.grab_focus();
            let frame_focus = first.clone();
            root.add_tick_callback(move |_, _| {
                frame_focus.grab_focus();
                gtk::glib::ControlFlow::Break
            });
            gtk::glib::idle_add_local_once(move || {
                first.grab_focus();
            });
        }
    }
    widgets
        .error
        .set_label(model.error.as_deref().unwrap_or(""));
    widgets.error.set_visible(model.error.is_some());
    widgets.actions.set_sensitive(!model.pending);
    widgets.prompts.set_sensitive(!model.pending);
}

fn add_prompt(
    container: &gtk::Box,
    inputs: &mut Vec<gtk::Widget>,
    prompt: &AuthPrompt,
    index: usize,
    sender: &ComponentSender<InteractionDialog>,
) {
    let label = gtk::Label::new(Some(&prompt.label));
    label.set_halign(gtk::Align::Start);
    container.append(&label);
    if prompt.echo {
        let entry = gtk::Entry::new();
        entry.update_property(&[gtk::accessible::Property::Label(&prompt.label)]);
        connect_editable(&entry, index, sender);
        container.append(&entry);
        inputs.push(entry.upcast());
    } else {
        let entry = gtk::PasswordEntry::new();
        entry.set_show_peek_icon(false);
        entry.update_property(&[gtk::accessible::Property::Label(&prompt.label)]);
        connect_editable(&entry, index, sender);
        container.append(&entry);
        inputs.push(entry.upcast());
    }
}

fn connect_editable(
    entry: &impl IsA<gtk::Editable>,
    index: usize,
    sender: &ComponentSender<InteractionDialog>,
) {
    let input = sender.input_sender().clone();
    entry.connect_changed(move |entry| {
        let _ = input.send(InteractionDialogMsg::Answer(index, entry.text().into()));
    });
}

fn action_button(action: InteractionAction) -> gtk::Button {
    let (label, icon) = match action {
        InteractionAction::Reject => ("Reject", ProductIcon::Warning),
        InteractionAction::AcceptAndStore => ("Accept and store", ProductIcon::HostTrust),
        InteractionAction::CopyDiagnostics => ("Copy diagnostics", ProductIcon::CopyDiagnostics),
        InteractionAction::Close => ("Close", ProductIcon::CloseTab),
        InteractionAction::Submit => ("Submit", ProductIcon::HostTrust),
        InteractionAction::Cancel => ("Cancel", ProductIcon::CloseTab),
    };
    let button = gtk::Button::new();
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    content.append(
        &icon
            .image(IconRenderRequest::for_widget(16, &button))
            .expect("embedded interaction icon"),
    );
    content.append(&gtk::Label::new(Some(label)));
    button.set_child(Some(&content));
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    if matches!(
        action,
        InteractionAction::AcceptAndStore | InteractionAction::Submit
    ) {
        button.add_css_class("suggested-action");
    }
    button
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
