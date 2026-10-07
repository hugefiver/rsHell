use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{
    Component, ComponentController, ComponentParts, ComponentSender, Controller, SimpleComponent,
};
use rshell_core::{
    AppBootstrapState, AppEvent, AppViewModel, CellAttributes, Color, PaneId, PaneLaunchTarget,
    PaneTree, RenderCell, RenderFrame, RenderRow, SessionId, SessionState, SessionUiCommand,
    SessionUiEvent, TabId, TabState, TerminalProfile, TerminalSize, UiCommand, UiCommandPort,
    UiPortError, WorkspaceState,
};
use rshell_ui::{
    MainWindow, MainWindowInit, MainWindowMsg, PaneHost, PaneHostInit, PaneHostMsg, PaneHostOutput,
    SmokeAction, SmokeDriverInit, SmokeFrameEvidence, SmokeScenario, SmokeScenarioState, SmokeStep,
    SmokeStepState, StartupProbe, apply_global_css,
};

const DEADLINE: Duration = Duration::from_secs(8);
const MARKER: &str = "P0-GEOMETRY-READY";

fn main() {
    if cfg!(target_os = "macos") {
        println!(
            "DYNAMIC_GEOMETRY_NATIVE_SKIP platform=macos reason=native_gui_integration_explicitly_skipped"
        );
        return;
    }
    gtk::init().expect("dynamic geometry proof requires an available GTK display; do not skip");
    apply_global_css();
    let settings = gtk::Settings::default().expect("GTK settings");
    settings.set_property("gtk-enable-animations", false);
    settings.set_property("gtk-cursor-blink", false);

    run_populated_before_present();
    run_present_empty_then_populated();
    run_main_window_binding();

    println!("DYNAMIC_GEOMETRY_NATIVE_PASS cases=3 backend=synthetic transport=false");
}

#[derive(Clone, Copy)]
enum PaneEvent {
    Resize(SessionId, TerminalSize),
    GeometryReady(SessionId),
    RenderedSession(Option<SessionId>),
}

#[derive(Debug)]
enum HarnessMsg {
    Pane(PaneHostOutput),
    SetViewModel(Box<AppViewModel>),
}

struct HarnessInit {
    view: AppViewModel,
    probe: StartupProbe,
    events: Rc<RefCell<Vec<PaneEvent>>>,
}

struct Harness {
    host: Controller<PaneHost>,
    events: Rc<RefCell<Vec<PaneEvent>>>,
}

struct HarnessWidgets;

impl SimpleComponent for Harness {
    type Init = HarnessInit;
    type Input = HarnessMsg;
    type Output = ();
    type Root = gtk::Box;
    type Widgets = HarnessWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let host = PaneHost::builder()
            .launch(PaneHostInit {
                view_model: init.view,
                startup_probe: Some(init.probe),
            })
            .forward(sender.input_sender(), HarnessMsg::Pane);
        root.append(host.widget());
        ComponentParts {
            model: Self {
                host,
                events: init.events,
            },
            widgets: HarnessWidgets,
        }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            HarnessMsg::Pane(PaneHostOutput::Command(command)) => {
                if let UiCommand::Session {
                    session,
                    command: SessionUiCommand::Resize(size),
                } = *command
                {
                    self.events
                        .borrow_mut()
                        .push(PaneEvent::Resize(session, size));
                }
            }
            HarnessMsg::Pane(PaneHostOutput::GeometryReady(session)) => self
                .events
                .borrow_mut()
                .push(PaneEvent::GeometryReady(session)),
            HarnessMsg::Pane(PaneHostOutput::RenderedSession(session)) => self
                .events
                .borrow_mut()
                .push(PaneEvent::RenderedSession(session)),
            HarnessMsg::Pane(PaneHostOutput::Error(message)) => {
                panic!("PaneHost rejected dynamic geometry probe: {message}")
            }
            HarnessMsg::Pane(_) => {}
            HarnessMsg::SetViewModel(view) => {
                self.host.emit(PaneHostMsg::SetViewModel(view));
            }
        }
    }
}

