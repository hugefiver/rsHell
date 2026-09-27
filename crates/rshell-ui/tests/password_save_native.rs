#![cfg(not(target_os = "macos"))]

use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AuthenticationKind, CatalogMutation, ConnectionProfile, SecretUpdate, TerminalProfile,
    TransportKind, UiCommand,
};
use rshell_ui::{
    ConnectionEditor, ConnectionEditorInit, ConnectionEditorMsg, ConnectionEditorOutput,
    EditorValidationError,
};
use secrecy::ExposeSecret;

#[test]
fn password_type_clear_save_shows_error_without_dispatch_and_retries_natively() {
    gtk::init().expect("password save proof requires a GTK display");
    rshell_ui::apply_global_css();
    // Exercise the existing scrollable editor at compact, standard, and wide allocations.
    for (width, height) in [(640, 480), (1_000, 700), (1_440, 900)] {
        assert_password_save_retry(width, height);
    }
}

fn assert_password_save_retry(width: i32, height: i32) {
    let outputs = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&outputs);
    let editor = ConnectionEditor::builder()
        .launch(ConnectionEditorInit {
            terminal_profiles: vec![TerminalProfile::default()],
        })
        .connect_receiver(move |_, output| recorded.borrow_mut().push(output));
    let window = gtk::Window::new();
    window.set_default_size(width, height);
    window.set_child(Some(editor.widget()));
    window.present();

    let mut source = ConnectionProfile::new("Stored password", "password.example.test");
    source.transport = TransportKind::NativeSsh;
    source.authentication = AuthenticationKind::Password;
    source.credential_ref = Some("rshell://credential/password-save-regression".into());
    editor.emit(ConnectionEditorMsg::OpenEdit(Box::new(source.clone())));
    flush_gtk();
    let widgets = descendants(editor.widget());
    let password = widgets
        .iter()
        .find_map(|widget| widget.clone().downcast::<gtk::PasswordEntry>().ok())
        .expect("native password field");
    let save = widgets
        .iter()
        .filter_map(|widget| widget.clone().downcast::<gtk::Button>().ok())
        .find(|button| button.label().as_deref() == Some("Save connection"))
        .expect("native Save connection action");
    let error = widgets
        .iter()
        .find(|widget| widget.has_css_class("dialog-error"))
        .unwrap()
        .clone()
        .downcast::<gtk::Label>()
        .unwrap();
    assert!(
        password.text().is_empty(),
        "stored passwords must never be loaded"
    );

    // An untouched blank GTK field still preserves the original stored credential.
    save.emit_clicked();
    flush_gtk();
    assert!(outputs.borrow().iter().any(|output| matches!(
        output,
        ConnectionEditorOutput::Command(command)
            if matches!(command.secret_update(), Some(SecretUpdate::Unchanged))
    )));
    editor.emit(ConnectionEditorMsg::CommandAccepted);
    editor.emit(ConnectionEditorMsg::OpenEdit(Box::new(source.clone())));
    flush_gtk();
    outputs.borrow_mut().clear();

    password.set_text("discarded-native-password");
    flush_gtk();
    password.set_text("");
    flush_gtk();
    for _ in 0..2 {
        save.emit_clicked();
        flush_gtk();
        assert!(
            editor.widget().is_visible(),
            "rejected Save must keep the draft open"
        );
        assert!(save.is_sensitive(), "rejected Save must allow retry");
        assert!(error.is_visible());
        assert_eq!(
            error.text(),
            EditorValidationError::SecretRequired.to_string()
        );
        // Persistence is reachable only through Command. No dispatch means the old
        // credential cannot be deleted, even when the user repeats Save.
        assert!(outputs.borrow().iter().all(|output| !matches!(
            output,
            ConnectionEditorOutput::Command(_) | ConnectionEditorOutput::Closed
        )));
        assert!(outputs.borrow().iter().any(|output| matches!(
            output,
            ConnectionEditorOutput::StateChanged(state)
                if state.open && !state.pending && state.has_error
                    && state.draft.as_ref().is_some_and(|draft|
                        draft.secret_changed && !draft.secret_present)
        )));
    }

    // Validation lives in the established scrolling form body; verify the wrapped
    // feedback can actually be read, not merely that its visible flag is set.
    let scroll = error
        .ancestor(gtk::ScrolledWindow::static_type())
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    wait_for_gtk(|| {
        let adjustment = scroll.vadjustment();
        adjustment.set_value(adjustment.upper() - adjustment.page_size());
        error.is_mapped()
            && error.compute_bounds(&scroll).is_some_and(|bounds| {
                bounds.width() > 0.0
                    && bounds.height() > 0.0
                    && bounds.x() >= -1.0
                    && bounds.y() >= -1.0
                    && bounds.x() + bounds.width() <= scroll.width() as f32 + 1.0
                    && bounds.y() + bounds.height() <= scroll.height() as f32 + 1.0
            })
    });

    password.set_text("replacement-native-password");
    flush_gtk();
    save.emit_clicked();
    flush_gtk();
    assert!(
        !save.is_sensitive(),
        "valid retry waits for coordinator acknowledgement"
    );
    assert!(
        !error.is_visible(),
        "valid retry clears the validation error"
    );
    assert!(
        password.text().is_empty(),
        "submitted GTK secret must be cleared"
    );
    let commands = outputs
        .borrow_mut()
        .drain(..)
        .filter_map(|output| match output {
            ConnectionEditorOutput::Command(command) => Some(*command),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        commands.len(),
        1,
        "only the valid retry may reach persistence"
    );
    let UiCommand::ApplyCatalog {
        mutation: CatalogMutation::Update(updated),
        secret: SecretUpdate::Set(secret),
    } = commands.into_iter().next().unwrap()
    else {
        panic!("retry must replace the password, never clear it");
    };
    assert_eq!(updated.id, source.id);
    assert_eq!(updated.authentication, AuthenticationKind::Password);
    assert!(secret.expose_secret() == "replacement-native-password");
    editor.emit(ConnectionEditorMsg::CommandAccepted);
    flush_gtk();
    assert!(!editor.widget().is_visible());
    window.close();
    flush_gtk();
}

fn flush_gtk() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..512 {
        if !context.iteration(false) {
            return;
        }
    }
    panic!("password editor messages did not quiesce");
}

fn wait_for_gtk(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        flush_gtk();
        if condition() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "password validation must fit in the visible form body"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut output = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        output.push(widget.clone());
        output.extend(descendants(&widget));
        child = widget.next_sibling();
    }
    output
}
