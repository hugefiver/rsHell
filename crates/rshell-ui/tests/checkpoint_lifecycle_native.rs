use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppEvent, AppViewModel, CellAttributes, Color, ConnectionId,
    ConnectionProfile, PaneId, PaneLaunchTarget, PaneTree, RenderCell, RenderFrame, RenderRow,
    SessionId, SessionState, SessionUiCommand, SessionUiEvent, SplitAxis, TabId, TabState,
    TerminalProfile, TerminalSize, UiCommand, UiCommandPort, UiPortError, WorkspaceState,
};
use rshell_ui::{
    MainWindow, MainWindowInit, MainWindowMsg, ShellLayoutMode, SmokeAction, SmokeDriverInit,
    SmokeReportHandle, SmokeScenario, SmokeScenarioState, SmokeStep, SmokeStepState,
    SmokeVisualCheckpoint, SmokeVisualState, apply_global_css, collect_visual_facts,
    selection_treatment_surface,
};

#[path = "support/grid_bounds.rs"]
mod grid_bounds;

fn main() {
    if cfg!(target_os = "macos") {
        println!(
            "CHECKPOINT_LIFECYCLE_NATIVE_SKIP platform=macos reason=native_gui_integration_explicitly_skipped"
        );
        return;
    }
    gtk::init().expect("checkpoint lifecycle proof requires an available GTK display; do not skip");
    apply_global_css();
    let settings = gtk::Settings::default().expect("GTK settings");
    settings.set_property("gtk-enable-animations", false);
    settings.set_property("gtk-cursor-blink", false);

    let mut evidence_root = EvidenceRoot::prepare();
    let path = evidence_root.path().to_path_buf();
    println!(
        "CHECKPOINT_EVIDENCE_ROOT mode={} path={}",
        if evidence_root.is_temporary() {
            "temporary"
        } else {
            "retained"
        },
        path.display()
    );
    if std::env::var("RSHELL_CHECKPOINT_CASE").as_deref() == Ok("compact-editor-r4") {
        run_compact_editor_r4(&path);
    } else {
        run(&path);
    }
    if evidence_root.is_temporary() {
        evidence_root.cleanup();
        assert!(!path.exists(), "owned evidence root must be removed");
        println!("CHECKPOINT_EVIDENCE_CLEANUP removed={}", path.display());
    }
}