fn run_populated_before_present() {
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let events = Rc::new(RefCell::new(Vec::new()));
    let probe = StartupProbe::new();
    let harness = Harness::builder()
        .launch(HarnessInit {
            view: populated_view(pane, session, tab),
            probe: probe.clone(),
            events: Rc::clone(&events),
        })
        .detach();
    let window = present_harness(&harness);

    wait_for("startup PaneHost geometry", || {
        let current = events.borrow();
        host_has_positive_canvas(&harness)
            && settled_for(&current, session)
            && !has_pending_geometry(harness.widget())
    });

    let observed = events.borrow().clone();
    let size = first_resize(&observed, session).expect("startup positive Resize");
    assert_positive(size);
    assert_eq!(count_resizes(&observed, session), 1);
    assert!(probe.report(false).measured_terminal_geometry_ready);
    assert_resize_precedes_ready(&observed, session);
    println!(
        "DYNAMIC_GEOMETRY_CASE case=startup mapped=true host={}x{} resize_px={}x{} cells={}x{} dpi={} resize_count={} geometry_ready_count={} rendered_count={} pending=false order={}",
        harness.widget().width(),
        harness.widget().height(),
        size.pixel_width,
        size.pixel_height,
        size.cols,
        size.rows,
        size.dpi,
        count_resizes(&observed, session),
        count_ready(&observed, session),
        count_rendered(&observed, session),
        event_order(&observed),
    );

    close_window(&window);
    drop(window);
    drop(harness);
    drain_pending();
}

fn run_present_empty_then_populated() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let probe = StartupProbe::new();
    let harness = Harness::builder()
        .launch(HarnessInit {
            view: empty_view(),
            probe: probe.clone(),
            events: Rc::clone(&events),
        })
        .detach();
    let window = present_harness(&harness);
    wait_for("positive empty PaneHost allocation", || {
        harness.widget().is_mapped()
            && harness.widget().width() > 0
            && harness.widget().height() > 0
    });
    let empty_width = harness.widget().width();
    let empty_height = harness.widget().height();

    let pane = PaneId::new();
    let first = SessionId::new();
    let tab = TabId::new_v4();
    let mut view = populated_view(pane, first, tab);
    harness.emit(HarnessMsg::SetViewModel(Box::new(view.clone())));
    wait_for("dynamic PaneHost geometry", || {
        let current = events.borrow();
        host_has_positive_canvas(&harness)
            && settled_for(&current, first)
            && !has_pending_geometry(harness.widget())
    });
    drain_pending();
    let first_events = events.borrow().clone();
    let first_size = first_resize(&first_events, first).expect("dynamic positive Resize");
    assert_positive(first_size);
    assert_resize_precedes_ready(&first_events, first);
    let replacement_start = first_events.len();

    let replacement = SessionId::new();
    view.workspace.tabs[0]
        .pane_tree
        .replace_session(pane, Some(replacement))
        .expect("replace bound session");
    view.latest_frames.remove(&first);
    view.session_states.remove(&first);
    view.latest_frames.insert(replacement, zero_frame());
    view.session_states
        .insert(replacement, SessionState::Connected);
    harness.emit(HarnessMsg::SetViewModel(Box::new(view)));
    wait_for("replacement PaneHost geometry", || {
        let current = events.borrow();
        let replacement_events = &current[replacement_start..];
        settled_for(replacement_events, replacement) && !has_pending_geometry(harness.widget())
    });

    let all_events = events.borrow().clone();
    let replacement_events = &all_events[replacement_start..];
    let replacement_size =
        first_resize(replacement_events, replacement).expect("replacement positive Resize");
    assert_positive(replacement_size);
    assert_resize_precedes_ready(replacement_events, replacement);
    assert_eq!(count_resizes(replacement_events, replacement), 1);
    assert_eq!(count_ready(replacement_events, first), 0);
    assert_eq!(count_rendered(replacement_events, first), 0);
    assert!(probe.report(false).measured_terminal_geometry_ready);
    println!(
        "DYNAMIC_GEOMETRY_CASE case=dynamic empty_host={}x{} first_resize_px={}x{} first_cells={}x{} first_dpi={} replacement_resize_px={}x{} replacement_cells={}x{} replacement_dpi={} first_resize_count={} replacement_resize_count={} replacement_ready_count={} replacement_rendered_count={} stale_identity_events=0 pending=false order_first={} order_replacement={}",
        empty_width,
        empty_height,
        first_size.pixel_width,
        first_size.pixel_height,
        first_size.cols,
        first_size.rows,
        first_size.dpi,
        replacement_size.pixel_width,
        replacement_size.pixel_height,
        replacement_size.cols,
        replacement_size.rows,
        replacement_size.dpi,
        count_resizes(&first_events, first),
        count_resizes(replacement_events, replacement),
        count_ready(replacement_events, replacement),
        count_rendered(replacement_events, replacement),
        event_order(&first_events),
        event_order(replacement_events),
    );

    close_window(&window);
    drop(window);
    drop(harness);
    drain_pending();
}

