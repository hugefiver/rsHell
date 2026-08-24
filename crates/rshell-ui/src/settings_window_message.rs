use rshell_core::{AppSettings, TerminalProfile, UiCommand, UiPortError};

#[derive(Debug, Clone)]
pub struct SettingsWindowInit {
    pub settings: AppSettings,
    pub profiles: Vec<TerminalProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTextField {
    ProfileName,
    TerminalType,
    FontFamily,
    ProfileBindings,
    Answerback,
    AppBindings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsBoolField {
    LeftAltMeta,
    RightAltMeta,
    CsiU,
    KittyKeyboard,
    MouseReporting,
    ScrollOnOutput,
    ScrollOnKeypress,
}

#[derive(Debug, Clone)]
pub enum SettingsWindowMsg {
    Open,
    Close,
    SelectProfile(u32),
    Text(SettingsTextField, String),
    Columns(u16),
    Rows(u16),
    Scrollback(usize),
    FontSize(f32),
    Scheme(u32),
    Bool(SettingsBoolField, bool),
    DefaultProfile(u32),
    AppScheme(u32),
    SaveProfile,
    SaveApp,
    ProfilesAccepted(Vec<TerminalProfile>),
    SettingsAccepted(AppSettings),
    OperationFailed(&'static str),
    CommandRejected(UiPortError),
}

#[derive(Debug)]
pub enum SettingsWindowOutput {
    Command(Box<UiCommand>),
    Closed,
}
