use std::collections::BTreeSet;

use rshell_core::{ConnectionId, InteractionId, TabId};

use crate::{SmokeAction, SmokeBindingEvidence, SmokeCounters};

#[derive(Clone)]
pub(crate) struct SmokeBindingRequest {
    pub(crate) action: SmokeAction,
    pub(crate) surface: Option<String>,
    pub(crate) connection: Option<String>,
}

pub(crate) struct SmokeObservation {
    pub window_realized: bool,
    pub editor_open: bool,
    pub sidebar_selection: Option<ConnectionId>,
    pub connection_panes: BTreeSet<ConnectionId>,
    pub import_preview_ready: bool,
    pub active_tab: Option<TabId>,
    pub tab_ids: Vec<TabId>,
    pub shutdown_complete: bool,
    pub active_interaction: Option<InteractionId>,
    pub answered_prompts: Vec<usize>,
    pub last_interaction_response: Option<InteractionId>,
    pub visual_checkpoint_complete: bool,
    pub binding: Option<SmokeBindingEvidence>,
    pub counters: SmokeCounters,
}
