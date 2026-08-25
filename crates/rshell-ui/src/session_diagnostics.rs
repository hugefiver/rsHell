use rshell_core::{SessionFailure, SessionUiEvent};

use crate::pane_view_model::failure_label;

pub(crate) fn failure_line(event: &SessionUiEvent) -> Option<String> {
    let (state, failure) = match event {
        SessionUiEvent::Failed(failure) => ("failed", *failure),
        SessionUiEvent::Crashed(_) => ("crashed", SessionFailure::Crashed),
        _ => return None,
    };
    Some(format!(
        "P0_SESSION state={state} code={}",
        failure_label(failure)
    ))
}

pub(crate) fn emit_session_failure(event: &SessionUiEvent) {
    if std::env::var_os("RSHELL_QA_SMOKE").is_some()
        && let Some(line) = failure_line(event)
    {
        eprintln!("{line}");
    }
}
