use gtk::gdk;

use crate::TerminalViewMsg;

pub(crate) fn split_smoke_terminal_submission(mut text: String) -> (String, bool) {
    let suffix = if text.ends_with("\r\n") {
        2
    } else if text.ends_with(['\r', '\n']) {
        1
    } else {
        0
    };
    if suffix > 0 {
        text.truncate(text.len() - suffix);
    }
    (text, suffix > 0)
}

pub(crate) fn smoke_terminal_messages(text: String, submit: bool) -> Vec<TerminalViewMsg> {
    let mut messages = Vec::with_capacity(2);
    if !text.is_empty() {
        messages.push(TerminalViewMsg::CommittedText(text));
    }
    if submit {
        messages.push(TerminalViewMsg::Key {
            key: gdk::Key::Return,
            state: gdk::ModifierType::empty(),
        });
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_submission_routes_enter_as_a_real_key_event() {
        for input in ["Write-Output READY\r", "Write-Output READY\r\n"] {
            let (text, submit) = split_smoke_terminal_submission(input.into());
            let messages = smoke_terminal_messages(text, submit);
            assert!(matches!(
                messages.as_slice(),
                [TerminalViewMsg::CommittedText(text), TerminalViewMsg::Key { key, state }]
                    if text == "Write-Output READY"
                        && *key == gdk::Key::Return
                        && state.is_empty()
            ));
        }

        let (text, submit) = split_smoke_terminal_submission("partial".into());
        assert!(!submit);
        assert!(matches!(
            smoke_terminal_messages(text, submit).as_slice(),
            [TerminalViewMsg::CommittedText(text)] if text == "partial"
        ));
    }
}
