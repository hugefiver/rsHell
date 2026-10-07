use super::*;
use crate::fluent_native::FailurePhase;

fn failure(phase: FailurePhase) -> FrameFailure {
    FrameFailure {
        phase,
        paints: 127,
        predicate: Some(false),
        elapsed_ms: 2_001,
        budget_ms: 2_000,
    }
}

#[test]
fn bounded_changes_keep_initial_and_final_without_duplicate_paints() {
    let mut history = History::new(State::default());
    for n in 1..=100 {
        let state = State {
            first: Some((true, true, n, 36)),
            ..Default::default()
        };
        history.sample(n as u128, state.clone());
        history.sample(n as u128, state);
    }
    assert_eq!(history.changes.len(), 32);
    assert_eq!(history.samples, 200);
    assert_eq!(history.transitions, 100);
    let output = history.failure_lines(
        1,
        5,
        failure(FailurePhase::AfterPaint),
        (2_002, history.last.clone()),
    );
    assert_eq!(output.lines().count(), 35);
    assert!(output.contains("retained=32 dropped=68"));
    assert!(output.contains("initial t_ms=0 State {"));
    assert!(output.contains("final t_ms=2002 State {"));
    assert!(output.contains("first: Some((true, true, 100, 36))"));
}

#[test]
fn fixed_context_never_echoes_unknown_payloads_and_phases_are_distinct() {
    let history = History::new(State::default());
    for (phase, code) in [(FailurePhase::FrameClock, 1), (FailurePhase::AfterPaint, 2)] {
        let output = history.failure_lines(
            fixed_mode("private\npayload"),
            fixed_case("private\npayload"),
            failure(phase),
            (2_002, State::default()),
        );
        assert!(output.starts_with(&format!("NATIVE_MODAL_FAILURE mode=0 case=0 phase={code} paints=127 predicate=Some(false) elapsed_ms=2001 budget_ms=2000")));
        assert!(!output.contains("private"));
        assert!(!output.contains("0x"));
    }
    assert_eq!(
        [
            fixed_mode("compact"),
            fixed_mode("standard"),
            fixed_mode("wide")
        ],
        [1, 2, 3]
    );
    for (n, case) in [
        "host-unknown-reject",
        "host-unknown-accept",
        "host-changed-close",
        "host-unknown-escape",
        "password",
        "passphrase",
        "keyboard",
        "auth-cancel",
        "auth-escape-adversarial",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(fixed_case(case), n as u8 + 1);
    }
}

#[test]
fn original_compound_predicate_and_outer_oracle_are_unchanged() {
    let interactions = include_str!("fluent_interactions.rs");
    assert!(interactions.contains(
        "modal_ready(root, \"interaction-dialog\") && measurements::first_open_ready(root)"
    ));
    let layout = include_str!("fluent_interaction_layout.rs").replace("\r\n", "\n");
    let predicate = layout
        .split("pub(super) fn first_open_ready")
        .nth(1)
        .unwrap()
        .split("pub(super) fn verify")
        .next()
        .unwrap();
    assert!(predicate.contains("input.x() >= visible.x() - 2.0\n        && input.y() >= visible.y() - 2.0\n        && input.x() + input.width() <= visible.x() + visible.width() + 2.0\n        && input.y() + input.height() <= visible.y() + visible.height() + 2.0"));
    for forbidden in [
        "grab_focus",
        "set_value",
        "clamp_page",
        "sleep",
        "queue_draw",
    ] {
        assert!(!predicate.contains(forbidden));
        assert!(!include_str!("fluent_interaction_diagnostics.rs").contains(forbidden));
    }
}
