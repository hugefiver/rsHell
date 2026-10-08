use super::*;
use rshell_core::{
    AppBootstrapState, AppSettings, AppViewModel, PaneLaunchTarget, PaneTree, TabState,
    TerminalProfile,
};

fn model() -> PaneHostModel {
    PaneHostModel::new(AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: AppSettings::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    }))
}
fn identity() -> Identity {
    Identity {
        tab: TabId::new_v4(),
        pane: PaneId::new(),
        session: SessionId::new(),
    }
}
fn add(model: &mut PaneHostModel, id: Identity, state: SessionState) {
    let mut view = model.view_model().clone();
    view.workspace.tabs.push(TabState {
        id: id.tab,
        title: "owned receipt".into(),
        active_pane: id.pane,
        pane_tree: PaneTree::with_session(id.pane, id.session),
    });
    view.workspace.active_tab = Some(id.tab);
    view.pane_launches.insert(id.pane, PaneLaunchTarget::Local);
    view.session_states.insert(id.session, state);
    model.replace_view_model(view);
}
#[test]
fn snapshot_never_invents_completion() {
    let mut model = model();
    let mut policy = FocusPolicy::default();
    policy.begin_new();
    add(&mut model, identity(), SessionState::Connected);
    policy.synchronize(&model);
    assert!(policy.active());
    assert_eq!(policy.bound(), None);
}
#[test]
fn receipt_before_view_waits_past_old_selected_baseline() {
    let mut model = model();
    let old = identity();
    add(&mut model, old, SessionState::Connected);
    let mut policy = FocusPolicy::default();
    policy.synchronize(&model);
    let generation = policy.begin_new();
    let exact = identity();
    policy.created(generation, exact);
    policy.synchronize(&model);
    assert!(policy.active());
    assert!(!policy.observed());
    add(&mut model, exact, SessionState::Connecting);
    policy.synchronize(&model);
    assert_eq!(policy.bound(), Some(exact));
    assert!(policy.observed());
    model.apply_session_event(
        exact.session,
        rshell_core::SessionUiEvent::State(SessionState::Connected),
    );
    policy.synchronize(&model);
    assert_eq!(policy.bound(), Some(exact));
}
#[test]
fn view_before_receipt_uses_only_its_exact_identity() {
    let mut model = model();
    let mut policy = FocusPolicy::default();
    let gen_a = policy.begin_new();
    let gen_c = policy.begin_new();
    let b = identity();
    let c = identity();
    add(&mut model, b, SessionState::Connected);
    add(&mut model, c, SessionState::Connected);
    policy.synchronize(&model);
    policy.created(gen_a, b);
    assert_eq!(policy.bound(), None);
    policy.created(gen_c, c);
    assert_eq!(policy.bound(), Some(c));
}
#[test]
fn cancellation_and_away_back_do_not_revive_old_receipts() {
    let mut model = model();
    let mut policy = FocusPolicy::default();
    let generation = policy.begin_new();
    policy.cancel();
    let a = identity();
    add(&mut model, a, SessionState::Connected);
    policy.synchronize(&model);
    policy.created(generation, a);
    assert!(!policy.active());
    let current = policy.begin_new();
    policy.created(generation, a);
    assert_eq!(policy.bound(), None);
    policy.created(current, a);
    assert_eq!(policy.bound(), Some(a));
}
#[test]
fn failure_has_no_permanent_state_and_new_c_can_focus() {
    let mut model = model();
    let mut policy = FocusPolicy::default();
    let a = policy.begin_new();
    policy.cancel();
    let c_gen = policy.begin_new();
    let c = identity();
    add(&mut model, c, SessionState::Connected);
    policy.synchronize(&model);
    policy.created(a, identity());
    policy.created(c_gen, c);
    assert_eq!(policy.bound(), Some(c));
}
#[test]
fn observed_target_removal_replacement_error_and_other_selection_cancel() {
    for kind in 0..4 {
        let mut model = model();
        let mut policy = FocusPolicy::default();
        let a = identity();
        add(&mut model, a, SessionState::Connected);
        policy.synchronize(&model);
        let generation = policy.begin_new();
        policy.created(generation, a);
        assert!(policy.observed());
        match kind {
            0 => {
                let mut v = model.view_model().clone();
                v.workspace.tabs.clear();
                v.workspace.active_tab = None;
                model.replace_view_model(v);
            }
            1 => {
                let mut v = model.view_model().clone();
                v.workspace.tabs[0]
                    .pane_tree
                    .replace_session(a.pane, Some(SessionId::new()))
                    .unwrap();
                model.replace_view_model(v);
            }
            2 => {
                model.apply_session_event(
                    a.session,
                    rshell_core::SessionUiEvent::State(SessionState::Failed),
                );
            }
            _ => add(&mut model, identity(), SessionState::Connected),
        }
        policy.synchronize(&model);
        assert!(!policy.active());
    }
}