fn run_main_window_binding() {
    late_resize_keeps_checkpoint_bound_to_its_observed_frame();
    let commands = Arc::new(RecordingPort::default());
    let scenario = SmokeScenario::with_steps(
        "dynamic-geometry-native",
        vec![
            bound_step(SmokeAction::WaitWindowRealized, "gtk", None),
            bound_step(SmokeAction::NewTab, "local_terminal", Some("local")),
            bound_step(
                SmokeAction::WaitFrameContains(MARKER.into()),
                "local_terminal",
                Some("local"),
            ),
            bound_step(SmokeAction::CloseAll, "cleanup", None),
        ],
    );
    let probe = StartupProbe::new();
    let (init, report) = MainWindowInit::new(commands.clone(), empty_view())
        .with_startup_probe(probe.clone())
        .with_smoke_driver(SmokeDriverInit::new(scenario));
    let main = MainWindow::builder().launch(init).detach();
    main.widget().set_default_size(640, 360);
    main.widget().present();

    let mut view = empty_view();
    let mut actual_session = None;
    let mut measured = None;
    let mut measured_sizes = Vec::new();
    let mut published_frames = Vec::new();
    let mut generation = 1;
    let mut new_tab_seen = false;
    let mut marker_frame_published = false;
    let mut shutdown_seen = false;
    let deadline = Instant::now() + DEADLINE;
    while !report.is_complete() {
        gtk::glib::MainContext::default().iteration(false);
        for command in commands.drain() {
            match command {
                RecordedCommand::NewLocalTab => {
                    assert!(!new_tab_seen, "NewLocalTab must be issued exactly once");
                    assert!(view.workspace.tabs.is_empty());
                    let pane = PaneId::new();
                    let session = SessionId::new();
                    let tab = TabId::new_v4();
                    view = populated_view(pane, session, tab);
                    let initial = view.latest_frames.get(&session).expect("initial frame");
                    assert_eq!(initial.generation, 1);
                    assert_eq!((initial.size.cols, initial.size.rows), (120, 36));
                    assert_eq!(
                        (initial.size.pixel_width, initial.size.pixel_height),
                        (0, 0)
                    );
                    assert!(!frame_contains(initial, MARKER));
                    actual_session = Some(session);
                    new_tab_seen = true;
                    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
                }
                RecordedCommand::Resize(session, size) => {
                    assert!(new_tab_seen, "Resize cannot precede NewLocalTab");
                    assert_eq!(actual_session, Some(session));
                    assert_positive(size);
                    measured = Some(size);
                    measured_sizes.push(size);
                    generation += 1;
                    let advanced = Arc::new(marker_frame(generation, size));
                    published_frames.push((session, Arc::clone(&advanced)));
                    view.latest_frames.insert(session, advanced.clone());
                    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
                    main.emit(MainWindowMsg::AppEvent(AppEvent::Session {
                        session,
                        event: SessionUiEvent::Frame(advanced),
                    }));
                    marker_frame_published = true;
                }
                RecordedCommand::Shutdown => {
                    assert!(
                        marker_frame_published,
                        "Shutdown cannot precede marker frame"
                    );
                    assert!(!view.workspace.tabs.is_empty());
                    view.workspace = WorkspaceState::default();
                    view.latest_frames.clear();
                    view.display_recovery.clear();
                    view.error_panes.clear();
                    view.pane_launches.clear();
                    view.session_states.clear();
                    main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
                    gtk::glib::MainContext::default().iteration(false);
                    main.emit(MainWindowMsg::AppEvent(AppEvent::ShutdownComplete));
                    shutdown_seen = true;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "MainWindow geometry binding timed out"
        );
        std::thread::yield_now();
    }

    let completed = report.report();
    assert_eq!(completed.state, SmokeScenarioState::Passed);
    assert!(
        completed
            .steps
            .iter()
            .all(|step| step.state == SmokeStepState::Passed)
    );
    let new_tab_binding = completed.steps[1]
        .binding
        .as_ref()
        .expect("NewTab binding evidence");
    let frame_binding = completed.steps[2]
        .binding
        .as_ref()
        .expect("WaitFrameContains binding evidence");
    assert!(new_tab_binding.verified && new_tab_binding.component_verified);
    assert!(frame_binding.verified && frame_binding.component_verified);
    assert_eq!(new_tab_binding.session_id, actual_session);
    assert_eq!(frame_binding.session_id, actual_session);
    let frame = completed.steps[2]
        .evidence
        .latest_frame
        .expect("advanced frame evidence");
    let size = measured.expect("measured MainWindow Resize");
    let session = actual_session.expect("actual MainWindow session");
    assert_resize_frame_history(session, &measured_sizes, &published_frames);
    let checkpoint_frame = assert_frame_evidence_in_history(frame, session, &published_frames);
    assert!(frame_contains(checkpoint_frame, MARKER));
    assert!(frame.generation >= 2);
    let (final_session, final_frame) = published_frames.last().expect("final published frame");
    assert_eq!(*final_session, session);
    assert_eq!(final_frame.generation, generation);
    assert_eq!(final_frame.size, size);
    assert!(frame_contains(final_frame, MARKER));
    assert!(new_tab_seen && marker_frame_published && shutdown_seen);
    assert!(probe.report(false).measured_terminal_geometry_ready);
    let history = commands.history();
    assert_command_order(&history);
    assert_eq!(
        history
            .iter()
            .filter(|event| matches!(event, CommandKind::Resize))
            .count(),
        measured_sizes.len(),
        "every observed Resize command must receive a synthetic frame response"
    );
    println!(
        "DYNAMIC_GEOMETRY_CASE case=main_window initial_generation=1 initial_cells=120x36 initial_pixels=0x0 checkpoint_generation={} final_generation={} resize_px={}x{} cells={}x{} dpi={} resize_count={} resize_sequence={} newtab_binding=true frame_binding=true same_session=true shutdown_after_empty=true order={}",
        frame.generation,
        final_frame.generation,
        size.pixel_width,
        size.pixel_height,
        size.cols,
        size.rows,
        size.dpi,
        history
            .iter()
            .filter(|event| matches!(event, CommandKind::Resize))
            .count(),
        size_sequence(&measured_sizes),
        command_order(&history),
    );

    main.widget().close();
    wait_for("MainWindow close", || !main.widget().is_visible());
    drop(main);
    drain_pending();
}

fn late_resize_keeps_checkpoint_bound_to_its_observed_frame() {
    let session = SessionId::new();
    let first = TerminalSize {
        cols: 34,
        rows: 9,
        pixel_width: 376,
        pixel_height: 196,
        dpi: 144,
    };
    let second = TerminalSize {
        cols: 53,
        rows: 9,
        pixel_width: 590,
        pixel_height: 196,
        dpi: 144,
    };
    let history = [
        (session, Arc::new(marker_frame(2, first))),
        (session, Arc::new(marker_frame(3, second))),
    ];
    let checkpoint = frame_evidence(&history[0].1);
    let latest = &history[1].1;

    assert_ne!(checkpoint.generation, latest.generation);
    let observed = assert_frame_evidence_in_history(checkpoint, session, &history);
    assert_eq!(observed.generation, 2);
    assert_eq!(observed.size, first);

    let fabricated_generation = SmokeFrameEvidence {
        generation: 4,
        ..checkpoint
    };
    assert!(!frame_evidence_matches_history(
        fabricated_generation,
        session,
        &history
    ));
    let mismatched_geometry = SmokeFrameEvidence {
        pixel_width: checkpoint.pixel_width + 1,
        ..checkpoint
    };
    assert!(!frame_evidence_matches_history(
        mismatched_geometry,
        session,
        &history
    ));
    assert!(!frame_evidence_matches_history(
        checkpoint,
        SessionId::new(),
        &history
    ));
    assert_eq!(latest.generation, 3);
    assert_eq!(latest.size, second);
}

fn assert_resize_frame_history(
    expected_session: SessionId,
    sizes: &[TerminalSize],
    history: &[(SessionId, Arc<RenderFrame>)],
) {
    assert_eq!(
        history.len(),
        sizes.len(),
        "every real Resize must receive exactly one synthetic frame response"
    );
    for (index, ((session, frame), size)) in history.iter().zip(sizes).enumerate() {
        assert_eq!(*session, expected_session);
        assert_eq!(frame.generation, index as u64 + 2);
        assert_eq!(frame.size, *size);
        assert!(frame_contains(frame, MARKER));
    }
}

fn assert_frame_evidence_in_history(
    evidence: SmokeFrameEvidence,
    expected_session: SessionId,
    history: &[(SessionId, Arc<RenderFrame>)],
) -> &RenderFrame {
    let mut matching_generation = history.iter().filter(|(session, frame)| {
        *session == expected_session && frame.generation == evidence.generation
    });
    let (_, frame) = matching_generation
        .next()
        .expect("checkpoint generation must come from a real observed Resize response");
    assert!(
        matching_generation.next().is_none(),
        "checkpoint generation must identify exactly one observed frame"
    );
    assert_eq!(
        frame_evidence(frame),
        evidence,
        "checkpoint geometry must exactly match its observed frame"
    );
    frame
}

fn frame_evidence_matches_history(
    evidence: SmokeFrameEvidence,
    expected_session: SessionId,
    history: &[(SessionId, Arc<RenderFrame>)],
) -> bool {
    let mut matching_generation = history.iter().filter(|(session, frame)| {
        *session == expected_session && frame.generation == evidence.generation
    });
    let Some((_, frame)) = matching_generation.next() else {
        return false;
    };
    matching_generation.next().is_none() && frame_evidence(frame) == evidence
}

fn frame_evidence(frame: &RenderFrame) -> SmokeFrameEvidence {
    SmokeFrameEvidence {
        generation: frame.generation,
        cols: frame.size.cols,
        rows: frame.size.rows,
        pixel_width: frame.size.pixel_width,
        pixel_height: frame.size.pixel_height,
        dpi: frame.size.dpi,
    }
}

fn bound_step(action: SmokeAction, surface: &str, connection: Option<&str>) -> SmokeStep {
    SmokeStep {
        action,
        surface: Some(surface.into()),
        connection: connection.map(str::to_owned),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommandKind {
    NewLocalTab,
    Resize,
    Shutdown,
}

#[derive(Clone, Copy)]
enum RecordedCommand {
    NewLocalTab,
    Resize(SessionId, TerminalSize),
    Shutdown,
}

#[derive(Default)]
struct RecordingPort {
    commands: Mutex<VecDeque<RecordedCommand>>,
    history: Mutex<Vec<CommandKind>>,
}

impl RecordingPort {
    fn drain(&self) -> Vec<RecordedCommand> {
        self.commands.lock().unwrap().drain(..).collect()
    }

    fn history(&self) -> Vec<CommandKind> {
        self.history.lock().unwrap().clone()
    }
}

impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        let (recorded, kind) = match command {
            UiCommand::NewLocalTab => (RecordedCommand::NewLocalTab, CommandKind::NewLocalTab),
            UiCommand::Session {
                session,
                command: SessionUiCommand::Resize(size),
            } => (RecordedCommand::Resize(session, size), CommandKind::Resize),
            UiCommand::Shutdown => (RecordedCommand::Shutdown, CommandKind::Shutdown),
            _ => panic!("unexpected command in dynamic geometry fixture"),
        };
        self.history.lock().unwrap().push(kind);
        self.commands.lock().unwrap().push_back(recorded);
        Ok(())
    }
}

fn present_harness(harness: &Controller<Harness>) -> gtk::Window {
    let window = gtk::Window::new();
    window.set_default_size(640, 360);
    window.set_child(Some(harness.widget()));
    window.present();
    window
}

fn close_window(window: &gtk::Window) {
    window.close();
    wait_for("native window close", || !window.is_visible());
}

fn wait_for(label: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + DEADLINE;
    while !condition() {
        gtk::glib::MainContext::default().iteration(false);
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        std::thread::yield_now();
    }
}

fn drain_pending() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..1_024 {
        if !context.iteration(false) {
            break;
        }
    }
}

fn host_has_positive_canvas(harness: &Controller<Harness>) -> bool {
    harness.widget().is_mapped()
        && harness.widget().width() > 0
        && harness.widget().height() > 0
        && descendants(harness.widget()).into_iter().any(|widget| {
            widget.has_css_class("terminal-canvas")
                && widget.is_mapped()
                && widget.width() > 0
                && widget.height() > 0
        })
}

fn has_pending_geometry(root: &impl IsA<gtk::Widget>) -> bool {
    root.as_ref().has_css_class("pane-geometry-pending")
        || root.as_ref().has_css_class("terminal-geometry-pending")
        || descendants(root).into_iter().any(|widget| {
            widget.has_css_class("pane-geometry-pending")
                || widget.has_css_class("terminal-geometry-pending")
        })
}

fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    fn push_children(widget: &gtk::Widget, output: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(current) = child {
            output.push(current.clone());
            push_children(&current, output);
            child = current.next_sibling();
        }
    }

    let mut output = Vec::new();
    push_children(root.as_ref(), &mut output);
    output
}

