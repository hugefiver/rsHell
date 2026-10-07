use super::*;
use std::cell::Cell;

fn failed(phase: FailurePhase) -> FrameFailure {
    FrameFailure {
        phase,
        paints: 126,
        predicate: Some(false),
        elapsed_ms: 2_000,
        budget_ms: 2_000,
    }
}

fn facts(width: i32) -> Facts {
    Facts {
        allocation: [0, 0, width, 384],
        content: [width, 384],
        relative_outer: Some([0.0, 0.0, width as f32, 384.0]),
        visible: true,
        mapped: true,
        request_mode: 2,
    }
}

#[derive(Default)]
struct Fake {
    events: Vec<(usize, i32)>,
    missing: bool,
}

impl Provider for Fake {
    fn capture(&mut self) -> Captured {
        // Distinct capture events for every fact; measurements must come after all ten.
        for node in 0..10 {
            self.events.push((node, -2));
        }
        if self.missing {
            return Captured::default();
        }
        Captured {
            version: [4, 18, 5],
            viewport: Some(facts(630)),
            body: Some(facts(630)),
            children: std::array::from_fn(|i| Some(facts(if i == 7 { 0 } else { 100 + i as i32 }))),
            children_capped: true,
            policies_hv: Some([1, 1]),
            adjustment_luvp: Some([0.0, 357.0, 0.0, 357.0]),
        }
    }

    fn vertical_measure(&mut self, node: usize, width: i32) -> Option<[i32; 4]> {
        self.events.push((node, width));
        // Simulate cache refresh: cannot overwrite the already captured request mode/allocation.
        Some([if width == -1 { 171 } else { 384 }, 384, -1, -1])
    }
}

#[test]
fn successful_opt_out_non_target_and_wrong_phase_never_construct_provider() {
    let calls = Cell::new(0);
    let failure = Some(failed(FailurePhase::AfterPaint));
    let clock_failure = Some(failed(FailurePhase::FrameClock));
    for (windows, opt_in, mode, case, failure) in [
        (true, Some("1"), 1, 5, None),
        (false, Some("1"), 1, 5, failure),
        (true, None, 1, 5, failure),
        (true, Some("0"), 1, 5, failure),
        (true, Some("true"), 1, 5, failure),
        (true, Some("1 "), 1, 5, failure),
        (true, Some("1"), 2, 5, failure),
        (true, Some("1"), 3, 5, failure),
        (true, Some("1"), 1, 4, failure),
        (true, Some("1"), 1, 6, failure),
        (true, Some("1"), 1, 5, clock_failure),
    ] {
        assert!(
            report_if(windows, opt_in.map(OsStr::new), mode, case, failure, || {
                calls.set(calls.get() + 1);
                Fake::default()
            })
            .is_none()
        );
    }
    assert_eq!(calls.get(), 0);
    let output = report_if(true, Some(OsStr::new("1")), 1, 5, failure, || {
        calls.set(calls.get() + 1);
        Fake::default()
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(output.matches("NATIVE_MODAL_MEASUREMENT").count(), 1);
}

#[test]
fn complete_capture_precedes_body_then_positive_width_child_measurements() {
    let mut fake = Fake::default();
    let output = collect(&mut fake);
    let mut expected: Vec<_> = (0..10).map(|i| (i, -2)).collect();
    expected.extend([(1, 630), (1, -1)]);
    expected.extend((0..7).map(|i| (i + 2, 100 + i as i32)));
    assert_eq!(fake.events, expected);
    assert!(output.contains("vertical_for_size=630 min_nat_baselines=[384, 384, -1, -1] vertical_for_size=-1 min_nat_baselines=[171, 384, -1, -1]"));
    assert!(output.contains("node=9 relative=2 allocation=[0, 0, 0, 384]"));
    assert!(output.contains("vertical_for_size=0 min_nat_baselines=unavailable"));
    assert!(output.contains("request_mode=2"));
}

#[test]
fn fixed_numeric_fields_budget_and_unavailable_do_not_mask_original_failure() {
    for missing in [false, true] {
        let mut fake = Fake {
            missing,
            ..Default::default()
        };
        let output = collect(&mut fake);
        assert_eq!(output.lines().count(), 11);
        assert_eq!(output.matches("node=").count(), 10);
        assert!(!output.contains("node=10"));
        assert!(output.contains(
            "post_verdict=1 cache_affecting=1 pre_measure_capture=1 historical_cache_proof=0"
        ));
        for forbidden in [
            "0x", "Widget", "password", "secret", "title", "text", "error",
        ] {
            assert!(!output.contains(forbidden));
        }
        if missing {
            assert_eq!(fake.events.len(), 10); // Capture only; no measure on missing widgets.
            assert!(output.contains(
                "node=0 relative=0 unavailable policies_hv=unavailable adjustment_luvp=unavailable"
            ));
            assert!(output.contains("node=1 relative=1 unavailable vertical_for_size=unavailable"));
        } else {
            assert!(output.contains("gtk=[4, 18, 5] child_cap=8 children_capped=true"));
            assert!(output.contains("policies_hv=[1, 1] adjustment_luvp=[0.0, 357.0, 0.0, 357.0]"));
        }
    }
}

#[test]
fn source_contract_preserves_historical_output_disconnect_report_and_original_assert() {
    let frames = include_str!("fluent_frames.rs");
    let after_disconnect = frames.split("clock.disconnect(signal);").nth(1).unwrap();
    assert!(
        after_disconnect.find("report_failure(").unwrap()
            < after_disconnect.find("assert!(").unwrap()
    );
    assert!(frames.contains("report: impl FnOnce(FrameFailure)"));
    let diagnostics = include_str!("fluent_interaction_diagnostics.rs");
    let report = diagnostics
        .split("pub fn report(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn wait")
        .next()
        .unwrap();
    let historical = report.find("eprint!(\"{output}\");").unwrap();
    assert!(
        report
            .find("let final_state = measurements::observe(root);")
            .unwrap()
            < historical
    );
    assert!(report.find(".failure_lines(").unwrap() < historical);
    assert!(historical < report.find("measurement::report(").unwrap());
    assert_eq!(diagnostics.matches("measurement::report(").count(), 1);
    let provider = include_str!("fluent_interaction_measurement_gtk.rs");
    let capture = provider
        .split("fn capture(")
        .nth(1)
        .unwrap()
        .split("fn vertical_measure")
        .next()
        .unwrap();
    assert!(!capture.contains(".measure("));
    assert!(provider.contains("while children.len() < CHILD_CAP"));
    assert!(provider.contains("children_capped: next.is_some()"));
    for source in [include_str!("fluent_interaction_measurement.rs"), provider] {
        for forbidden in [
            "unwrap(",
            "expect(",
            "grab_focus",
            "set_value",
            "clamp_page",
            "connect_",
            "iteration(",
            "first_open_ready",
            "set_property",
            ".text(",
            ".title(",
            "type_",
            "snapshot",
            "queue_draw",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
}

#[test]
fn diagnostic_support_files_obey_root_pure_loc_rule_including_tests() {
    for source in [
        include_str!("fluent_interaction_diagnostics.rs"),
        include_str!("fluent_interaction_diagnostics_tests.rs"),
        include_str!("fluent_interaction_measurement.rs"),
        include_str!("fluent_interaction_measurement_gtk.rs"),
        include_str!("fluent_interaction_measurement_tests.rs"),
    ] {
        let count = source
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty() && !line.starts_with("//")
            })
            .count();
        assert!(
            count <= 250,
            "diagnostic support has {count} pure lines including tests"
        );
    }
}
