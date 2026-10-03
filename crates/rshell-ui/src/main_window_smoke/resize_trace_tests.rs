use super::*;

fn sample(default: Option<(i32, i32)>) -> Snapshot {
    Snapshot {
        default,
        widget: Some((0, 0)),
        surface: Some((1, 1, 1)),
        widget_scale: Some(1),
        mapped: Some(true),
        realized: Some(true),
        mode: Some(ShellLayoutMode::Compact),
    }
}

fn ledger() -> Ledger {
    Ledger::new((800, 600), ShellLayoutMode::Compact)
}

#[test]
fn records_ordered_boundaries_and_first_observed_change_without_inventing_a_signal() {
    let mut ledger = ledger();
    let before = sample(Some((800, 600)));
    assert!(
        ledger
            .record(Event::Before, 0, before)
            .unwrap()
            .contains("seq=1 elapsed_ms=0")
    );
    let after = sample(Some((1920, 1152)));
    let line = ledger.record(Event::AfterRoute, 7, after).unwrap();
    assert!(line.contains("seq=2 elapsed_ms=7"));
    assert!(line.contains("previous_observed_default=Some((800, 600))"));
    assert_eq!(ledger.first_observed_default_change, Some((2, 7)));
    assert!(!ledger.notify_width);
    assert!(!ledger.notify_height);
    let terminal = ledger.terminal(10_000, after, "terminal_failed").unwrap();
    assert!(terminal.contains("first_observed_default_change_seq_ms=Some((2, 7))"));
    assert!(terminal.contains("notify_width_seen=false notify_height_seen=false"));
    assert!(terminal.contains("total=3"));
}

#[test]
fn notify_map_and_allocation_keep_causal_order_and_distinct_values() {
    let mut ledger = ledger();
    let before = sample(Some((800, 600)));
    ledger.record(Event::Before, 0, before);
    let width = sample(Some((1920, 600)));
    assert!(
        ledger
            .record(Event::NotifyWidth, 1, width)
            .unwrap()
            .contains("seq=2")
    );
    let height = sample(Some((1920, 1152)));
    ledger.record(Event::NotifyHeight, 2, height);
    ledger.record(Event::Map, 3, height);
    let allocation = ledger.record(Event::Allocation, 4, height).unwrap();
    assert!(allocation.contains("seq=5"));
    assert_eq!(ledger.first_observed_default_change, Some((2, 1)));
    assert!(ledger.notify_width && ledger.notify_height);
}

#[test]
fn refresh_tick_repeats_collapse_but_a_changed_sample_is_never_collapsed() {
    let mut ledger = ledger();
    let first = sample(Some((800, 600)));
    let changed = sample(Some((1920, 1152)));
    ledger.record(Event::TickEnter, 0, first);
    ledger.record(Event::RefreshEnter, 1, first);
    ledger.record(Event::RefreshExit, 2, first);
    ledger.record(Event::TickExit, 3, first);
    assert!(ledger.record(Event::TickEnter, 4, first).is_none());
    assert!(ledger.record(Event::RefreshEnter, 5, changed).is_some());
    assert_eq!(ledger.repeated, 1);
    assert_eq!(ledger.first_observed_default_change, Some((6, 5)));
}

#[test]
fn detail_budget_reserves_terminal_and_counts_dropped_events() {
    let mut ledger = ledger();
    let same = sample(Some((800, 600)));
    for index in 0..53 {
        let event = if index % 2 == 0 {
            Event::Map
        } else {
            Event::Unmap
        };
        let output = ledger.record(event, index, same);
        assert_eq!(output.is_some(), index < u128::from(DETAIL_LIMIT));
    }
    assert_eq!((ledger.details, ledger.dropped, ledger.total), (48, 5, 53));
    let terminal = ledger.terminal(60, same, "terminal_route_error").unwrap();
    assert!(terminal.contains("total=54 detailed=48 dropped=5 repeated=0"));
    assert!(ledger.terminal(61, same, "terminal_failed").is_none());
    assert!(ledger.record(Event::Map, 62, same).is_none());
}

#[test]
fn missing_samples_cannot_establish_a_default_change_or_an_internal_cause() {
    let mut ledger = ledger();
    ledger.record(Event::Before, 0, sample(None));
    ledger.record(Event::Map, 1, sample(Some((1920, 1152))));
    let terminal = ledger
        .terminal(2, sample(Some((1920, 1152))), "terminal_passed")
        .unwrap();
    assert!(terminal.contains("first_observed_default_change_seq_ms=None"));
    assert!(terminal.contains("notify_width_seen=false notify_height_seen=false"));
}

#[test]
fn trace_teardown_releases_state_and_terminal_does_not_rearm() {
    let _: fn(&mut ResizeTrace, &gtk::ApplicationWindow) = ResizeTrace::attach;
    let mut trace = ResizeTrace::new((800, 600), ShellLayoutMode::Compact);
    let weak = Rc::downgrade(&trace.ledger);
    trace.terminal("terminal_route_error", sample(None));
    assert!(
        trace
            .ledger
            .borrow_mut()
            .record(Event::Map, 2, sample(None))
            .is_none()
    );
    drop(trace);
    assert!(weak.upgrade().is_none());
}

#[test]
fn fixed_snapshot_grammar_cannot_echo_arbitrary_strings() {
    let mut ledger = ledger();
    let line = ledger.record(Event::Before, 0, sample(None)).unwrap();
    let secret = "fake/path\nPRIVATE_HOST\x1b[31m";
    assert!(!line.contains(secret));
    assert!(!line.contains("PRIVATE_HOST"));
    assert!(!line.contains('\x1b'));
}
