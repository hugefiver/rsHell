use std::{fmt, rc::Rc, sync::Arc};

use rshell_core::{AppViewModel, ApplicationHandle, LatestViewStream, UiCommandPort};
use rshell_platform::FileSelectionService;

use crate::{
    GtkFileSelectionService, SmokeDriverInit, SmokeReportHandle, StartupProbe,
    main_window_streams::MainWindowLiveSources,
};

pub struct MainWindowInit {
    pub(crate) command_port: Arc<dyn UiCommandPort>,
    pub(crate) view_model: AppViewModel,
    pub(crate) live_sources: Option<MainWindowLiveSources>,
    pub(crate) file_selection: Rc<dyn FileSelectionService>,
    pub(crate) startup_probe: Option<StartupProbe>,
    pub(crate) smoke: Option<(SmokeDriverInit, SmokeReportHandle)>,
}

impl MainWindowInit {
    pub fn new(command_port: Arc<dyn UiCommandPort>, view_model: AppViewModel) -> Self {
        Self {
            command_port,
            view_model,
            live_sources: None,
            file_selection: Rc::new(GtkFileSelectionService),
            startup_probe: None,
            smoke: None,
        }
    }

    pub fn from_application(application: &ApplicationHandle) -> Self {
        Self {
            command_port: application.ui_port(),
            view_model: application.view_model(),
            live_sources: Some(MainWindowLiveSources {
                events: application.event_stream(),
                views: application.view_stream(),
            }),
            file_selection: Rc::new(GtkFileSelectionService),
            startup_probe: None,
            smoke: None,
        }
    }

    pub fn with_file_selection(mut self, service: Rc<dyn FileSelectionService>) -> Self {
        self.file_selection = service;
        self
    }

    pub fn with_startup_probe(mut self, probe: StartupProbe) -> Self {
        self.startup_probe = Some(probe);
        self
    }

    pub fn with_smoke_driver(mut self, driver: SmokeDriverInit) -> (Self, SmokeReportHandle) {
        let report = SmokeReportHandle::new(&driver);
        self.smoke = Some((driver, report.clone()));
        (self, report)
    }

    pub fn latest_view_stream(&self) -> Option<LatestViewStream> {
        self.live_sources
            .as_ref()
            .map(|sources| sources.views.clone())
    }
}

impl fmt::Debug for MainWindowInit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MainWindowInit")
            .field("command_port", &"UiCommandPort")
            .field("view_model", &self.view_model)
            .field("live_updates", &self.live_sources.is_some())
            .field("startup_probe", &self.startup_probe.is_some())
            .field("smoke_driver", &self.smoke.is_some())
            .finish()
    }
}
