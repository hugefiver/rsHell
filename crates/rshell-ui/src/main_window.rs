use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    ConnectionEditor, ConnectionEditorInit, ConnectionEditorOutput, ConnectionSidebar,
    ConnectionSidebarInit, ConnectionSidebarOutput, ImportDialog, ImportDialogInit,
    ImportDialogOutput, InteractionDialog, InteractionDialogInit, InteractionDialogOutput,
    MainWindowInit, PaneHost, PaneHostInit, PaneHostOutput, SessionTabBar, SessionTabBarInit,
    SessionTabBarOutput, SettingsWindow, SettingsWindowInit, SettingsWindowOutput,
    main_window_dialogs::{DialogCommandSource, MainWindowDialogs},
    main_window_layout::{
        MainWindowContent, MainWindowWidgets, build_command_bar, install_content,
    },
    main_window_smoke::SmokeUiState,
    main_window_streams::spawn_live_forwarders,
    smoke_driver_state::SmokeDriver,
};
use gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
    gtk,
};
use rshell_core::{AppEvent, AppViewModel, InteractionId, SessionId, UiCommandPort};

#[derive(Debug)]
pub enum MainWindowMsg {
    Sidebar(ConnectionSidebarOutput),
    Editor(ConnectionEditorOutput),
    TabBar(SessionTabBarOutput),
    PaneHost(PaneHostOutput),
    Settings(SettingsWindowOutput),
    Import(ImportDialogOutput),
    Interaction(InteractionDialogOutput),
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

pub struct MainWindow {
    pub(crate) command_port: Arc<dyn UiCommandPort>,
    pub(crate) view_model: AppViewModel,
    pub(crate) sidebar: Controller<ConnectionSidebar>,
    pub(crate) editor: Controller<ConnectionEditor>,
    pub(crate) tab_bar: Controller<SessionTabBar>,
    pub(crate) pane_host: Controller<PaneHost>,
    pub(crate) dialogs: MainWindowDialogs,
    pub(crate) status: String,
    pub(crate) editor_command_pending: bool,
    pub(crate) authoritative_view: bool,
    pub(crate) pending_dialog: Option<DialogCommandSource>,
    pub(crate) pending_interaction: Option<(SessionId, InteractionId)>,
    pub(crate) startup_probe: Option<crate::StartupProbe>,
    pub(crate) smoke: Option<SmokeDriver>,
    pub(crate) smoke_state: SmokeUiState,
    pub(crate) smoke_tick_pending: bool,
    pub(crate) smoke_paintable: Option<gtk::WidgetPaintable>,
    pub(crate) smoke_png_path: Option<PathBuf>,
    live_forwarders: Vec<gtk::glib::JoinHandle<()>>,
}

impl SimpleComponent for MainWindow {
    type Init = MainWindowInit;
    type Input = MainWindowMsg;
    type Output = ();
    type Root = gtk::ApplicationWindow;
    type Widgets = MainWindowWidgets;

