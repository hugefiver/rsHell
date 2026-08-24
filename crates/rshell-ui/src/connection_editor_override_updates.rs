use rshell_core::TerminalSettingsV1;

use crate::{
    ConnectionEditor, TerminalOverrideKey,
    connection_editor_override_state::{
        set_override_bool, set_override_number, set_override_scheme, set_override_text,
    },
    key_binding_text::parse_bindings,
};

impl ConnectionEditor {
    pub(crate) fn set_override_inheritance(&mut self, key: TerminalOverrideKey, inherited: bool) {
        let base = self.selected_base().cloned();
        if let Some(draft) = &mut self.draft {
            if inherited {
                draft.set_inherited(key);
            } else if let Some(base) = &base {
                draft.set_explicit_from_base(key, base);
            }
        }
    }

    pub(crate) fn set_override_text(&mut self, key: TerminalOverrideKey, value: String) {
        if let Some(draft) = &mut self.draft {
            set_override_text(draft.view_mut(), key, value);
        }
    }

    pub(crate) fn set_override_number(&mut self, key: TerminalOverrideKey, value: f64) {
        if let Some(draft) = &mut self.draft {
            set_override_number(draft.view_mut(), key, value);
        }
    }

    pub(crate) fn set_override_scheme(&mut self, index: u32) {
        if let Some(draft) = &mut self.draft {
            set_override_scheme(draft.view_mut(), index);
        }
    }

    pub(crate) fn set_override_bool(&mut self, key: TerminalOverrideKey, value: bool) {
        if let Some(draft) = &mut self.draft {
            set_override_bool(draft.view_mut(), key, value);
        }
    }

    pub(crate) fn set_override_bindings(&mut self, text: &str) {
        match parse_bindings(text) {
            Ok(bindings) => {
                self.override_input_error = None;
                if let Some(draft) = &mut self.draft {
                    draft.view_mut().terminal_overrides.key_bindings = Some(bindings);
                }
            }
            Err(error) => self.override_input_error = Some(error),
        }
    }

    pub(crate) fn selected_base(&self) -> Option<&TerminalSettingsV1> {
        let selected = self
            .draft
            .as_ref()
            .and_then(|draft| draft.view().terminal_profile_id);
        selected
            .and_then(|id| {
                self.terminal_profiles
                    .iter()
                    .find(|profile| profile.id == id)
            })
            .or_else(|| self.terminal_profiles.first())
            .map(|profile| &profile.settings)
    }
}