fn run(evidence_root: &Path) {
    let mut view = connected_fixture();
    let bootstrap_id = view.workspace.active_tab.expect("bootstrap tab");
    let target_profile = ConnectionProfile::new("Synthetic SSH target", "safe.example.test");
    let target_connection = target_profile.id;
    let commands = Arc::new(RecordingPort::default());
    let report_base = evidence_root.join("checkpoint-lifecycle-report.json");
    let scenario = SmokeScenario::with_steps(
        "checkpoint-lifecycle-native",
        vec![
            SmokeStep {
                surface: Some("gtk".into()),
                ..SmokeStep::new(SmokeAction::WaitWindowRealized)
            },
            SmokeStep::new(SmokeAction::ResizeWindow {
                width: 800,
                height: 600,
                expected_mode: ShellLayoutMode::Compact,
            }),
            visual_step_at(
                "compact-empty",
                SmokeVisualState::Empty,
                800,
                600,
                ShellLayoutMode::Compact,
            ),
            SmokeStep::new(SmokeAction::NewTab),
            SmokeStep::new(SmokeAction::ResizeWindow {
                width: 1_360,
                height: 860,
                expected_mode: ShellLayoutMode::Standard,
            }),
            visual_step("standard-editor", SmokeVisualState::Editor),
            visual_step("standard-settings", SmokeVisualState::Settings),
            visual_step("standard-import", SmokeVisualState::Import),
            SmokeStep::new(SmokeAction::NewTab),
            SmokeStep::new(SmokeAction::SelectConnection("Synthetic SSH target".into())),
            SmokeStep::new(SmokeAction::Connect),
            visual_step("standard-connected", SmokeVisualState::Connected),
            visual_step("standard-grid", SmokeVisualState::Grid),
        ]
        .into_iter()
        .chain(
            layout_cases()
                .into_iter()
                .flat_map(|(id, state, width, height, mode)| {
                    [
                        SmokeStep::new(SmokeAction::ResizeWindow {
                            width,
                            height,
                            expected_mode: mode,
                        }),
                        visual_step_at(&id, state, width, height, mode),
                        SmokeStep::new(SmokeAction::WaitFrameContains(format!("LAYOUT_ACK_{id}"))),
                    ]
                }),
        )
        .chain([SmokeStep::new(SmokeAction::CloseAll)])
        .collect(),
    );
    let (init, report) = MainWindowInit::new(commands.clone(), view.clone())
        .with_smoke_driver(SmokeDriverInit::new(scenario).with_png_path(&report_base));
    let main = MainWindow::builder().launch(init).detach();
    let focus_triggers = observe_pre_modal_focus(&main);
    main.widget().set_default_size(1_360, 860);
    main.widget().present();

    let mut generation = 1;
    let compact_resize_started = Instant::now();
    let compact_resize_deadline = compact_resize_started + Duration::from_secs(8);
    let trace = std::env::var_os("CI").is_some()
        || std::env::var("RSHELL_CHECKPOINT_TRACE").as_deref() == Ok("1");
    let mut timing = trace.then(|| CompactPumpTrace::new(compact_resize_started));
    let mut readiness_checked = false;
    while report.report().steps[1].state != SmokeStepState::Passed {
        pump_once_timed(
            &main,
            &commands,
            &mut view,
            &mut generation,
            &target_profile,
            timing.as_mut(),
        );
        if !readiness_checked && report.report().steps[0].state == SmokeStepState::Passed {
            let window = main.widget();
            assert!(window.is_realized() && window.is_mapped());
            assert!(window.width() > 0 && window.height() > 0);
            assert!(
                report.report().steps[0]
                    .binding
                    .as_ref()
                    .is_some_and(|binding| { binding.verified && binding.component_verified })
            );
            println!(
                "CHECKPOINT_WINDOW_READY realized=true mapped=true allocation={}x{} elapsed_ms={}",
                window.width(),
                window.height(),
                compact_resize_started.elapsed().as_millis()
            );
            readiness_checked = true;
        }
        if Instant::now() >= compact_resize_deadline {
            if let Some(timing) = &timing {
                timing.report();
            }
            let current = report.report();
            let window = main.widget();
            panic!(
                "timed out waiting for compact resize: elapsed={:?} report_elapsed={:?} scenario={:?} step0={:?} step1={:?} step1_elapsed={:?} failure={:?} resize={:?} step1_resize={:?} window_mapped={} window_realized={} window_allocation={}x{} surface_size={:?}",
                compact_resize_started.elapsed(),
                current.elapsed,
                current.state,
                current.steps[0].state,
                current.steps[1].state,
                current.steps[1].elapsed,
                current
                    .failure
                    .as_ref()
                    .map(|failure| (failure.step, failure.code)),
                current.counters.window_resize,
                current.steps[1].evidence.window_resize,
                window.is_mapped(),
                window.is_realized(),
                window.width(),
                window.height(),
                window
                    .surface()
                    .map(|surface| (surface.width(), surface.height())),
            );
        }
    }
    if let Some(timing) = &timing {
        timing.report();
    }
    assert!(readiness_checked, "first readiness gate must be checked");
    let compact_report = report.report();
    let resize = compact_report.steps[1]
        .evidence
        .window_resize
        .expect("compact resize evidence");
    assert!(
        resize.sequence
            > compact_report.steps[0]
                .evidence
                .window_resize
                .map_or(0, |prior| prior.sequence)
    );
    assert_eq!(
        (resize.requested_width, resize.requested_height),
        (800, 600)
    );
    assert_eq!(resize.expected_layout, ShellLayoutMode::Compact);
    assert_eq!(resize.layout, ShellLayoutMode::Compact);
    let tolerance = if cfg!(target_os = "windows") { 2 } else { 0 };
    assert!(resize.realized_width.abs_diff(800) <= tolerance);
    assert!(resize.realized_height.abs_diff(600) <= tolerance);
    assert_eq!(
        (main.widget().width(), main.widget().height()),
        (resize.realized_width, resize.realized_height)
    );
    println!(
        "CHECKPOINT_FIRST_RESIZE requested=800x600 allocation={}x{} sequence={} mode=Compact elapsed_ms={} step_elapsed_ms={}",
        resize.realized_width,
        resize.realized_height,
        resize.sequence,
        compact_resize_started.elapsed().as_millis(),
        compact_report.steps[1].elapsed.as_millis()
    );
    assert_empty_closes_bootstrap(
        &main,
        &commands,
        &mut view,
        &mut generation,
        &report,
        &report_base,
        bootstrap_id,
    );
    wait_for(
        "Empty focus-ring capture",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[2].state == SmokeStepState::Passed,
    );
    let empty_path = checkpoint_path(&report_base, "compact-empty");
    let empty_report = report.report();
    let empty = empty_report.steps[2]
        .evidence
        .visual
        .get("compact-empty")
        .expect("completed Empty evidence");
    assert_eq!(
        (
            empty_report.steps[2].evidence.tabs,
            empty_report.steps[2].evidence.panes,
            empty_report.steps[2].evidence.sessions,
        ),
        (0, 0, 0)
    );
    assert!(!empty.facts.terminal_canvas);
    assert!(empty.facts.focus_or_selection_treatment);
    assert!((2..=4).contains(&empty.png.focus_or_selection_thickness_px));
    assert!(empty_path.is_file());
    assert!(mapped_terminal(main.widget()).is_none());
    println!(
        "CHECKPOINT_FIRST_COMPACT_PASS id=compact-empty elapsed_ms={}",
        compact_resize_started.elapsed().as_millis()
    );
    wait_for(
        "retained local tab",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[3].state == SmokeStepState::Passed,
    );
    assert_eq!(view.workspace.tabs.len(), 1);
    let retained_tab = view.workspace.tabs[0].id;
    let retained_pane = view.workspace.tabs[0].active_pane;
    assert_eq!(view.workspace.active_tab, Some(retained_tab));
    wait_for(
        "standard resize",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[4].state == SmokeStepState::Passed,
    );

    {
        let mut runtime = LifecycleRuntime {
            main: &main,
            commands: &commands,
            view: &mut view,
            generation: &mut generation,
            target_profile: &target_profile,
            report: &report,
            report_base: &report_base,
            focus_triggers: &focus_triggers,
        };
        assert_modal_lifecycle(&mut runtime, 5, "standard-editor", "editor-dialog");
        assert_modal_lifecycle(&mut runtime, 6, "standard-settings", "settings-window");
        assert_modal_lifecycle(&mut runtime, 7, "standard-import", "import-dialog");
    }

    wait_for(
        "disposable SSH target tab",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[8].state == SmokeStepState::Passed,
    );
    assert_eq!(view.workspace.tabs.len(), 2);
    assert_eq!(view.workspace.tabs[0].id, retained_tab);
    assert_eq!(view.workspace.tabs[0].active_pane, retained_pane);
    let target_tab = view.workspace.tabs[1].id;
    let target_pane = view.workspace.tabs[1].active_pane;
    assert_eq!(view.workspace.active_tab, Some(target_tab));
    wait_for(
        "recorded synthetic SSH connect",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[10].state == SmokeStepState::Passed,
    );
    assert_eq!(commands.connects(), vec![(target_pane, target_connection)]);
    assert_ne!(target_pane, retained_pane);

    let connected_path = checkpoint_path(&report_base, "standard-connected");
    wait_for(
        "connected capture",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().png_paths.contains(&connected_path),
    );
    assert!(connected_path.is_file(), "connected PNG must exist");
    assert!(
        mapped_dialog(main.widget()).is_none(),
        "Connected capture must not retain a mapped modal"
    );
    assert!(css_child(main.widget(), "modal-background").is_sensitive());

    let grid_path = checkpoint_path(&report_base, "standard-grid");
    let grid_tree = assert_grid_waits_for_mapped_import(
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        &report,
        &grid_path,
    );
    wait_for(
        "grid capture",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().png_paths.contains(&grid_path),
    );
    assert!(grid_path.is_file(), "grid PNG must exist");
    assert!(mapped_dialog(main.widget()).is_none());
    let panes = descendants(main.widget())
        .into_iter()
        .filter(|widget| widget.is_mapped() && widget.has_css_class("pane-surface"))
        .collect::<Vec<_>>();
    assert_eq!(panes.len(), 4);
    grid_bounds::assert_grid_tree(&grid_tree);
    grid_bounds::assert_allocations(&css_child(main.widget(), "pane-host"), &grid_tree, true);
    assert!(
        panes
            .iter()
            .all(|pane| pane.width() > 0 && pane.height() > 0)
    );

    for (id, state, width, height, mode) in layout_cases() {
        let path = checkpoint_path(&report_base, &id);
        wait_for(
            &id,
            &main,
            &commands,
            &mut view,
            &mut generation,
            &target_profile,
            || report.report().png_paths.contains(&path),
        );
        let tree = &view
            .workspace
            .tabs
            .iter()
            .find(|tab| tab.id == target_tab)
            .unwrap()
            .pane_tree;
        grid_bounds::assert_allocations(
            &css_child(main.widget(), "pane-host"),
            tree,
            state == SmokeVisualState::Grid,
        );
        let facts = &report.report().counters.visual[&id].facts;
        assert_eq!(
            rshell_ui::ShellLayout::for_width(facts.realized_width).mode,
            mode
        );
        assert!(
            css_child(
                main.widget(),
                match mode {
                    ShellLayoutMode::Compact => "shell-compact",
                    ShellLayoutMode::Standard => "shell-standard",
                    ShellLayoutMode::Wide => "shell-wide",
                }
            )
            .is_mapped()
        );
        assert_eq!(
            (facts.requested_width, facts.requested_height),
            (width, height)
        );
        println!(
            "LAYOUT_CASE id={id} requested={width}x{height} realized={}x{} mode={mode:?} font={} scale={} tree_panes={}",
            main.widget().width(),
            main.widget().height(),
            main.widget().pango_context().font_description().unwrap(),
            main.widget().scale_factor(),
            tree.pane_ids().len()
        );
        assert_eq!(view.workspace.tabs[0].id, retained_tab);
        assert_eq!(view.workspace.tabs[0].active_pane, retained_pane);
        let sessions = view.workspace.active_tab().unwrap().pane_tree.session_ids();
        let mut acknowledgements = Vec::new();
        for session in sessions {
            let size = view.latest_frames[&session].size;
            generation += 1;
            let ack = Arc::new(frame_with_text(
                generation,
                size,
                &format!("LAYOUT_ACK_{id}"),
            ));
            view.latest_frames.insert(session, ack.clone());
            acknowledgements.push((session, ack));
        }
        main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
        for (session, ack) in acknowledgements {
            main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
                session,
                event: SessionUiEvent::Frame(ack),
            }));
        }
    }

    wait_for(
        "scenario completion",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().state == SmokeScenarioState::Passed,
    );
    let completed = report.report();
    assert!(completed.failure.is_none());
    assert!(
        completed
            .steps
            .iter()
            .all(|step| step.state == SmokeStepState::Passed)
    );
    assert_eq!(
        completed.requested_png_path.as_deref(),
        Some(checkpoint_path(&report_base, "wide-single").as_path())
    );
    assert_eq!(
        completed.png_path.as_deref(),
        Some(checkpoint_path(&report_base, "wide-single").as_path())
    );
    assert_eq!(completed.requested_png_paths, completed.png_paths);
    assert_eq!(completed.png_paths.len(), 21);
    assert_eq!(commands.close_tabs(), vec![bootstrap_id]);
    assert_eq!(commands.new_tab_count(), 2);
    assert!(commands.shutdown_seen());
    println!("CHECKPOINT_BACKEND synthetic=true transport=false geometry_injected=false");
    println!("CHECKPOINT_REPORT_REQUEST {}", report_base.display());
    for path in &completed.png_paths {
        println!("CHECKPOINT_PNG {}", path.display());
    }
    println!(
        "CHECKPOINT_LIFECYCLE_NATIVE_PASS checkpoints=21 modal_checkpoints=3 tabs=2 connect_target=index1 grid_panes=4 modes=3"
    );
    main.widget().close();
    drain_pending();
}

