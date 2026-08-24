use gtk::prelude::*;
use relm4::ComponentController;

use crate::{
    MainWindow, SettingsWindowMsg, SmokeBindingEvidence, SmokeVisualEvidence, collect_visual_facts,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum VisualCheckpointPhase {
    #[default]
    Idle,
    Opening,
    Observed,
    Closing,
    Complete,
}

impl MainWindow {
    pub(crate) fn route_visual_checkpoint(&mut self) -> Result<bool, &'static str> {
        match self.smoke_state.visual_checkpoint {
            VisualCheckpointPhase::Idle => {
                self.send_settings(SettingsWindowMsg::Open);
                self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Opening;
                Ok(false)
            }
            VisualCheckpointPhase::Opening => {
                if !self.dialogs.settings.widget().is_visible()
                    || self.dialogs.settings.widget().width() <= 0
                    || self.dialogs.settings.widget().height() <= 0
                {
                    return Ok(false);
                }
                let root = self
                    .dialogs
                    .settings
                    .widget()
                    .root()
                    .and_then(|root| root.downcast::<gtk::ApplicationWindow>().ok())
                    .ok_or("visual_root_unavailable")?;
                let facts = collect_visual_facts(root.upcast_ref(), (1_360, 860));
                if !facts.contract_passes() {
                    return Err("visual_contract_incomplete");
                }
                self.smoke_state.visual = Some(SmokeVisualEvidence { facts, png: None });
                self.capture_smoke_png();
                if self
                    .smoke_state
                    .visual
                    .is_none_or(|visual| visual.png.is_none())
                {
                    return Err("visual_capture_failed");
                }
                self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Observed;
                Ok(false)
            }
            VisualCheckpointPhase::Observed => {
                self.send_settings(SettingsWindowMsg::Close);
                self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Closing;
                Ok(false)
            }
            VisualCheckpointPhase::Closing => {
                if self.dialogs.settings.widget().is_visible() {
                    return Ok(false);
                }
                self.smoke_state.visual_checkpoint = VisualCheckpointPhase::Complete;
                Ok(true)
            }
            VisualCheckpointPhase::Complete => Ok(true),
        }
    }
}

pub(crate) fn visual_checkpoint_component_verified(
    phase: VisualCheckpointPhase,
    visual: Option<&SmokeVisualEvidence>,
) -> bool {
    phase == VisualCheckpointPhase::Complete
        && visual.is_some_and(|evidence| evidence.facts.contract_passes())
}

pub(crate) fn visual_checkpoint_binding(
    surface: Option<&str>,
    connection: Option<&str>,
    component_verified: bool,
) -> SmokeBindingEvidence {
    SmokeBindingEvidence {
        verified: component_verified && surface == Some("gtk") && connection.is_none(),
        component_verified,
        actual_label: Some("main_window".to_owned()),
        connection_id: None,
        profile_name: None,
        endpoint: None,
        pane_id: None,
        session_id: None,
        local: false,
    }
}
