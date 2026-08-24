use std::collections::BTreeSet;

use crate::{
    SmokeAction, SmokeBindingEvidence, SmokeCounters, SmokeVisualEvidence, SmokeVisualFacts,
    smoke_driver_completion::{CompletionContext, action_is_complete},
    smoke_driver_observation::SmokeObservation,
};

#[test]
fn visual_checkpoint_requires_complete_facts_and_verified_main_window_binding() {
    let before = SmokeCounters::default();
    let action = SmokeAction::VisualCheckpoint;
    let mut observed = observation(SmokeCounters {
        visual: Some(SmokeVisualEvidence {
            facts: passing_visual_facts(),
            png: None,
        }),
        ..Default::default()
    });
    observed.visual_checkpoint_complete = true;
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
    observed.binding = Some(SmokeBindingEvidence {
        verified: false,
        component_verified: true,
        actual_label: Some("main_window".into()),
        ..Default::default()
    });
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
    observed.binding.as_mut().unwrap().verified = true;
    assert!(action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
}

fn observation(counters: SmokeCounters) -> SmokeObservation {
    SmokeObservation {
        window_realized: false,
        editor_open: false,
        sidebar_selection: None,
        connection_panes: BTreeSet::new(),
        import_preview_ready: false,
        active_tab: None,
        tab_ids: Vec::new(),
        shutdown_complete: false,
        active_interaction: None,
        answered_prompts: Vec::new(),
        last_interaction_response: None,
        visual_checkpoint_complete: false,
        binding: None,
        counters,
    }
}

fn passing_visual_facts() -> SmokeVisualFacts {
    SmokeVisualFacts {
        requested_width: 1_360,
        requested_height: 860,
        realized_width: 1_360,
        realized_height: 852,
        command_bar: true,
        dense_sidebar: true,
        tab_strip: true,
        pane_command_row: true,
        terminal_canvas: true,
        content_dialog: true,
        embedded_icon_count: 13,
        focus_or_selection_treatment: true,
    }
}
