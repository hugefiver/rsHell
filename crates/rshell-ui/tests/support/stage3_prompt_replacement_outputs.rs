use gtk::prelude::*;
use relm4::{ComponentController, Controller};
use rshell_core::{
    AuthPrompt, HostKeyDecision, HostKeyPrompt, InteractionId, InteractionRequest,
    InteractionResponse, KeyboardInteractivePrompt, SessionId, UiCommand,
};
use rshell_ui::{
    InteractionAction, InteractionDialog, InteractionDialogMsg, InteractionDialogOutput,
};
use secrecy::ExposeSecret;
use std::{cell::RefCell, rc::Rc};

pub(super) type Outputs = Rc<RefCell<Vec<InteractionDialogOutput>>>;

pub(super) fn fresh(outputs: &Outputs, id: InteractionId) {
    let outputs = outputs.borrow();
    let states = outputs
        .iter()
        .filter_map(|output| match output {
            InteractionDialogOutput::StateChanged(state) => Some(state),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!states.is_empty(), "promotion must emit actual state");
    let last = states.last().unwrap();
    eprintln!(
        "PROMPT_REPLACEMENT_STATE events={} answered={} error={}",
        states.len(),
        last.answered_prompts.len(),
        last.has_error
    );
    assert!(
        states.iter().all(|state| state.interaction == Some(id)
            && !state.has_error
            && state.answered_prompts.is_empty()),
        "promotion must not inherit old answers or errors"
    );
}

pub(super) fn answered(outputs: &Outputs, id: InteractionId) {
    assert!(
        outputs
            .borrow()
            .iter()
            .rev()
            .find_map(|output| match output {
                InteractionDialogOutput::StateChanged(state) => Some(state),
                _ => None,
            })
            .is_some_and(|state| state.interaction == Some(id) && state.answered_prompts == [0])
    );
}

pub(super) fn secret_submitted(
    outputs: &Outputs,
    session: SessionId,
    id: InteractionId,
    value: &str,
) {
    response(outputs, session, id, |response| {
        matches!(response,
        InteractionResponse::Secret(secret) if secret.expose_secret() == value)
    });
}

fn response(
    outputs: &Outputs,
    session: SessionId,
    id: InteractionId,
    expected: impl FnOnce(&InteractionResponse) -> bool,
) {
    let outputs = outputs.borrow();
    let commands = outputs
        .iter()
        .filter_map(|output| match output {
            InteractionDialogOutput::Command(command) => Some(command.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(commands.len(), 1);
    let UiCommand::Respond {
        session: actual_session,
        interaction,
        response,
    } = commands[0]
    else {
        panic!("expected actual response command");
    };
    assert!(*actual_session == session && *interaction == id);
    assert!(
        expected(response),
        "exact native answer must reach the new interaction"
    );
}

fn prompt() -> AuthPrompt {
    AuthPrompt {
        id: InteractionId::new(),
        label: "Synthetic prompt".into(),
        echo: false,
    }
}

fn keyboard(id: InteractionId, count: usize) -> InteractionRequest {
    InteractionRequest::KeyboardInteractive(KeyboardInteractivePrompt {
        id,
        name: "Synthetic challenge".into(),
        instruction: "Synthetic instruction".into(),
        prompts: (0..count).map(|_| prompt()).collect(),
    })
}

pub(super) fn changed_shape(dialog: &Controller<InteractionDialog>, outputs: &Outputs, kind: &str) {
    let old_session = SessionId::new();
    let old_id = InteractionId::new();
    let request = if kind == "fewer" {
        keyboard(old_id, 2)
    } else {
        InteractionRequest::Password(AuthPrompt {
            id: old_id,
            ..prompt()
        })
    };
    dialog.emit(InteractionDialogMsg::Open {
        session: old_session,
        request,
    });
    super::settled(dialog);
    let old = dialog.widgets().inputs.clone();
    let session = SessionId::new();
    let id = InteractionId::new();
    let request = match kind {
        "host" => InteractionRequest::HostKey(HostKeyPrompt {
            id,
            host: "synthetic.example.test".into(),
            port: 22,
            algorithm: "ssh-ed25519".into(),
            sha256: "SHA256:synthetic".into(),
            changed: false,
        }),
        "zero" => keyboard(id, 0),
        "fewer" => InteractionRequest::Password(AuthPrompt { id, ..prompt() }),
        _ => unreachable!(),
    };
    dialog.emit(InteractionDialogMsg::Open { session, request });
    super::settled(dialog);
    dialog.emit(InteractionDialogMsg::OperationFailed(
        old_id,
        "Synthetic retry",
    ));
    super::settled(dialog);
    for widget in &old {
        widget
            .clone()
            .downcast::<gtk::Editable>()
            .unwrap()
            .set_text("synthetic-old-answer");
    }
    super::settled(dialog);
    outputs.borrow_mut().clear();
    dialog.emit(InteractionDialogMsg::DismissSession(old_session));
    super::settled(dialog);
    fresh(outputs, id);
    outputs.borrow_mut().clear();
    for widget in &old {
        let input = widget.clone().downcast::<gtk::Editable>().unwrap();
        assert!(input.text().is_empty() && widget.parent().is_none());
        input.set_text("synthetic-retained-answer");
    }
    super::settled(dialog);
    assert!(
        outputs.borrow().is_empty(),
        "retired entries must never forward again"
    );
    for widget in &old {
        widget
            .clone()
            .downcast::<gtk::Editable>()
            .unwrap()
            .set_text("");
    }
    if kind == "fewer" {
        assert_eq!(dialog.widgets().inputs.len(), 1);
        dialog.widgets().inputs[0]
            .clone()
            .downcast::<gtk::Editable>()
            .unwrap()
            .set_text("synthetic-new-answer");
        super::settled(dialog);
        answered(outputs, id);
    } else {
        assert!(dialog.widgets().inputs.is_empty());
    }
    outputs.borrow_mut().clear();
    dialog.emit(InteractionDialogMsg::Action(if kind == "host" {
        InteractionAction::AcceptAndStore
    } else {
        InteractionAction::Submit
    }));
    super::settled(dialog);
    match kind {
        "host" => response(outputs, session, id, |response| {
            matches!(
                response,
                InteractionResponse::HostKey(HostKeyDecision::AcceptAndStore)
            )
        }),
        "zero" => response(outputs, session, id, |response| {
            matches!(response,
            InteractionResponse::Answers(answers) if answers.is_empty())
        }),
        "fewer" => secret_submitted(outputs, session, id, "synthetic-new-answer"),
        _ => unreachable!(),
    }
}
