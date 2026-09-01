use std::collections::BTreeSet;

use crate::{
    ShellLayoutMode, SmokeAccessibilityEvidence, SmokeAction, SmokeBindingEvidence, SmokeCounters,
    SmokeDpiEvidence, SmokePngEvidence, SmokeVisualCheckpoint, SmokeVisualCheckpointEvidence,
    SmokeVisualFacts, SmokeVisualState,
    smoke_driver_completion::{CompletionContext, action_is_complete},
    smoke_driver_observation::SmokeObservation,
};

#[test]
fn visual_checkpoint_uses_exact_persisted_evidence_and_verified_main_window_binding() {
    let before = SmokeCounters::default();
    let checkpoint = SmokeVisualCheckpoint {
        id: "standard-connected".into(),
        state: SmokeVisualState::Connected,
        width: 1_360,
        height: 860,
        expected_mode: ShellLayoutMode::Standard,
    };
    let action = SmokeAction::VisualCheckpoint(checkpoint.clone());
    let mut observed = observation(SmokeCounters::default());
    observed.binding = Some(SmokeBindingEvidence {
        verified: true,
        component_verified: true,
        actual_label: Some("main_window".into()),
        ..Default::default()
    });
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    observed
        .counters
        .visual
        .insert("wrong-key".into(), passing_visual_evidence());
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    let mut failing = passing_visual_evidence();
    failing.png.non_empty = false;
    observed
        .counters
        .visual
        .insert(checkpoint.id.clone(), failing);
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    let mut wrong = passing_visual_evidence();
    wrong.checkpoint_id = "wrong-id".into();
    observed
        .counters
        .visual
        .insert(checkpoint.id.clone(), wrong);
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    let mut wrong = passing_visual_evidence();
    wrong.state = SmokeVisualState::Empty;
    observed
        .counters
        .visual
        .insert(checkpoint.id.clone(), wrong);
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    let mut wrong = passing_visual_evidence();
    wrong.layout = ShellLayoutMode::Compact;
    observed
        .counters
        .visual
        .insert(checkpoint.id.clone(), wrong);
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    observed
        .counters
        .visual
        .insert(checkpoint.id.clone(), passing_visual_evidence());
    observed.binding.as_mut().unwrap().verified = false;
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    observed.binding.as_mut().unwrap().verified = true;
    observed.binding.as_mut().unwrap().component_verified = false;
    assert!(!action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));

    observed.binding.as_mut().unwrap().component_verified = true;
    assert!(action_is_complete(
        &action,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
}

#[test]
fn visual_contract_rejects_terminal_clipping_and_insufficient_line_separation() {
    let mut evidence = passing_visual_evidence();
    evidence.facts.terminal_glyph_clipped_cells = 1;
    assert!(!evidence.contract_passes());

    evidence.facts.terminal_glyph_clipped_cells = 0;
    evidence.facts.terminal_min_line_separation_bits = 0.5_f64.to_bits();
    assert!(!evidence.contract_passes());

    evidence.facts.terminal_min_line_separation_bits = 2.0_f64.to_bits();
    assert!(evidence.contract_passes());
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
        binding: None,
        counters,
    }
}

pub(crate) fn passing_visual_evidence() -> SmokeVisualCheckpointEvidence {
    let facts = passing_visual_facts();
    SmokeVisualCheckpointEvidence {
        checkpoint_id: "standard-connected".into(),
        state: SmokeVisualState::Connected,
        layout: ShellLayoutMode::Standard,
        facts,
        png: SmokePngEvidence {
            width: 1_360,
            height: 852,
            non_empty: true,
            luminance_buckets: 4,
            dark_regions_required: 4,
            dark_regions_passed: 4,
            focus_or_selection_thickness_px: 2,
        },
        dpi: SmokeDpiEvidence {
            logical_width: 1_360,
            logical_height: 852,
            effective_scale: 1.0,
            effective_dpi: 96.0,
            cell_width: 9.0,
            cell_height: 18.0,
            icon_logical_size: 16,
            icon_texture_width: 16,
            icon_texture_height: 16,
            dpi_fallback_used: false,
        },
        accessibility: SmokeAccessibilityEvidence::default(),
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
        content_dialog: false,
        embedded_icon_count: 13,
        icon_logical_size: 16,
        icon_texture_width: 16,
        icon_texture_height: 16,
        icon_backend: Some(crate::IconBackend::InternalVector),
        effective_scale_bits: 1.0_f64.to_bits(),
        effective_dpi_bits: 96.0_f64.to_bits(),
        measured_cell_width_bits: 9.0_f64.to_bits(),
        measured_cell_height_bits: 18.0_f64.to_bits(),
        dpi_fallback_used: false,
        focus_or_selection_treatment: true,
        terminal_glyph_clipped_cells: 0,
        terminal_min_line_separation_bits: 2.0_f64.to_bits(),
    }
}
