use std::sync::Arc;

use rshell_core::{
    CellAttributes, Color, RenderCell, RenderFrame, RenderRow, SessionId, TerminalSize,
};

use crate::{
    MainWindowMsg, SmokeTerminalEvidence,
    main_window_smoke::queue_visual_completion_tick,
    main_window_smoke_evidence::{
        P0_TUI_ACTIVE_TITLE, selection_frame_confirms, track_tui_transition, tui_frame_is_active,
    },
    main_window_smoke_resize::prepared_smoke_resize,
    main_window_smoke_terminal_effects::{
        frame_contains_text, has_new_marker_occurrence, marker_occurrences,
    },
    main_window_smoke_visual::{
        VisualCheckpointPhase, visual_checkpoint_binding, visual_checkpoint_component_verified,
    },
    smoke_driver_visual_tests::passing_visual_evidence,
};

#[test]
fn stale_timer_does_not_panic_after_main_window_shutdown() {
    let (sender, receiver) = relm4::channel();
    drop(receiver);

    let _ = sender.send(MainWindowMsg::SmokeTick);
}

#[test]
fn visual_completion_enqueues_one_component_tick_with_verified_binding() {
    let mut pending = true;
    let mut queued = Vec::new();

    assert!(queue_visual_completion_tick(&mut pending, |message| {
        queued.push(message);
    }));
    assert!(!pending);
    assert!(!queue_visual_completion_tick(&mut pending, |message| {
        queued.push(message);
    }));
    assert_eq!(queued.len(), 1, "completion tick must enqueue once");
    assert!(matches!(queued.pop(), Some(MainWindowMsg::SmokeTick)));

    let visual = passing_visual_evidence();
    let component_verified =
        visual_checkpoint_component_verified(VisualCheckpointPhase::Complete, Some(&visual));
    let binding = visual_checkpoint_binding(Some("gtk"), None, component_verified);
    assert!(binding.verified && binding.component_verified);
}

#[test]
fn selection_only_frame_with_the_same_terminal_generation_is_real_evidence() {
    let session = SessionId::new();
    let frame = RenderFrame {
        generation: 9,
        size: TerminalSize {
            cols: 1,
            rows: 1,
            pixel_width: 8,
            pixel_height: 16,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from([RenderCell {
                text: "x".into(),
                width: 1,
                foreground: Color::Default,
                background: Color::Default,
                attributes: CellAttributes::default(),
                selected: true,
            }]),
        }]),
        cursor: None,
        title: String::new(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    };

    assert!(selection_frame_confirms(session, session, &frame));
}

#[test]
fn prepared_resize_ignores_mismatched_automatic_command_and_accepts_exact_command() {
    let prepared = prepared_smoke_resize(960, 640, 1.0).expect("prepared resize");
    let mismatched = TerminalSize {
        cols: 106,
        rows: 35,
        pixel_width: 959,
        pixel_height: 640,
        dpi: 96,
    };
    assert!(!prepared.matches(mismatched));
    assert!(prepared.matches(TerminalSize {
        cols: 106,
        rows: 35,
        pixel_width: 960,
        pixel_height: 640,
        dpi: 96,
    }));
}

#[test]
fn paste_effect_requires_a_new_stable_marker_occurrence_after_command_echo() {
    let marker = "p0-paste-effect";
    let echoed_command = smoke_frame(&[(7, marker)]);
    let baseline = marker_occurrences(&echoed_command, marker);

    // The former `frame_contains_text` rule would accept the pre-paste command echo.
    assert!(frame_contains_text(&echoed_command, marker));
    assert!(!has_new_marker_occurrence(
        &echoed_command,
        marker,
        &baseline
    ));

    let post_paste_output = smoke_frame(&[(7, marker), (8, marker)]);
    assert!(has_new_marker_occurrence(
        &post_paste_output,
        marker,
        &baseline
    ));
}

#[test]
fn tui_evidence_requires_an_alternate_screen_transition_on_one_session() {
    let local = SessionId::new();
    let unrelated = SessionId::new();
    let mut terminal = SmokeTerminalEvidence::default();
    let mut tracked = None;

    track_tui_transition(&mut terminal, &mut tracked, local, false);
    assert!(!terminal.tui_entered);
    assert!(!terminal.tui_exited);

    track_tui_transition(&mut terminal, &mut tracked, local, true);
    assert_eq!(tracked, Some(local));
    assert!(terminal.tui_entered);
    assert!(!terminal.tui_exited);

    track_tui_transition(&mut terminal, &mut tracked, unrelated, false);
    assert!(!terminal.tui_exited);

    track_tui_transition(&mut terminal, &mut tracked, local, false);
    assert!(terminal.tui_exited);
}

#[test]
fn tui_evidence_accepts_the_fixture_title_when_conpty_consumes_screen_switches() {
    let mut frame = smoke_frame(&[(7, "P0-TUI-ENTERED")]);
    frame.title = P0_TUI_ACTIVE_TITLE.into();
    assert!(tui_frame_is_active(&frame));

    frame.title = "rshell-p0-tui-exited".into();
    assert!(!tui_frame_is_active(&frame));
}

fn smoke_frame(rows: &[(i64, &str)]) -> RenderFrame {
    RenderFrame {
        generation: 1,
        size: TerminalSize {
            cols: 80,
            rows: 24,
            pixel_width: 720,
            pixel_height: 432,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from(
            rows.iter()
                .map(|(stable_row, text)| RenderRow {
                    stable_row: *stable_row,
                    wrapped: false,
                    cells: Arc::from(
                        text.chars()
                            .map(|character| RenderCell {
                                text: character.to_string(),
                                width: 1,
                                foreground: Color::Default,
                                background: Color::Default,
                                attributes: CellAttributes::default(),
                                selected: false,
                            })
                            .collect::<Vec<_>>(),
                    ),
                })
                .collect::<Vec<_>>(),
        ),
        cursor: None,
        title: String::new(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    }
}