    fn init_root() -> Self::Root {
        gtk::ApplicationWindow::builder()
            .title("rsHell")
            .default_width(1_360)
            .default_height(860)
            .build()
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let MainWindowInit {
            command_port,
            view_model,
            live_sources,
            file_selection,
            startup_probe,
            smoke,
        } = init;
        let authoritative_view = live_sources.is_some();
        if let Some(probe) = &startup_probe {
            let probe = probe.clone();
            root.connect_realize(move |_| probe.observe_window_realized());
        }
        if smoke.is_some() {
            let sender = sender.clone();
            root.connect_realize(move |_| sender.input(MainWindowMsg::SmokeWindowRealized));
        }
        root.add_css_class("fluent-shell");
        let command_bar = build_command_bar(&sender);

        let sidebar = ConnectionSidebar::builder()
            .launch(ConnectionSidebarInit {
                catalog: view_model.catalog.clone(),
            })
            .forward(sender.input_sender(), MainWindowMsg::Sidebar);
        let editor = ConnectionEditor::builder()
            .launch(ConnectionEditorInit {
                terminal_profiles: view_model.terminal_profiles.clone(),
            })
            .forward(sender.input_sender(), MainWindowMsg::Editor);
        let tab_bar = SessionTabBar::builder()
            .launch(SessionTabBarInit {
                workspace: view_model.workspace.clone(),
            })
            .forward(sender.input_sender(), MainWindowMsg::TabBar);
        let pane_host = PaneHost::builder()
            .launch(PaneHostInit {
                view_model: view_model.clone(),
                startup_probe: startup_probe.clone(),
            })
            .forward(sender.input_sender(), MainWindowMsg::PaneHost);
        let settings = SettingsWindow::builder()
            .launch(SettingsWindowInit {
                settings: view_model.settings.clone(),
                profiles: view_model.terminal_profiles.clone(),
            })
            .forward(sender.input_sender(), MainWindowMsg::Settings);
        let import = ImportDialog::builder()
            .launch(ImportDialogInit { file_selection })
            .forward(sender.input_sender(), MainWindowMsg::Import);
        let interaction = InteractionDialog::builder()
            .launch(InteractionDialogInit)
            .forward(sender.input_sender(), MainWindowMsg::Interaction);

        let status = install_content(
            &root,
            MainWindowContent {
                command_bar: command_bar.upcast_ref(),
                sidebar: sidebar.widget().upcast_ref(),
                editor: editor.widget().upcast_ref(),
                tab_bar: tab_bar.widget().upcast_ref(),
                pane_host: pane_host.widget().upcast_ref(),
                settings: settings.widget().upcast_ref(),
                import: import.widget().upcast_ref(),
                interaction: interaction.widget().upcast_ref(),
            },
        );

        let live_forwarders = live_sources
            .map(|sources| spawn_live_forwarders(sources, &sender))
            .unwrap_or_default();
        let smoke_png_path = smoke.as_ref().and_then(|(init, _)| init.png_path.clone());
        let smoke_paintable = smoke_png_path
            .as_ref()
            .map(|_| gtk::WidgetPaintable::new(Some(&root)));
        let smoke = smoke.map(|(init, report)| SmokeDriver::new(init, report));
        let model = Self {
            command_port,
            view_model,
            sidebar,
            editor,
            tab_bar,
            pane_host,
            dialogs: MainWindowDialogs {
                settings,
                import,
                interaction,
            },
            status: "Ready".into(),
            editor_command_pending: false,
            authoritative_view,
            pending_dialog: None,
            pending_interaction: None,
            startup_probe,
            smoke,
            smoke_state: SmokeUiState::default(),
            smoke_tick_pending: false,
            smoke_paintable,
            smoke_png_path,
            live_forwarders,
        };
        if model.smoke.is_some() {
            sender.input(MainWindowMsg::SmokeTick);
        }
        let widgets = MainWindowWidgets { status };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        if matches!(&message, MainWindowMsg::SmokeTick) {
            self.smoke_tick_pending = false;
        }
        match message {
            MainWindowMsg::Sidebar(output) => self.handle_sidebar(output),
            MainWindowMsg::Editor(output) => self.handle_editor(output),
            MainWindowMsg::TabBar(output) => self.handle_tab_bar(output),
            MainWindowMsg::PaneHost(output) => self.handle_pane_host(output),
            MainWindowMsg::Settings(output) => self.handle_settings(output),
            MainWindowMsg::Import(output) => self.handle_import(output),
            MainWindowMsg::Interaction(output) => self.handle_interaction(output),
            MainWindowMsg::OpenSettings => self.open_settings(),
            MainWindowMsg::OpenImport => self.open_import(),
            MainWindowMsg::AppEvent(event) => self.handle_event(event),
            MainWindowMsg::ReplaceViewModel(view_model) => self.replace_view_model(view_model),
            MainWindowMsg::LiveEvent {
                view,
                event,
                pending,
            } => {
                self.replace_view_model(*view);
                self.handle_event(*event);
                pending.store(false, Ordering::Release);
            }
            MainWindowMsg::LiveView { view, pending } => {
                self.replace_view_model(*view);
                pending.store(false, Ordering::Release);
            }
            MainWindowMsg::SmokeTick => {}
            MainWindowMsg::SmokeWindowRealized => self.smoke_state.window_realized = true,
        }
        self.drive_smoke(&sender);
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets.status.set_label(&self.status);
    }
}

impl Drop for MainWindow {
    fn drop(&mut self) {
        for forwarder in &self.live_forwarders {
            forwarder.abort();
        }
    }
}