fn run_compact_editor_r4(evidence_root: &Path) {
    let mut view = connected_fixture();
    let target_profile = ConnectionProfile::new("Synthetic SSH target", "safe.example.test");
    let commands = Arc::new(RecordingPort::default());
    let report_base = evidence_root.join("compact-editor-r4-report.json");
    let mut steps = vec![
        SmokeStep::new(SmokeAction::WaitWindowRealized),
        SmokeStep::new(SmokeAction::ResizeWindow {
            width: 800,
            height: 600,
            expected_mode: ShellLayoutMode::Compact,
        }),
    ];
    steps.extend((0..19).map(|_| SmokeStep::new(SmokeAction::NewTab)));
    steps.extend([
        SmokeStep::new(SmokeAction::SwitchTab(0)),
        SmokeStep::new(SmokeAction::SwitchTab(19)),
        SmokeStep::new(SmokeAction::SwitchTab(0)),
        SmokeStep::new(SmokeAction::OpenConnectionEditor),
        SmokeStep::new(SmokeAction::WaitFrameContains(
            "COMPACT_EDITOR_MANUAL_CLOSE_ACK".into(),
        )),
        SmokeStep::new(SmokeAction::CloseAll),
    ]);
    let scenario = SmokeScenario::with_steps("compact-editor-r4-native", steps);
    let (init, report) = MainWindowInit::new(commands.clone(), view.clone())
        .with_smoke_driver(SmokeDriverInit::new(scenario).with_png_path(&report_base));
    let main = MainWindow::builder().launch(init).detach();
    let focus_triggers = observe_pre_modal_focus(&main);
    main.widget().set_default_size(800, 600);
    main.widget().present();

    let mut generation = 1;
    wait_for(
        "focused compact resize",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[1].state == SmokeStepState::Passed,
    );
    wait_for(
        "focused twenty-tab state",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[21].state == SmokeStepState::Passed,
    );
    wait_for(
        "focused return to tab zero",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[23].state == SmokeStepState::Passed,
    );
    assert_eq!(view.workspace.tabs.len(), 20);
    let grid_tab = view.workspace.tabs[0].id;
    view.workspace.active_tab = Some(grid_tab);
    build_native_grid(&main, &commands, &mut view, &mut generation, grid_tab);

    wait_for(
        "focused compact editor open",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().steps[24].state == SmokeStepState::Passed,
    );
    let surface = css_child(main.widget(), "editor-dialog");
    let background = css_child(main.widget(), "modal-background");
    let trigger = focus_triggers
        .borrow()
        .get("editor-dialog")
        .cloned()
        .expect("compact editor pre-modal trigger");
    print_compact_editor_runtime("opened", main.widget(), &surface, &background, &trigger);
    assert!(press_escape(&surface), "focused compact editor Escape");
    print_compact_editor_runtime(
        "after-escape-signal",
        main.widget(),
        &surface,
        &background,
        &trigger,
    );

    let deadline = Instant::now() + Duration::from_secs(8);
    while surface.is_mapped() || !background.is_sensitive() {
        pump_once(
            &main,
            &commands,
            &mut view,
            &mut generation,
            &target_profile,
        );
        assert!(
            Instant::now() < deadline,
            "focused compact editor did not close"
        );
    }
    drain_pending();
    print_compact_editor_runtime("final", main.widget(), &surface, &background, &trigger);
    let focused = main
        .widget()
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    let exact_focus_restored = focused.as_ref() == Some(&trigger);
    println!(
        "COMPACT_EDITOR_R4_RESULT editor_open_passed=true tabs={} panes={} surface_hidden={} background_sensitive={} exact_focus_restored={exact_focus_restored}",
        view.workspace.tabs.len(),
        view.workspace
            .tabs
            .iter()
            .find(|tab| tab.id == grid_tab)
            .map_or(0, |tab| tab.pane_tree.pane_ids().len()),
        !surface.is_visible(),
        background.is_sensitive(),
    );

    let active_session = view
        .workspace
        .tabs
        .iter()
        .find(|tab| tab.id == grid_tab)
        .and_then(|tab| tab.pane_tree.session_id(tab.active_pane).ok().flatten())
        .expect("focused grid active session");
    let size = view.latest_frames[&active_session].size;
    generation += 1;
    let ack = Arc::new(frame_with_text(
        generation,
        size,
        "COMPACT_EDITOR_MANUAL_CLOSE_ACK",
    ));
    view.latest_frames.insert(active_session, ack.clone());
    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
    main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
        session: active_session,
        event: SessionUiEvent::Frame(ack),
    }));
    wait_for(
        "focused scenario completion",
        &main,
        &commands,
        &mut view,
        &mut generation,
        &target_profile,
        || report.report().state == SmokeScenarioState::Passed,
    );
    main.widget().close();
    drain_pending();
    assert!(
        exact_focus_restored,
        "compact editor checkpoint must close and publish evidence after the real compact 20-tab grid"
    );
}

