//! Production widget-state regression; not the hosted GTK 4.16.12 sizing oracle.
#![cfg(not(target_os = "macos"))]

use gtk::prelude::*;
use relm4::{Component, ComponentController, Controller};
use rshell_core::{AuthPrompt, InteractionId, InteractionRequest, SessionId};
use rshell_ui::{
    InteractionAction, InteractionDialog, InteractionDialogInit, InteractionDialogMsg,
    apply_global_css,
};

#[path = "support/stage3_prompt_cache_failure.rs"]
mod failure;
#[path = "support/stage3_prompt_replacement_outputs.rs"]
mod outputs;

#[path = "support/fluent_native.rs"]
#[allow(dead_code, unused_imports)]
mod native;

fn open(dialog: &Controller<InteractionDialog>, session: SessionId, id: InteractionId) {
    dialog.emit(InteractionDialogMsg::Open {
        session,
        request: InteractionRequest::Password(AuthPrompt {
            id,
            label: "Synthetic password".into(),
            echo: false,
        }),
    });
}

fn settled(dialog: &Controller<InteractionDialog>) {
    native::wait_for_frame(dialog.widget(), "production widget state", |_| true);
}

fn verify(
    dialog: &Controller<InteractionDialog>,
    window: &gtk::ApplicationWindow,
    outputs: &outputs::Outputs,
) {
    let initial = dialog.widgets().prompts.clone();
    let body = initial.parent().unwrap();
    let previous = initial.prev_sibling();
    let next = initial.next_sibling();
    let old_session = SessionId::new();
    let old_id = InteractionId::new();
    open(dialog, old_session, old_id);
    settled(dialog);
    let old = dialog.widgets().prompts.clone();
    let old_secret = dialog.widgets().inputs[0]
        .clone()
        .downcast::<gtk::PasswordEntry>()
        .unwrap();
    let old_action = dialog.widgets().actions.first_child().unwrap();
    old_secret.set_text("synthetic-old-answer");
    settled(dialog);
    open(dialog, old_session, old_id);
    settled(dialog);
    assert!(dialog.widgets().prompts == old);
    assert!(!old_secret.text().is_empty(), "same ID preserves input");

    let new_session = SessionId::new();
    let new_id = InteractionId::new();
    open(dialog, new_session, new_id);
    settled(dialog);
    assert!(
        dialog.widgets().prompts == old,
        "queued request stays hidden"
    );
    assert!(old_secret.text().is_empty());
    assert!(!old.is_sensitive());
    assert!(!dialog.widgets().actions.is_sensitive());
    dialog.emit(InteractionDialogMsg::OperationFailed(
        old_id,
        "Synthetic retry",
    ));
    settled(dialog);
    assert!(dialog.widgets().prompts == old, "error must not rebuild");
    assert!(old.is_sensitive());
    assert!(dialog.widgets().error.is_visible());
    // Dismissal promotes the queued request without first submitting this text.
    old_secret.set_text("synthetic-retained-answer");
    settled(dialog);
    assert!(!old_secret.text().is_empty());
    outputs.borrow_mut().clear();
    dialog.emit(InteractionDialogMsg::DismissSession(old_session));
    settled(dialog);
    outputs::fresh(outputs, new_id);
    assert!(
        old_secret.text().is_empty(),
        "replacement wipes retained input"
    );
    assert!(old_secret.parent().is_none());
    assert!(old.first_child().is_none());
    assert!(old.parent().is_none());
    assert!(old_action.parent().is_none());
    assert!(initial.parent().is_none());
    assert!(initial.first_child().is_none());
    outputs.borrow_mut().clear();
    old_secret.set_text("synthetic-retired-answer");
    settled(dialog);
    assert!(
        outputs.borrow().is_empty(),
        "retired input must not forward"
    );
    old_secret.set_text("");

    let prompts = dialog.widgets().prompts.clone();
    assert!(prompts != old && old != initial);
    assert!(prompts.parent().as_ref() == Some(&body));
    assert!(prompts.prev_sibling() == previous);
    assert!(prompts.next_sibling() == next);
    assert_eq!(prompts.orientation(), gtk::Orientation::Vertical);
    assert_eq!(prompts.spacing(), 8);
    assert!(prompts.has_css_class("interaction-prompts"));
    assert!(prompts.is_sensitive());
    assert!(dialog.widgets().actions.is_sensitive());
    assert!(!dialog.widgets().error.is_visible());
    assert!(dialog.widgets().rendered == Some(new_id));
    assert_eq!(dialog.widgets().inputs.len(), 1);
    let label = prompts
        .first_child()
        .unwrap()
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(label.wraps());
    assert_eq!(label.wrap_mode(), gtk::pango::WrapMode::WordChar);
    let secret = dialog.widgets().inputs[0]
        .clone()
        .downcast::<gtk::PasswordEntry>()
        .unwrap();
    assert!(secret.parent().as_ref() == Some(prompts.upcast_ref()));
    assert!(label.next_sibling().as_ref() == Some(secret.upcast_ref()));
    assert!(secret.next_sibling().is_none());
    assert!(!secret.shows_peek_icon());
    assert!(native::focus_within(secret.upcast_ref()));
    secret.set_text("synthetic-new-answer");
    settled(dialog);
    outputs::answered(outputs, new_id);
    open(dialog, new_session, new_id);
    settled(dialog);
    assert!(dialog.widgets().prompts == prompts);
    assert!(dialog.widgets().inputs[0] == *secret.upcast_ref::<gtk::Widget>());
    assert!(!secret.text().is_empty());
    outputs.borrow_mut().clear();
    dialog.emit(InteractionDialogMsg::Action(InteractionAction::Submit));
    settled(dialog);
    outputs::secret_submitted(outputs, new_session, new_id, "synthetic-new-answer");
    assert!(dialog.widgets().prompts == prompts);
    assert!(dialog.widgets().inputs[0] == *secret.upcast_ref::<gtk::Widget>());
    assert!(secret.text().is_empty());
    assert!(!prompts.is_sensitive());
    assert!(!dialog.widgets().actions.is_sensitive());
    dialog.emit(InteractionDialogMsg::OperationFailed(
        new_id,
        "Synthetic retry",
    ));
    settled(dialog);
    assert!(dialog.widgets().prompts == prompts);
    assert!(dialog.widgets().inputs[0] == *secret.upcast_ref::<gtk::Widget>());
    assert!(prompts.is_sensitive());
    secret.set_text("synthetic-before-empty-edit");
    settled(dialog);
    secret.set_text("");
    settled(dialog);
    outputs::answered(outputs, new_id);
    outputs.borrow_mut().clear();
    dialog.emit(InteractionDialogMsg::Action(InteractionAction::Submit));
    settled(dialog);
    outputs::secret_submitted(outputs, new_session, new_id, "");
    dialog.emit(InteractionDialogMsg::ResponseAccepted(new_id));
    let hidden = dialog.widget().clone();
    native::wait_for_frame(window, "production close", move |_| !hidden.is_mapped());
    assert!(secret.text().is_empty());
    outputs.borrow_mut().clear();
    secret.set_text("synthetic-closed-answer");
    native::wait_for_frame(window, "closed retained input", |_| true);
    assert!(
        outputs.borrow().is_empty(),
        "closed input forwarding must retire"
    );
    secret.set_text("");
}

