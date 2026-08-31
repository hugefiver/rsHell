use rshell_ui::{PaneAction, PaneActionLayout, TabOverflowModel};

#[test]
fn twenty_tabs_keep_hidden_tabs_and_the_active_tab_in_overflow() {
    let model = TabOverflowModel::new(20, Some(2), &[0, 1, 2, 3, 20, 99]);

    assert_eq!(model.active_index, Some(2));
    assert_eq!(
        model.overflow_indices,
        std::iter::once(2).chain(4..20).collect::<Vec<_>>()
    );
    assert_eq!(model.cycle(1), Some(3));
    assert_eq!(model.cycle(-1), Some(1));
}

#[test]
fn keyboard_cycle_wraps_across_all_twenty_tabs() {
    let first = TabOverflowModel::new(20, Some(0), &[]);
    let last = TabOverflowModel::new(20, Some(19), &[]);

    assert_eq!(first.cycle(-1), Some(19));
    assert_eq!(last.cycle(1), Some(0));
    assert_eq!(first.cycle(20), Some(0));
    assert_eq!(first.cycle(41), Some(1));
    assert_eq!(TabOverflowModel::new(0, None, &[]).cycle(1), None);
    assert_eq!(TabOverflowModel::new(20, Some(20), &[]).active_index, None);
}

#[test]
fn pane_actions_follow_deterministic_width_priorities() {
    use PaneAction::*;

    let actions = [
        ResetDisplay,
        SplitHorizontal,
        SplitVertical,
        Retry,
        EditConnection,
        CopyDiagnostics,
        Close,
    ];

    assert_eq!(
        PaneActionLayout::for_width(&actions, 180),
        PaneActionLayout {
            visible: vec![ResetDisplay],
            overflow: vec![
                SplitHorizontal,
                SplitVertical,
                Retry,
                EditConnection,
                CopyDiagnostics,
                Close,
            ],
        }
    );
    assert_eq!(
        PaneActionLayout::for_width(&actions, 320),
        PaneActionLayout {
            visible: vec![ResetDisplay, SplitHorizontal, SplitVertical, Retry, Close,],
            overflow: vec![EditConnection, CopyDiagnostics],
        }
    );
    assert_eq!(
        PaneActionLayout::for_width(&actions, 600),
        PaneActionLayout {
            visible: actions.to_vec(),
            overflow: Vec::new(),
        }
    );
}

#[test]
fn edit_and_diagnostics_yield_before_recovery_and_close() {
    use PaneAction::*;

    let actions = [EditConnection, CopyDiagnostics, Reconnect, Close];
    let layout = PaneActionLayout::for_width(&actions, 180);

    assert_eq!(layout.visible, vec![Reconnect]);
    assert_eq!(
        layout.overflow,
        vec![EditConnection, CopyDiagnostics, Close]
    );

    let medium = PaneActionLayout::for_width(&actions, 320);
    assert_eq!(medium.visible, vec![Reconnect, Close]);
    assert_eq!(medium.overflow, vec![EditConnection, CopyDiagnostics]);
}
