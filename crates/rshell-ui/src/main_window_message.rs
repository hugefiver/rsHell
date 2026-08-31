use std::sync::{Arc, atomic::AtomicBool};

use rshell_core::{AppEvent, AppViewModel};

use crate::{
    ConnectionEditorOutput, ConnectionSidebarOutput, ImportDialogOutput, InteractionDialogOutput,
    ModalRequest, NavigationAction, PaneHostOutput, SessionTabBarOutput, SettingsWindowOutput,
};

#[derive(Debug)]
pub enum MainWindowMsg {
    Allocated {
        width: i32,
    },
    NewLocalTab,
    CycleTabs(i32),
    Navigation(NavigationAction),
    Sidebar(ConnectionSidebarOutput),
    Editor(ConnectionEditorOutput),
    TabBar(SessionTabBarOutput),
    PaneHost(PaneHostOutput),
    Settings(SettingsWindowOutput),
    Import(ImportDialogOutput),
    Interaction(InteractionDialogOutput),
    Modal(ModalRequest),
    OpenSettings,
    OpenImport,
    AppEvent(AppEvent),
    ReplaceViewModel(AppViewModel),
    LiveEvent {
        view: Box<AppViewModel>,
        event: Box<AppEvent>,
        pending: Arc<AtomicBool>,
    },
    LiveView {
        view: Box<AppViewModel>,
        pending: Arc<AtomicBool>,
    },
    SmokeTick,
    SmokeWindowRealized,
}