fn run_case(kind: &str) {
    let outputs = outputs::Outputs::default();
    let recorded = outputs.clone();
    let dialog = InteractionDialog::builder()
        .launch(InteractionDialogInit)
        .connect_receiver(move |_, output| recorded.borrow_mut().push(output));
    let window = gtk::ApplicationWindow::builder().build();
    window.set_default_size(600, 420);
    window.set_child(Some(dialog.widget()));
    window.present();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if kind == "password" {
            verify(&dialog, &window, &outputs);
        } else {
            outputs::changed_shape(&dialog, &outputs, kind);
        }
    }));
    if let Err(panic) = result {
        failure::resume_after_failure(panic, || {}, || native::close_window(&window));
    }
    native::close_window(&window);
}

#[test]
fn new_interaction_replaces_prompts_and_wipes_retained_native_inputs() {
    gtk::init().expect("native GTK display required");
    apply_global_css();
    let mut primary = None;
    for kind in ["password", "host", "zero", "fewer"] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_case(kind)));
        eprintln!(
            "PROMPT_REPLACEMENT_CASE kind={kind} passed={}",
            result.is_ok()
        );
        if let Err(panic) = result
            && primary.is_none()
        {
            primary = Some(panic);
        }
    }
    if let Some(panic) = primary {
        std::panic::resume_unwind(panic);
    }
}