fn settled_for(events: &[PaneEvent], session: SessionId) -> bool {
    first_resize(events, session).is_some()
        && count_ready(events, session) > 0
        && count_rendered(events, session) > 0
}

fn first_resize(events: &[PaneEvent], expected: SessionId) -> Option<TerminalSize> {
    events.iter().find_map(|event| match event {
        PaneEvent::Resize(session, size) if *session == expected => Some(*size),
        _ => None,
    })
}

fn count_resizes(events: &[PaneEvent], expected: SessionId) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, PaneEvent::Resize(session, _) if *session == expected))
        .count()
}

fn count_ready(events: &[PaneEvent], expected: SessionId) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, PaneEvent::GeometryReady(session) if *session == expected))
        .count()
}

fn count_rendered(events: &[PaneEvent], expected: SessionId) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(event, PaneEvent::RenderedSession(Some(session)) if *session == expected)
        })
        .count()
}

fn assert_resize_precedes_ready(events: &[PaneEvent], expected: SessionId) {
    let resize = events
        .iter()
        .position(|event| matches!(event, PaneEvent::Resize(session, _) if *session == expected))
        .expect("matching Resize output");
    let ready = events
        .iter()
        .position(
            |event| matches!(event, PaneEvent::GeometryReady(session) if *session == expected),
        )
        .expect("matching GeometryReady output");
    assert!(
        resize < ready,
        "GeometryReady must follow its forwarded Resize"
    );
}

