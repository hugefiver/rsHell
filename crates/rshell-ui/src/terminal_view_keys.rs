use gtk::gdk::{Key, ModifierType};
use rshell_core::{
    KeyCode, KeyModifiers, SessionUiCommand, SplitAxis, TerminalInput, TerminalKeyAction,
    UiCommand, parse_terminal_key_action,
};

use crate::{
    TerminalViewError, TerminalViewModel,
    terminal_input::{map_gdk_key_with_modifiers, modifiers},
};

impl TerminalViewModel {
    pub fn key(
        &mut self,
        key: Key,
        state: ModifierType,
    ) -> Result<Option<UiCommand>, TerminalViewError> {
        self.key_pressed(key, state)
    }

    pub fn key_pressed(
        &mut self,
        key: Key,
        state: ModifierType,
    ) -> Result<Option<UiCommand>, TerminalViewError> {
        if self.alt.press(key) {
            return Ok(None);
        }
        let mut key_modifiers = modifiers(state);
        key_modifiers.alt = self.alt.as_meta(
            key_modifiers.alt,
            self.profile.left_alt_as_meta,
            self.profile.right_alt_as_meta,
        );
        self.resolve_key(key, key_modifiers)
    }

    pub fn key_released(&mut self, key: Key) {
        self.alt.release(key);
    }

    pub fn focus_lost(&mut self) {
        self.alt.clear();
    }

    fn resolve_key(
        &mut self,
        key: Key,
        key_modifiers: KeyModifiers,
    ) -> Result<Option<UiCommand>, TerminalViewError> {
        let character = key.to_unicode().map(|value| value.to_ascii_lowercase());
        if character == Some('c')
            && key_modifiers.control
            && !key_modifiers.shift
            && !key_modifiers.alt
            && !key_modifiers.super_key
        {
            return Ok(Some(self.command(SessionUiCommand::Interrupt)));
        }
        if key_modifiers.control && key_modifiers.shift {
            match character {
                Some('f') => {
                    self.search.open();
                    return Ok(None);
                }
                Some('c' | 'v') => return Ok(None),
                _ => {}
            }
        }
        if self.search.is_open() && matches!(key, Key::Return | Key::KP_Enter) {
            return Ok(self.navigate_search(key_modifiers.shift));
        }
        if self.search.is_open() && key == Key::Escape {
            self.search.close();
            return Ok(None);
        }
        let Some(TerminalInput::Key { code, modifiers }) =
            map_gdk_key_with_modifiers(key, key_modifiers)
        else {
            return Ok(None);
        };
        if let Some(action) = self.configured_action(&code, modifiers) {
            return Ok(Some(self.action_command(action)));
        }
        Ok(Some(self.command(SessionUiCommand::Input(
            TerminalInput::Key { code, modifiers },
        ))))
    }

    fn configured_action(
        &self,
        code: &KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<TerminalKeyAction> {
        self.profile
            .key_bindings
            .iter()
            .find(|binding| binding.code == *code && binding.modifiers == modifiers)
            .and_then(|binding| parse_terminal_key_action(&binding.action).ok())
    }

    fn action_command(&self, action: TerminalKeyAction) -> UiCommand {
        match action {
            TerminalKeyAction::Send(sequence) => self.command(SessionUiCommand::Input(
                TerminalInput::CommittedText(sequence.as_str().to_owned()),
            )),
            TerminalKeyAction::ClearScrollback => self.command(SessionUiCommand::ClearScrollback),
            TerminalKeyAction::NewTab => UiCommand::NewLocalTab,
            TerminalKeyAction::SplitVertical => UiCommand::Split {
                pane: self.pane,
                axis: SplitAxis::Vertical,
            },
        }
    }
}