fn build_native_grid(
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    grid_tab: TabId,
) {
    *commands.import_guard_used.lock().unwrap() = true;
    let first = view
        .workspace
        .tabs
        .iter()
        .find(|tab| tab.id == grid_tab)
        .expect("focused grid tab")
        .pane_tree
        .pane_ids()[0];
    publish_split(
        main,
        view,
        generation,
        first,
        SplitAxis::Horizontal,
        commands,
    );
    let second = view
        .workspace
        .tabs
        .iter()
        .find(|tab| tab.id == grid_tab)
        .expect("focused grid tab after root split")
        .pane_tree
        .pane_ids()[1];
    publish_split(main, view, generation, first, SplitAxis::Vertical, commands);
    publish_split(
        main,
        view,
        generation,
        second,
        SplitAxis::Vertical,
        commands,
    );
    let grid = &view
        .workspace
        .tabs
        .iter()
        .find(|tab| tab.id == grid_tab)
        .expect("focused completed grid tab")
        .pane_tree;
    assert_eq!(grid.pane_ids().len(), 4);
    grid_bounds::assert_grid_tree(grid);
}

fn print_compact_editor_runtime(
    stage: &str,
    main: &gtk::ApplicationWindow,
    surface: &gtk::Widget,
    background: &gtk::Widget,
    trigger: &gtk::Widget,
) {
    let focused = main
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    println!(
        "COMPACT_EDITOR_R4_STAGE stage={stage} surface_visible={} surface_mapped={} background_sensitive={} trigger_type={} trigger_rooted={} trigger_same_root={} trigger_in_main={} trigger_visible={} trigger_mapped={} trigger_sensitive={} trigger_focusable={} focused_type={} focused_same_trigger={} focused_in_main={}",
        surface.is_visible(),
        surface.is_mapped(),
        background.is_sensitive(),
        trigger.type_().name(),
        trigger.root().is_some(),
        trigger.root().as_ref() == main.root().as_ref(),
        is_descendant_of(trigger, main.upcast_ref()),
        trigger.is_visible(),
        trigger.is_mapped(),
        trigger.is_sensitive(),
        trigger.is_focusable(),
        focused
            .as_ref()
            .map_or("none", |widget| widget.type_().name()),
        focused.as_ref() == Some(trigger),
        focused
            .as_ref()
            .is_some_and(|widget| is_descendant_of(widget, main.upcast_ref())),
    );
}