fn event_order(events: &[PaneEvent]) -> String {
    events
        .iter()
        .map(|event| match event {
            PaneEvent::Resize(_, _) => "resize",
            PaneEvent::GeometryReady(_) => "geometry_ready",
            PaneEvent::RenderedSession(Some(_)) => "rendered_some",
            PaneEvent::RenderedSession(None) => "rendered_none",
        })
        .collect::<Vec<_>>()
        .join(">")
}

fn assert_command_order(history: &[CommandKind]) {
    let new_tab = history
        .iter()
        .position(|event| *event == CommandKind::NewLocalTab)
        .expect("NewLocalTab command");
    let resize = history
        .iter()
        .position(|event| *event == CommandKind::Resize)
        .expect("Resize command");
    let shutdown = history
        .iter()
        .position(|event| *event == CommandKind::Shutdown)
        .expect("Shutdown command");
    assert!(new_tab < resize && resize < shutdown);
    assert_eq!(
        history
            .iter()
            .filter(|event| **event == CommandKind::NewLocalTab)
            .count(),
        1
    );
    assert_eq!(
        history
            .iter()
            .filter(|event| **event == CommandKind::Shutdown)
            .count(),
        1
    );
}

fn command_order(history: &[CommandKind]) -> String {
    history
        .iter()
        .map(|event| match event {
            CommandKind::NewLocalTab => "new_tab",
            CommandKind::Resize => "resize",
            CommandKind::Shutdown => "shutdown",
        })
        .collect::<Vec<_>>()
        .join(">")
}

