use super::*;

#[test]
fn window_readiness_requires_realized_mapped_positive_widget_allocation() {
    let before = SmokeCounters::default();
    let mut observed = observation(before.clone());
    observed.binding = Some(SmokeBindingEvidence {
        verified: true,
        component_verified: true,
        ..Default::default()
    });
    for (realized, mapped, allocation, expected) in [
        (false, false, (0, 0), false),
        (false, true, (800, 600), false),
        (true, false, (0, 0), false),
        (true, false, (800, 600), false),
        (true, true, (0, 0), false),
        (true, true, (0, 600), false),
        (true, true, (800, 0), false),
        (true, true, (-1, 600), false),
        (true, true, (800, -1), false),
        (true, true, (1, 1), true),
        (true, true, (800, 600), true),
    ] {
        observed.window_realized = realized;
        observed.window_mapped = mapped;
        observed.window_allocation = allocation;
        for binding_required in [false, true] {
            let mut context = CompletionContext::new(&before, &observed);
            context.binding_required = binding_required;
            assert_eq!(
                action_is_complete(&SmokeAction::WaitWindowRealized, &context, |_| false),
                expected,
                "realized={realized} mapped={mapped} allocation={allocation:?} binding={binding_required}"
            );
        }
    }
}

#[test]
fn window_readiness_zero_allocation_cannot_complete_with_realized_only_binding() {
    let before = SmokeCounters::default();
    let mut observed = observation(before.clone());
    observed.window_realized = true;
    observed.window_mapped = true;
    observed.binding = Some(SmokeBindingEvidence {
        verified: true,
        component_verified: true,
        ..Default::default()
    });
    assert!(!action_is_complete(
        &SmokeAction::WaitWindowRealized,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
    observed.window_allocation = (800, 600);
    assert!(action_is_complete(
        &SmokeAction::WaitWindowRealized,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
    observed.binding = None;
    assert!(!action_is_complete(
        &SmokeAction::WaitWindowRealized,
        &CompletionContext::new(&before, &observed).require_binding(),
        |_| false,
    ));
}
