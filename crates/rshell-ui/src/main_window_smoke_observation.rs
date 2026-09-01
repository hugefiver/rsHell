use std::collections::BTreeSet;

use rshell_core::{AppViewModel, PaneLaunchTarget, SessionId, TabId};

use crate::{
    MainWindow, SmokeCounters, SmokeFrameEvidence,
    smoke_driver_observation::{SmokeBindingRequest, SmokeObservation},
};

impl MainWindow {
    pub(crate) fn smoke_observation(
        &mut self,
        binding: Option<&SmokeBindingRequest>,
    ) -> SmokeObservation {
        self.refresh_smoke_reconnect();
        let panes = self
            .view_model
            .workspace
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.pane_ids())
            .collect::<Vec<_>>();
        let sessions = self
            .view_model
            .workspace
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.session_ids())
            .count();
        let connection_panes = self
            .view_model
            .pane_launches
            .values()
            .filter_map(|target| match target {
                PaneLaunchTarget::Connection { id, .. } => Some(*id),
                PaneLaunchTarget::Local => None,
            })
            .collect::<BTreeSet<_>>();
        let active_tab = self
            .smoke_state
            .active_tab
            .or(self.view_model.workspace.active_tab);
        let active_session = active_session(&self.view_model, active_tab);
        let active_frame = active_session
            .and_then(|session| self.view_model.latest_frames.get(&session))
            .cloned();
        if let (Some(session), Some(frame)) = (active_session, active_frame.as_deref()) {
            self.observe_smoke_tui_frame(session, frame);
        }
        let latest_frame = active_frame.as_deref().map(|frame| SmokeFrameEvidence {
            generation: frame.generation,
            cols: frame.size.cols,
            rows: frame.size.rows,
            pixel_width: frame.size.pixel_width,
            pixel_height: frame.size.pixel_height,
            dpi: frame.size.dpi,
        });
        SmokeObservation {
            window_realized: self.smoke_state.window_realized,
            editor_open: self.smoke_state.editor_open,
            sidebar_selection: self.smoke_state.sidebar_selection,
            connection_panes,
            import_preview_ready: self.smoke_state.import_preview_ready,
            active_tab,
            tab_ids: self
                .view_model
                .workspace
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect(),
            shutdown_complete: self.smoke_state.shutdown_complete,
            active_interaction: self.smoke_state.interaction,
            answered_prompts: self.smoke_state.interaction_answered_prompts.clone(),
            last_interaction_response: self.smoke_state.last_interaction_response,
            binding: self.smoke_binding(binding),
            counters: SmokeCounters {
                tabs: self.view_model.workspace.tabs.len(),
                panes: panes.len(),
                sessions,
                frames: self.view_model.latest_frames.len(),
                active_session,
                active_session_state: active_session
                    .and_then(|session| self.view_model.session_states.get(&session).copied()),
                terminal_commands: self.smoke_state.terminal_commands,
                clipboard_writes: self.smoke_state.clipboard_writes,
                clipboard_bytes: self.smoke_state.clipboard_bytes,
                catalog_changes: self.smoke_state.catalog_changes,
                interaction_responses: self.smoke_state.interaction_responses,
                import_completions: self.smoke_state.import_completions,
                import_cancellations: self.smoke_state.import_cancellations,
                latest_frame,
                editor_revisions: self.smoke_state.editor_revision,
                interaction_revisions: self.smoke_state.interaction_revision,
                import_revisions: self.smoke_state.import_revision,
                terminal: self.smoke_state.terminal.clone(),
                imports: self.smoke_state.imports.clone(),
                visual: self.smoke_state.visuals.clone(),
                window_resize: self.smoke_state.window_resize,
            },
        }
    }
}

pub(crate) fn frame_contains(view_model: &AppViewModel, tab: Option<TabId>, needle: &str) -> bool {
    let Some(frame) =
        active_session(view_model, tab).and_then(|session| view_model.latest_frames.get(&session))
    else {
        return false;
    };
    frame.rows.iter().any(|row| {
        row.cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
            .contains(needle)
    })
}

fn active_session(view_model: &AppViewModel, active_tab: Option<TabId>) -> Option<SessionId> {
    active_tab
        .and_then(|active| {
            view_model
                .workspace
                .tabs
                .iter()
                .find(|tab| tab.id == active)
        })
        .or_else(|| view_model.workspace.active_tab())
        .and_then(|tab| tab.pane_tree.session_id(tab.active_pane).ok().flatten())
}
