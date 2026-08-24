use rshell_core::AppViewModel;

use crate::{PaneHostModel, StartupProbe};

#[derive(Debug, Clone)]
pub struct PaneHostInit {
    pub view_model: AppViewModel,
    pub startup_probe: Option<StartupProbe>,
}

impl PaneHostInit {
    pub(crate) fn into_model(self) -> PaneHostModel {
        PaneHostModel::new(self.view_model).with_startup_probe(self.startup_probe)
    }
}