fn is_descendant_of(widget: &gtk::Widget, ancestor: &gtk::Widget) -> bool {
    let mut current = Some(widget.clone());
    while let Some(widget) = current {
        if widget == *ancestor {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn assert_grid_waits_for_mapped_import(
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    target_profile: &ConnectionProfile,
    report: &SmokeReportHandle,
    grid_path: &Path,
) -> PaneTree {
    let deadline = Instant::now() + Duration::from_secs(8);
    while view
        .workspace
        .active_tab()
        .is_none_or(|tab| tab.pane_tree.pane_ids().len() != 4)
        || mapped_dialog(main.widget()).is_none()
    {
        pump_once(main, commands, view, generation, target_profile);
        assert!(
            Instant::now() < deadline,
            "Grid/modal guard setup timed out"
        );
    }
    pump_once(main, commands, view, generation, target_profile);
    let blocked = report.report();
    assert_eq!(blocked.steps[12].state, SmokeStepState::Running);
    assert!(!blocked.png_paths.iter().any(|path| path == grid_path));
    assert_eq!(
        view.workspace
            .active_tab()
            .expect("active grid tab")
            .pane_tree
            .pane_ids()
            .len(),
        4
    );
    let import = mapped_dialog(main.widget()).expect("mapped Import guard surface");
    assert!(!css_child(main.widget(), "modal-background").is_sensitive());
    println!("CHECKPOINT_NONMODAL_GUARD_PASS state=grid panes=4 modal=import capture_blocked=true");
    let tree = view.workspace.active_tab().unwrap().pane_tree.clone();
    grid_bounds::assert_grid_tree(&tree);
    assert!(
        press_escape(&import),
        "guard Import must close through Escape"
    );
    while import.is_mapped()
        || mapped_dialog(main.widget()).is_some()
        || !css_child(main.widget(), "modal-background").is_sensitive()
    {
        pump_once(main, commands, view, generation, target_profile);
        assert!(Instant::now() < deadline, "guard Import did not close");
    }
    assert!(css_child(main.widget(), "modal-background").is_sensitive());
    tree
}

fn assert_empty_closes_bootstrap(
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    report: &SmokeReportHandle,
    report_base: &Path,
    bootstrap_id: TabId,
) {
    let path = checkpoint_path(report_base, "compact-empty");
    let deadline = Instant::now() + Duration::from_secs(8);
    let closed = loop {
        gtk::glib::MainContext::default().iteration(false);
        let mut closed = Vec::new();
        for command in commands.drain() {
            match command {
                RecordedCommand::Resize(session, size) => {
                    publish_frame(main, view, generation, session, size);
                }
                RecordedCommand::CloseTab(tab) => closed.push(tab),
                RecordedCommand::NewLocalTab
                | RecordedCommand::Connect(_, _)
                | RecordedCommand::Split(_, _)
                | RecordedCommand::ClosePane(_) => {
                    panic!("workspace command arrived before Empty completed")
                }
                RecordedCommand::Shutdown => panic!("shutdown before Empty completed"),
            }
        }
        if !closed.is_empty() {
            break closed;
        }
        let current = report.report();
        assert!(
            !current.png_paths.contains(&path) && current.steps[2].state != SmokeStepState::Passed,
            "Empty completed without issuing CloseTab for the populated bootstrap"
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for Empty CloseTab"
        );
        std::thread::yield_now();
    };
    assert_eq!(
        closed,
        vec![bootstrap_id],
        "Empty must close bootstrap exactly once"
    );

    gtk::glib::MainContext::default().iteration(false);
    let deferred = commands.drain();
    assert!(
        deferred
            .iter()
            .all(|command| !matches!(command, RecordedCommand::CloseTab(_))),
        "Empty must not submit duplicate CloseTab commands"
    );
    assert_eq!(view.workspace.tabs.len(), 1);
    assert!(mapped_terminal(main.widget()).is_some());
    let withheld = report.report();
    assert!(!withheld.png_paths.contains(&path));
    assert_eq!(withheld.steps[2].state, SmokeStepState::Running);

    view.revision = view.revision.saturating_add(1);
    view.workspace = WorkspaceState::default();
    view.latest_frames.clear();
    view.display_recovery.clear();
    view.error_panes.clear();
    view.pane_launches.clear();
    view.session_states.clear();
    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
    wait_for_empty_focus_target(main, report, &path);
    println!("CHECKPOINT_EMPTY_CLOSE_PASS bootstrap_closed_once=true update_withheld=true");
}

fn wait_for_empty_focus_target(
    main: &relm4::Controller<MainWindow>,
    report: &SmokeReportHandle,
    path: &Path,
) {
    let button = command_button(main.widget());
    let deadline = Instant::now() + Duration::from_secs(8);
    while mapped_terminal(main.widget()).is_some()
        || !button.is_mapped()
        || button.width() <= 0
        || button.height() <= 0
        || !button.has_focus()
        || main
            .widget()
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
            .as_ref()
            != Some(button.upcast_ref())
    {
        gtk::glib::MainContext::default().iteration(false);
        let current = report.report();
        assert!(!current.png_paths.iter().any(|candidate| candidate == path));
        assert_eq!(current.steps[2].state, SmokeStepState::Running);
        assert!(
            Instant::now() < deadline,
            "Empty surface did not become focusable"
        );
    }
    assert!(button.is_sensitive() && button.is_focusable());
    let focus_visible = button
        .state_flags()
        .contains(gtk::StateFlags::FOCUS_VISIBLE);
    println!(
        "CHECKPOINT_EMPTY_NATIVE_FOCUS focused=true focus_visible={focus_visible} allocation={}x{}",
        button.width(),
        button.height()
    );
    let root: gtk::Widget = main.widget().clone().upcast();
    let facts = collect_visual_facts(&root, (800, 600));
    assert!(
        facts.focus_or_selection_treatment,
        "Empty visual facts ignored the genuinely focused command-bar button"
    );
    assert_eq!(
        selection_treatment_surface(&root).as_ref(),
        button.parent().as_ref(),
        "Empty accent surface ignored the genuinely focused command-bar button"
    );

    let other = descendants(main.widget())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|candidate| {
            candidate.is_mapped()
                && candidate.is_sensitive()
                && candidate.is_focusable()
                && !has_ancestor_class(candidate.upcast_ref(), "command-bar")
        })
        .expect("mapped non-command focus target");
    assert!(other.grab_focus());
    assert!(!collect_visual_facts(&root, (800, 600)).focus_or_selection_treatment);
    assert!(selection_treatment_surface(&root).is_none());

    assert!(button.grab_focus());
    button.set_sensitive(false);
    assert!(!collect_visual_facts(&root, (800, 600)).focus_or_selection_treatment);
    assert!(selection_treatment_surface(&root).is_none());
    button.set_sensitive(true);

    assert!(button.grab_focus());
    button.set_visible(false);
    assert!(!collect_visual_facts(&root, (800, 600)).focus_or_selection_treatment);
    assert!(selection_treatment_surface(&root).is_none());
    button.set_visible(true);
    while !button.is_mapped()
        || !button.has_focus()
        || main
            .widget()
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
            .as_ref()
            != Some(button.upcast_ref())
    {
        if button.is_mapped() && !button.has_focus() {
            let _ = button.grab_focus();
            button.queue_draw();
        }
        gtk::glib::MainContext::default().iteration(false);
        assert!(
            Instant::now() < deadline,
            "restored command button did not regain mapped root focus"
        );
    }
    assert!(button.has_focus() && button.is_mapped() && button.is_sensitive());
    assert_eq!(
        main.widget()
            .root()
            .and_then(|root| gtk::prelude::RootExt::focus(&root))
            .as_ref(),
        Some(button.upcast_ref())
    );
    button.queue_draw();
    root.queue_draw();
}

struct LifecycleRuntime<'a> {
    main: &'a relm4::Controller<MainWindow>,
    commands: &'a RecordingPort,
    view: &'a mut AppViewModel,
    generation: &'a mut u64,
    target_profile: &'a ConnectionProfile,
    report: &'a SmokeReportHandle,
    report_base: &'a Path,
    focus_triggers: &'a RefCell<BTreeMap<&'static str, gtk::Widget>>,
}

fn assert_modal_lifecycle(runtime: &mut LifecycleRuntime<'_>, step: usize, id: &str, class: &str) {
    let path = checkpoint_path(runtime.report_base, id);
    wait_for(
        &format!("{id} capture"),
        runtime.main,
        runtime.commands,
        runtime.view,
        runtime.generation,
        runtime.target_profile,
        || runtime.report.report().png_paths.contains(&path),
    );
    let surface = css_child(runtime.main.widget(), class);
    let background = css_child(runtime.main.widget(), "modal-background");
    assert!(
        path.is_file(),
        "captured PNG must exist: {}",
        path.display()
    );
    assert!(surface.is_mapped(), "{id} must still be mapped at capture");
    assert!(!background.is_sensitive(), "{id} background must be inert");
    let captured = runtime.report.report();
    assert_eq!(captured.steps[step].state, SmokeStepState::Running);
    assert!(
        captured.steps[step]
            .binding
            .as_ref()
            .is_none_or(|binding| !binding.verified && !binding.component_verified),
        "{id} binding must remain unverified while its modal is mapped"
    );
    assert!(
        !captured.counters.visual.contains_key(id),
        "{id} captured evidence must remain private while its modal is mapped"
    );

    let deadline = Instant::now() + Duration::from_secs(8);
    while surface.is_mapped() || !background.is_sensitive() {
        pump_once(
            runtime.main,
            runtime.commands,
            runtime.view,
            runtime.generation,
            runtime.target_profile,
        );
        let current = runtime.report.report();
        assert!(
            !current.counters.visual.contains_key(id),
            "{id} evidence was published before modal closure completed"
        );
        assert_eq!(
            current.steps[step].state,
            SmokeStepState::Running,
            "{id} passed before modal closure completed"
        );
        assert!(
            Instant::now() < deadline,
            "{id} modal did not close before deadline"
        );
    }
    assert!(
        background.is_sensitive(),
        "{id} background sensitivity not restored"
    );
    let focused = runtime
        .main
        .widget()
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root));
    let trigger = runtime
        .focus_triggers
        .borrow()
        .get(class)
        .cloned()
        .unwrap_or_else(|| panic!("{id} pre-modal trigger focus was not captured"));
    assert_eq!(
        focused.as_ref(),
        Some(&trigger),
        "{id} did not restore the exact pre-modal trigger focus"
    );
    wait_for(
        &format!("{id} finalized evidence"),
        runtime.main,
        runtime.commands,
        runtime.view,
        runtime.generation,
        runtime.target_profile,
        || {
            let current = runtime.report.report();
            current.steps[step].state == SmokeStepState::Passed
                && current.counters.visual.get(id).is_some_and(|evidence| {
                    evidence.contract_passes()
                        && evidence.accessibility.focus_restored
                        && evidence.accessibility.escape_cancelled
                })
        },
    );
}

