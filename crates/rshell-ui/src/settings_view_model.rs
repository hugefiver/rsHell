use rshell_core::{
    AppSettings, SettingsValidationError, TerminalProfile, TerminalProfileId, UiCommand,
    validate_app_settings, validate_terminal_profile,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSave {
    Profile(TerminalProfileId),
    App,
}

#[derive(Debug, Clone)]
pub struct SettingsViewModel {
    authoritative_settings: AppSettings,
    authoritative_profiles: Vec<TerminalProfile>,
    draft_settings: AppSettings,
    draft_profiles: Vec<TerminalProfile>,
    active_profile: Option<TerminalProfileId>,
    pending: Option<PendingSave>,
    error: Option<String>,
}

impl SettingsViewModel {
    pub fn new(settings: AppSettings, profiles: Vec<TerminalProfile>) -> Self {
        let active_profile = profiles
            .iter()
            .find(|profile| profile.id == settings.default_terminal_profile)
            .or_else(|| profiles.first())
            .map(|profile| profile.id);
        Self {
            authoritative_settings: settings.clone(),
            authoritative_profiles: profiles.clone(),
            draft_settings: settings,
            draft_profiles: profiles,
            active_profile,
            pending: None,
            error: None,
        }
    }

    pub fn profiles(&self) -> &[TerminalProfile] {
        &self.draft_profiles
    }

    pub fn active_profile(&self) -> Option<&TerminalProfile> {
        let active = self.active_profile?;
        self.draft_profiles
            .iter()
            .find(|profile| profile.id == active)
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut TerminalProfile> {
        let active = self.active_profile?;
        self.draft_profiles
            .iter_mut()
            .find(|profile| profile.id == active)
    }

    pub fn select_profile(&mut self, index: usize) {
        self.active_profile = self.draft_profiles.get(index).map(|profile| profile.id);
        self.error = None;
    }

    pub fn app_settings(&self) -> &AppSettings {
        &self.draft_settings
    }

    pub fn app_settings_mut(&mut self) -> &mut AppSettings {
        &mut self.draft_settings
    }

    pub fn profile_dirty(&self) -> bool {
        self.active_profile().is_some_and(|draft| {
            self.authoritative_profiles
                .iter()
                .find(|profile| profile.id == draft.id)
                != Some(draft)
        })
    }

    pub fn app_dirty(&self) -> bool {
        self.draft_settings != self.authoritative_settings
    }

    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn save_profile_command(&mut self) -> Result<UiCommand, SettingsValidationError> {
        let profile = self
            .active_profile()
            .cloned()
            .ok_or(SettingsValidationError {
                field: "profile",
                code: rshell_core::SettingsValidationCode::UnknownProfile,
            })?;
        validate_terminal_profile(&profile)?;
        self.pending = Some(PendingSave::Profile(profile.id));
        self.error = None;
        Ok(UiCommand::SaveTerminalProfile(profile))
    }

    pub fn save_settings_command(&mut self) -> Result<UiCommand, SettingsValidationError> {
        validate_app_settings(&self.draft_settings, &self.draft_profiles)?;
        self.pending = Some(PendingSave::App);
        self.error = None;
        Ok(UiCommand::SaveSettings(self.draft_settings.clone()))
    }

    pub fn accept_profiles(&mut self, profiles: Vec<TerminalProfile>) {
        let pending_id = match self.pending {
            Some(PendingSave::Profile(id)) => Some(id),
            _ => None,
        };
        self.authoritative_profiles = profiles.clone();
        for accepted in profiles {
            if let Some(draft) = self
                .draft_profiles
                .iter_mut()
                .find(|draft| draft.id == accepted.id)
                && pending_id == Some(accepted.id)
            {
                *draft = accepted;
            } else if !self
                .draft_profiles
                .iter()
                .any(|draft| draft.id == accepted.id)
            {
                self.draft_profiles.push(accepted);
            }
        }
        if pending_id.is_some() {
            self.pending = None;
            self.error = None;
        }
    }

    pub fn accept_settings(&mut self, settings: AppSettings) {
        self.authoritative_settings = settings.clone();
        if self.pending == Some(PendingSave::App) {
            self.draft_settings = settings;
            self.pending = None;
            self.error = None;
        }
    }

    pub fn failed(&mut self, context: &'static str) {
        self.pending = None;
        self.error = Some(context.into());
    }

    pub fn rejected(&mut self, message: String) {
        self.pending = None;
        self.error = Some(message);
    }
}
