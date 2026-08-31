use crate::{SmokeVisualCheckpointEvidence, SmokeVisualState};

impl SmokeVisualCheckpointEvidence {
    pub fn contract_passes(&self) -> bool {
        let modal = matches!(
            self.state,
            SmokeVisualState::Editor
                | SmokeVisualState::Settings
                | SmokeVisualState::Import
                | SmokeVisualState::HostKey
                | SmokeVisualState::Authentication
        );
        self.checkpoint_id.len() <= 96
            && self.facts.contract_passes()
            && self.png.non_empty
            && self.png.width > 0
            && self.png.height > 0
            && self.png.luminance_buckets > 1
            && self.dpi.logical_width > 0
            && self.dpi.logical_height > 0
            && (!self.facts.terminal_canvas
                || (self.dpi.effective_scale.is_finite()
                    && self.dpi.effective_scale > 0.0
                    && self.dpi.effective_dpi.is_finite()
                    && self.dpi.effective_dpi > 0.0
                    && self.dpi.cell_width > 0.0
                    && self.dpi.cell_height > 0.0))
            && self.dpi.icon_logical_size > 0
            && self.dpi.icon_texture_width >= i32::from(self.dpi.icon_logical_size)
            && self.dpi.icon_texture_height >= i32::from(self.dpi.icon_logical_size)
            && self.accessibility.unnamed_icon_controls == 0
            && self.accessibility.hidden_primary_actions == 0
            && self.accessibility.zero_size_panes == 0
            && !self.accessibility.horizontal_clipping
            && (!modal
                || (self.accessibility.background_insensitive
                    && self.accessibility.focus_contained
                    && self.accessibility.focus_restored
                    && self.accessibility.escape_cancelled))
            && crate::visual_contract::visual_contrast_passes()
    }
}