fn visual_step(id: &str, state: SmokeVisualState) -> SmokeStep {
    visual_step_at(id, state, 1_360, 860, ShellLayoutMode::Standard)
}

fn layout_cases() -> Vec<(String, SmokeVisualState, i32, i32, ShellLayoutMode)> {
    [
        ("compact", 800, 600, ShellLayoutMode::Compact),
        ("standard", 1360, 860, ShellLayoutMode::Standard),
        ("wide", 1920, 1080, ShellLayoutMode::Wide),
    ]
    .into_iter()
    .flat_map(|(mode_name, w, h, mode)| {
        [
            ("grid-layout", SmokeVisualState::Grid),
            ("horizontal", SmokeVisualState::HSplit),
            ("vertical", SmokeVisualState::VSplit),
            ("three", SmokeVisualState::TopBottom3),
            ("single", SmokeVisualState::Single),
        ]
        .into_iter()
        .map(move |(name, state)| (format!("{mode_name}-{name}"), state, w, h, mode))
    })
    .collect()
}

fn visual_step_at(
    id: &str,
    state: SmokeVisualState,
    width: i32,
    height: i32,
    mode: ShellLayoutMode,
) -> SmokeStep {
    SmokeStep {
        action: SmokeAction::VisualCheckpoint(SmokeVisualCheckpoint {
            id: id.into(),
            state,
            width,
            height,
            expected_mode: mode,
        }),
        surface: Some("gtk".into()),
        connection: None,
    }
}

fn wait_for(
    label: &str,
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    target_profile: &ConnectionProfile,
    mut condition: impl FnMut() -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !condition() {
        pump_once(main, commands, view, generation, target_profile);
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
    }
}

fn pump_once(
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    target_profile: &ConnectionProfile,
) {
    pump_once_timed(main, commands, view, generation, target_profile, None);
}

#[derive(Clone, Copy)]
enum PumpStage {
    Iteration,
    Drain,
    Command,
    PublishFrame,
    Pump,
}

impl PumpStage {
    const NAMES: [&str; 5] = ["iteration", "drain", "command", "publish_frame", "pump"];
}

#[derive(Clone, Copy, Debug, Default)]
struct CallStats {
    count: u64,
    total: Duration,
    max: Duration,
}

struct CompactPumpTrace {
    started: Instant,
    calls: [CallStats; 5],
    slow_calls: u64,
    logged_slow_calls: usize,
}

impl CompactPumpTrace {
    fn new(started: Instant) -> Self {
        Self {
            started,
            calls: [CallStats::default(); 5],
            slow_calls: 0,
            logged_slow_calls: 0,
        }
    }

    fn record(&mut self, stage: PumpStage, duration: Duration) {
        let calls = &mut self.calls[stage as usize];
        calls.count = calls.count.saturating_add(1);
        calls.total = calls.total.saturating_add(duration);
        calls.max = calls.max.max(duration);
        if duration >= Duration::from_millis(100) {
            self.slow_calls = self.slow_calls.saturating_add(1);
            if self.logged_slow_calls < 16 {
                eprintln!(
                    "CHECKPOINT_COMPACT_SLOW stage={} elapsed_ms={} duration_ms={} inclusive={}",
                    PumpStage::NAMES[stage as usize],
                    self.started.elapsed().as_millis(),
                    duration.as_millis(),
                    matches!(stage, PumpStage::Command | PumpStage::Pump),
                );
                self.logged_slow_calls += 1;
            }
        }
    }

    fn report(&self) {
        eprintln!(
            "CHECKPOINT_COMPACT_TIMING iteration={:?} drain={:?} command={:?} publish_frame={:?} pump={:?} slow_calls={} logged_slow_calls={} threshold_ms=100 command_includes_publish_frame=true pump_includes_all=true",
            self.calls[0],
            self.calls[1],
            self.calls[2],
            self.calls[3],
            self.calls[4],
            self.slow_calls,
            self.logged_slow_calls,
        );
    }
}

fn time_call<T>(
    trace: &mut Option<&mut CompactPumpTrace>,
    stage: PumpStage,
    call: impl FnOnce() -> T,
) -> T {
    let started = trace.is_some().then(Instant::now);
    let result = call();
    if let (Some(trace), Some(started)) = (trace.as_deref_mut(), started) {
        trace.record(stage, started.elapsed());
    }
    result
}

fn pump_once_timed(
    main: &relm4::Controller<MainWindow>,
    commands: &RecordingPort,
    view: &mut AppViewModel,
    generation: &mut u64,
    target_profile: &ConnectionProfile,
    mut trace: Option<&mut CompactPumpTrace>,
) {
    let pump_started = trace.is_some().then(Instant::now);
    time_call(&mut trace, PumpStage::Iteration, || {
        gtk::glib::MainContext::default().iteration(false)
    });
    for command in time_call(&mut trace, PumpStage::Drain, || commands.drain()) {
        let command_started = trace.is_some().then(Instant::now);
        match command {
            RecordedCommand::Resize(session, size) => {
                time_call(&mut trace, PumpStage::PublishFrame, || {
                    publish_frame(main, view, generation, session, size)
                });
            }
            RecordedCommand::NewLocalTab => {
                publish_new_local_tab(main, view, generation, target_profile);
            }
            RecordedCommand::Connect(pane, connection) => {
                publish_connection(main, view, generation, pane, connection, target_profile);
            }
            RecordedCommand::Split(pane, axis) => {
                publish_split(main, view, generation, pane, axis, commands);
            }
            RecordedCommand::ClosePane(pane) => publish_close_pane(main, view, pane),
            RecordedCommand::CloseTab(tab) => panic!("unexpected unhandled CloseTab({tab})"),
            RecordedCommand::Shutdown => {
                view.revision = view.revision.saturating_add(1);
                view.workspace = WorkspaceState::default();
                view.latest_frames.clear();
                view.display_recovery.clear();
                view.error_panes.clear();
                view.pane_launches.clear();
                view.session_states.clear();
                main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
                main.emit(MainWindowMsg::AppEvent(AppEvent::ShutdownComplete));
            }
        }
        if let (Some(trace), Some(started)) = (trace.as_deref_mut(), command_started) {
            trace.record(PumpStage::Command, started.elapsed());
        }
    }
    std::thread::yield_now();
    if let (Some(trace), Some(started)) = (trace, pump_started) {
        trace.record(PumpStage::Pump, started.elapsed());
    }
}

