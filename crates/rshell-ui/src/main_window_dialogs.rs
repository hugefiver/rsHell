use relm4::Controller;

use crate::{ImportDialog, InteractionDialog, SettingsWindow};

pub struct MainWindowDialogs {
    pub settings: Controller<SettingsWindow>,
    pub import: Controller<ImportDialog>,
    pub interaction: Controller<InteractionDialog>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCommandSource {
    Settings,
    Import,
}