fn size_sequence(sizes: &[TerminalSize]) -> String {
    sizes
        .iter()
        .map(|size| {
            format!(
                "{}x{}@{}x{}@{}",
                size.pixel_width, size.pixel_height, size.cols, size.rows, size.dpi
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn assert_positive(size: TerminalSize) {
    assert!(
        size.cols > 0
            && size.rows > 0
            && size.pixel_width > 0
            && size.pixel_height > 0
            && size.dpi > 0,
        "forwarded Resize must contain positive dimensions, cells, and dpi"
    );
}

fn empty_view() -> AppViewModel {
    AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    })
}

fn populated_view(pane: PaneId, session: SessionId, tab: TabId) -> AppViewModel {
    let mut view = empty_view();
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Synthetic geometry fixture".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(session, zero_frame());
    view
}

fn zero_frame() -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation: 1,
        size: TerminalSize {
            cols: 120,
            rows: 36,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([]),
        cursor: None,
        title: "Synthetic geometry fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

fn marker_frame(generation: u64, size: TerminalSize) -> RenderFrame {
    RenderFrame {
        generation,
        size,
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from(
                MARKER
                    .chars()
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
        title: "Synthetic geometry fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    }
}

fn frame_contains(frame: &RenderFrame, needle: &str) -> bool {
    frame.rows.iter().any(|row| {
        row.cells
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
            .contains(needle)
    })
}