fn publish_new_local_tab(
    main: &relm4::Controller<MainWindow>,
    view: &mut AppViewModel,
    generation: &mut u64,
    target_profile: &ConnectionProfile,
) {
    if view.catalog.connections.is_empty() {
        view.catalog
            .connections
            .insert(target_profile.id, target_profile.clone());
    }
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    *generation += 1;
    view.workspace.tabs.push(TabState {
        id: tab,
        title: "Synthetic local checkpoint".into(),
        pane_tree: PaneTree::with_session(pane, session),
        active_pane: pane,
    });
    view.workspace.active_tab = Some(tab);
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(
        session,
        Arc::new(frame(*generation, synthetic_terminal_size())),
    );
    publish_view(main, view);
}

fn publish_connection(
    main: &relm4::Controller<MainWindow>,
    view: &mut AppViewModel,
    generation: &mut u64,
    pane: PaneId,
    connection: ConnectionId,
    target_profile: &ConnectionProfile,
) {
    assert_eq!(connection, target_profile.id);
    let tab = view
        .workspace
        .tabs
        .iter_mut()
        .find(|tab| tab.pane_tree.contains_pane(pane))
        .expect("Connect target pane");
    if let Some(old) = tab.pane_tree.session_id(pane).expect("Connect pane") {
        view.latest_frames.remove(&old);
        view.session_states.remove(&old);
    }
    let session = SessionId::new();
    tab.pane_tree
        .replace_session(pane, Some(session))
        .expect("replace connected session");
    tab.title = "Synthetic SSH target".into();
    *generation += 1;
    view.pane_launches.insert(
        pane,
        PaneLaunchTarget::Connection {
            id: connection,
            host: target_profile.host.clone(),
        },
    );
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(
        session,
        Arc::new(frame(*generation, synthetic_terminal_size())),
    );
    publish_view(main, view);
}

fn publish_split(
    main: &relm4::Controller<MainWindow>,
    view: &mut AppViewModel,
    generation: &mut u64,
    pane: PaneId,
    axis: SplitAxis,
    commands: &RecordingPort,
) {
    let tab = view
        .workspace
        .tabs
        .iter_mut()
        .find(|tab| tab.pane_tree.contains_pane(pane))
        .expect("Split target pane");
    let new_pane = PaneId::new();
    let session = SessionId::new();
    let mut tree = tab
        .pane_tree
        .clone()
        .split(pane, axis, new_pane, 0.5)
        .expect("synthetic split");
    tree.replace_session(new_pane, Some(session))
        .expect("bind split session");
    tab.pane_tree = tree;
    tab.active_pane = new_pane;
    *generation += 1;
    view.pane_launches.insert(new_pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(
        session,
        Arc::new(frame(*generation, synthetic_terminal_size())),
    );
    if tab.pane_tree.pane_ids().len() == 4
        && !std::mem::replace(&mut *commands.import_guard_used.lock().unwrap(), true)
    {
        main.emit(MainWindowMsg::OpenImport);
    }
    publish_view(main, view);
}

fn publish_view(main: &relm4::Controller<MainWindow>, view: &mut AppViewModel) {
    view.revision = view.revision.saturating_add(1);
    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
}

fn publish_close_pane(main: &relm4::Controller<MainWindow>, view: &mut AppViewModel, pane: PaneId) {
    let tab = view
        .workspace
        .tabs
        .iter_mut()
        .find(|tab| tab.pane_tree.contains_pane(pane))
        .expect("ClosePane target");
    if let Some(session) = tab.pane_tree.session_id(pane).unwrap() {
        view.latest_frames.remove(&session);
        view.session_states.remove(&session);
        view.display_recovery.remove(&session);
        view.error_panes.remove(&session);
    }
    tab.pane_tree = tab
        .pane_tree
        .clone()
        .close(pane)
        .expect("close nonlast pane");
    view.pane_launches.remove(&pane);
    if tab.active_pane == pane {
        tab.active_pane = tab.pane_tree.pane_ids()[0];
    }
    publish_view(main, view);
}

fn synthetic_terminal_size() -> TerminalSize {
    TerminalSize {
        cols: 80,
        rows: 24,
        pixel_width: 720,
        pixel_height: 432,
        dpi: 96,
    }
}

fn publish_frame(
    main: &relm4::Controller<MainWindow>,
    view: &mut AppViewModel,
    generation: &mut u64,
    session: SessionId,
    size: TerminalSize,
) {
    *generation += 1;
    let previous = view
        .latest_frames
        .get(&session)
        .and_then(|frame| frame.rows.first())
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<String>()
        });
    let text = previous
        .as_deref()
        .filter(|text| text.starts_with("LAYOUT_ACK_"))
        .unwrap_or("SYNTHETIC LOCAL CHECKPOINT");
    let frame = Arc::new(frame_with_text(*generation, size, text));
    view.latest_frames.insert(session, frame.clone());
    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
    main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
        session,
        event: SessionUiEvent::Frame(frame),
    }));
}

fn drain_pending() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..1_024 {
        if !context.iteration(false) {
            break;
        }
    }
}

fn observe_pre_modal_focus(
    main: &relm4::Controller<MainWindow>,
) -> Rc<RefCell<BTreeMap<&'static str, gtk::Widget>>> {
    let triggers = Rc::new(RefCell::new(BTreeMap::new()));
    for class in ["editor-dialog", "settings-window", "import-dialog"] {
        let surface = css_child(main.widget(), class);
        let observed = Rc::clone(&triggers);
        surface.connect_visible_notify(move |surface| {
            if surface.is_visible() {
                let focused = surface
                    .root()
                    .and_then(|root| gtk::prelude::RootExt::focus(&root))
                    .unwrap_or_else(|| panic!("{class} opened without a pre-modal root focus"));
                observed.borrow_mut().insert(class, focused);
            }
        });
    }
    triggers
}

