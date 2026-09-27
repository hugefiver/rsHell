//! Windows pointer -> production Select -> real terminal engine -> rendered selection.
//! Called by the existing root GTK test (root already depends on rshell-session).
use relm4::{
    Component, ComponentController,
    gtk::{self, prelude::*},
};
use rshell_core::{
    AppBootstrapState, AppViewModel, PaneId, PaneLaunchTarget, PaneTree, SelectionRange, SessionId,
    SessionState, SessionUiCommand, TabId, TabState, TerminalProfile, TerminalSize, UiCommand,
    UiCommandPort, UiPortError, Viewport, WorkspaceState,
};
use rshell_session::DefaultTerminalEngine;
use rshell_ui::{MainWindow, MainWindowInit, MainWindowMsg};
use std::sync::{Arc, Mutex};

#[path = "../../crates/rshell-ui/tests/support/fluent_frames.rs"]
mod frames;
#[allow(dead_code)]
#[path = "../../crates/rshell-ui/tests/support/fluent_pixels.rs"]
mod image;
use frames::wait_for_frame;
#[path = "windows_pointer.rs"]
mod pointer;

struct SelectionState {
    engine: DefaultTerminalEngine,
    size: TerminalSize,
    range: Option<SelectionRange>,
    selects: usize,
    generation: u64,
}
struct Port {
    session: SessionId,
    state: Mutex<SelectionState>,
}
impl UiCommandPort for Port {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        if let UiCommand::Session { session, command } = command {
            assert_eq!(session, self.session);
            let mut state = self.state.lock().unwrap();
            match command {
                SessionUiCommand::Resize(size) => {
                    state.engine.resize(size).unwrap();
                    state.size = size;
                }
                SessionUiCommand::Select(range) => {
                    state.range = Some(range);
                    state.selects += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

pub(super) fn run() {
    rshell_ui::apply_global_css();
    let profile = TerminalProfile::default();
    let resolved = profile.settings.resolve(&Default::default());
    let size = TerminalSize {
        cols: 80,
        rows: 24,
        pixel_width: 800,
        pixel_height: 600,
        dpi: 96,
    };
    let mut engine = DefaultTerminalEngine::new(&resolved, size).unwrap();
    engine
        .input(b"Synthetic pointer selection survives each real breakpoint.")
        .unwrap();
    let session = SessionId::new();
    let pane = PaneId::new();
    let tab = TabId::new_v4();
    let port = Arc::new(Port {
        session,
        state: Mutex::new(SelectionState {
            engine,
            size,
            range: None,
            selects: 0,
            generation: 0,
        }),
    });
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![profile],
    });
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Synthetic pointer selection".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames.insert(session, snapshot(&port));
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(port.clone(), view.clone()))
        .detach();
    main.widget().set_default_size(1360, 860);
    main.widget().present();
    wait_for_frame(main.widget(), "pointer fixture mapped", |w| {
        find(w, "terminal-canvas")
            .is_some_and(|w| w.width() > 0 && !w.has_css_class("terminal-geometry-pending"))
    });
    let root = main.widget().upcast_ref::<gtk::Widget>();
    let canvas = find(root, "terminal-canvas").unwrap();
    let terminal_view = find(root, "terminal-view").unwrap();
    image::capture(main.widget(), "standard", "synthetic-pointer-before");
    pointer::drag(main.widget(), &canvas);
    let seen = port.clone();
    wait_for_frame(&canvas, "pointer-generated Select commands", move |_| {
        seen.state.lock().unwrap().selects >= 2
    });
    let (range, selected_text, count) = {
        let s = port.state.lock().unwrap();
        let range = s.range.unwrap();
        (range, s.engine.selection_text(range), s.selects)
    };
    assert!(!selected_text.is_empty());
    for (index, (mode, width, height)) in [
        ("compact", 800, 600),
        ("standard", 1360, 860),
        ("wide", 1920, 1080),
        ("compact", 800, 600),
    ]
    .into_iter()
    .enumerate()
    {
        main.widget().set_default_size(width, height);
        wait_for_frame(
            main.widget(),
            &format!(
                "selection breakpoint real allocation requested_mode={mode} requested={width}x{height}"
            ),
            move |w| {
                find(w, &format!("shell-{mode}")).is_some()
                    && (w.width() - width).abs() <= 2
                    && (w.height() - height).abs() <= 2
            },
        );
        let frame = snapshot(&port);
        let selected_cells = frame
            .rows
            .iter()
            .flat_map(|r| r.cells.iter())
            .filter(|c| c.selected)
            .count();
        assert!(
            selected_cells > 0,
            "real engine must render the pointer-generated range"
        );
        view.latest_frames.insert(session, frame);
        main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
        wait_for_frame(&canvas, "real engine selection frame paint", |_| true);
        assert_eq!(find(root, "terminal-canvas").unwrap(), canvas);
        assert_eq!(find(root, "terminal-view").unwrap(), terminal_view);
        let s = port.state.lock().unwrap();
        assert_eq!(s.range, Some(range));
        assert_eq!(
            s.selects, count,
            "resize must not fabricate or reset selection"
        );
        assert_eq!(s.engine.selection_text(range), selected_text);
        drop(s);
        image::capture(
            main.widget(),
            mode,
            &format!("synthetic-pointer-selection-{index}"),
        );
        let facts = rshell_ui::collect_visual_facts(root, (width, height));
        let cell_width = f64::from_bits(facts.measured_cell_width_bits);
        let cell_height = f64::from_bits(facts.measured_cell_height_bits);
        let painted = image::pixels(&canvas);
        assert_ne!(
            painted.at(
                (cell_width * f64::from(range.start.column) + 1.0) as i32,
                cell_height as i32 - 1
            ),
            painted.at(1, cell_height as i32 - 1),
            "pointer-generated selection must actually be painted, not only retained in backend"
        );
        println!(
            "POINTER_SELECTION mode={mode} requested={width}x{height} actual={}x{} session={session:?} pane={pane:?} tab={tab} view={} canvas={} range={range:?} select_count={count} selected_cells={selected_cells} engine=true native_windows_pointer=true",
            root.width(),
            root.height(),
            terminal_view.as_ptr() as usize,
            canvas.as_ptr() as usize
        );
    }
    main.widget().close();
    assert!(frames::iterate_until(
        std::time::Instant::now() + std::time::Duration::from_secs(2),
        || !main.widget().is_mapped()
    ));
}

fn snapshot(port: &Port) -> Arc<rshell_core::RenderFrame> {
    let mut s = port.state.lock().unwrap();
    let mut frame = s.engine.snapshot(
        Viewport {
            top_stable_row: 0,
            rows: s.size.rows,
        },
        s.range,
    );
    // Engine snapshots deliberately carry generation 0; the real session actor
    // assigns publication generations. Fulfil that same public frame contract
    // here without writing any selection cells or private UI model state.
    s.generation += 1;
    Arc::make_mut(&mut frame).generation = s.generation;
    frame
}

fn find(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    let mut child = root.first_child();
    while let Some(w) = child {
        if w.has_css_class(class) {
            return Some(w);
        }
        if let Some(found) = find(&w, class) {
            return Some(found);
        }
        child = w.next_sibling();
    }
    None
}