fn mapped_dialog(root: &impl IsA<gtk::Widget>) -> Option<gtk::Widget> {
    descendants(root)
        .into_iter()
        .find(|widget| widget.is_mapped() && widget.has_css_class("content-dialog"))
}

fn press_escape(surface: &gtk::Widget) -> bool {
    let controllers = surface.observe_controllers();
    (0..controllers.n_items())
        .filter_map(|index| controllers.item(index))
        .filter_map(|controller| controller.downcast::<gtk::EventControllerKey>().ok())
        .any(|controller| {
            controller.emit_by_name::<bool>(
                "key-pressed",
                &[
                    &gtk::gdk::Key::Escape,
                    &0u32,
                    &gtk::gdk::ModifierType::empty(),
                ],
            )
        })
}

fn mapped_terminal(root: &impl IsA<gtk::Widget>) -> Option<gtk::Widget> {
    descendants(root)
        .into_iter()
        .find(|widget| widget.is_mapped() && widget.has_css_class("terminal-canvas"))
}

fn command_button(root: &impl IsA<gtk::Widget>) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| has_ancestor_class(button.upcast_ref(), "command-bar"))
        .expect("command-bar button")
}

fn has_ancestor_class(widget: &gtk::Widget, class: &str) -> bool {
    let mut current = widget.parent();
    while let Some(widget) = current {
        if widget.has_css_class(class) {
            return true;
        }
        current = widget.parent();
    }
    false
}

fn css_child(root: &impl IsA<gtk::Widget>, class: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|widget| widget.has_css_class(class))
        .unwrap_or_else(|| panic!("missing .{class}"))
}

fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    fn collect(widget: &gtk::Widget, output: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(current) = child {
            output.push(current.clone());
            collect(&current, output);
            child = current.next_sibling();
        }
    }
    let mut output = Vec::new();
    collect(root.as_ref(), &mut output);
    output
}

fn checkpoint_path(base: &Path, id: &str) -> PathBuf {
    let parent = base.parent().expect("report parent");
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("report stem");
    parent.join(format!("{stem}-{id}.png"))
}

struct EvidenceRoot {
    path: PathBuf,
    remove_on_drop: bool,
}

impl EvidenceRoot {
    fn prepare() -> Self {
        if let Some(path) = std::env::var_os("RSHELL_CHECKPOINT_LIFECYCLE_DIR") {
            let path = PathBuf::from(path);
            assert!(path.is_dir(), "explicit evidence root must exist");
            assert!(
                fs::read_dir(&path)
                    .expect("explicit evidence root must be readable")
                    .next()
                    .is_none(),
                "explicit evidence root must be empty to prevent artifact overwrite"
            );
            return Self {
                path,
                remove_on_drop: false,
            };
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        for attempt in 0..32 {
            let path = std::env::temp_dir().join(format!(
                "rshell-checkpoint-lifecycle-{}-{nonce}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    return Self {
                        path,
                        remove_on_drop: true,
                    };
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create owned evidence root: {error}"),
            }
        }
        panic!("failed to allocate a unique owned evidence root");
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn is_temporary(&self) -> bool {
        self.remove_on_drop
    }

    fn cleanup(&mut self) {
        fs::remove_dir_all(&self.path).expect("failed to remove owned evidence root");
        self.remove_on_drop = false;
    }
}

impl Drop for EvidenceRoot {
    fn drop(&mut self) {
        if self.remove_on_drop
            && let Err(error) = fs::remove_dir_all(&self.path)
        {
            eprintln!(
                "failed to remove owned checkpoint evidence root {}: {error}",
                self.path.display()
            );
        }
    }
}

#[derive(Clone, Copy)]
enum RecordedCommand {
    Resize(SessionId, TerminalSize),
    NewLocalTab,
    Connect(PaneId, ConnectionId),
    Split(PaneId, SplitAxis),
    ClosePane(PaneId),
    CloseTab(TabId),
    Shutdown,
}

#[derive(Default)]
struct RecordingPort {
    commands: Mutex<VecDeque<RecordedCommand>>,
    shutdown_seen: Mutex<bool>,
    close_tabs: Mutex<Vec<TabId>>,
    connects: Mutex<Vec<(PaneId, ConnectionId)>>,
    new_tab_count: Mutex<usize>,
    import_guard_used: Mutex<bool>,
}

impl RecordingPort {
    fn drain(&self) -> Vec<RecordedCommand> {
        self.commands.lock().unwrap().drain(..).collect()
    }

    fn shutdown_seen(&self) -> bool {
        *self.shutdown_seen.lock().unwrap()
    }

    fn close_tabs(&self) -> Vec<TabId> {
        self.close_tabs.lock().unwrap().clone()
    }

    fn connects(&self) -> Vec<(PaneId, ConnectionId)> {
        self.connects.lock().unwrap().clone()
    }

    fn new_tab_count(&self) -> usize {
        *self.new_tab_count.lock().unwrap()
    }
}

impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        let recorded = match command {
            UiCommand::Session {
                session,
                command: SessionUiCommand::Resize(size),
            } => RecordedCommand::Resize(session, size),
            UiCommand::NewLocalTab => {
                *self.new_tab_count.lock().unwrap() += 1;
                RecordedCommand::NewLocalTab
            }
            UiCommand::Connect { pane, connection } => {
                self.connects.lock().unwrap().push((pane, connection));
                RecordedCommand::Connect(pane, connection)
            }
            UiCommand::Split { pane, axis } => RecordedCommand::Split(pane, axis),
            UiCommand::ClosePane(pane) => RecordedCommand::ClosePane(pane),
            UiCommand::CloseTab(tab) => {
                self.close_tabs.lock().unwrap().push(tab);
                RecordedCommand::CloseTab(tab)
            }
            UiCommand::Shutdown => {
                *self.shutdown_seen.lock().unwrap() = true;
                RecordedCommand::Shutdown
            }
            other => panic!("unexpected lifecycle fixture command: {other:?}"),
        };
        self.commands.lock().unwrap().push_back(recorded);
        Ok(())
    }
}

fn connected_fixture() -> AppViewModel {
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Synthetic local checkpoint".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(
        session,
        Arc::new(frame(
            1,
            TerminalSize {
                cols: 80,
                rows: 24,
                pixel_width: 720,
                pixel_height: 432,
                dpi: 96,
            },
        )),
    );
    view
}

fn frame(generation: u64, size: TerminalSize) -> RenderFrame {
    frame_with_text(generation, size, "SYNTHETIC LOCAL CHECKPOINT")
}

fn frame_with_text(generation: u64, size: TerminalSize, text: &str) -> RenderFrame {
    RenderFrame {
        generation,
        size,
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
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
        }]),
        cursor: None,
        title: "Synthetic local checkpoint".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    }
}
