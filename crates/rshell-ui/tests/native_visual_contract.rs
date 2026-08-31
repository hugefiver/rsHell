#![cfg(not(target_os = "macos"))]

use std::{sync::Arc, time::Duration};

use gtk::gdk::prelude::TextureExtManual;
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppSettings, AppViewModel, CellAttributes, Color, ConnectionCatalog,
    ConnectionProfile, PaneId, PaneLaunchTarget, PaneTree, RenderCell, RenderFrame, RenderRow,
    SessionId, SessionState, TabId, TabState, TerminalProfile, TerminalSize, UiCommand,
    UiCommandPort, UiPortError, WorkspaceState,
};
use rshell_ui::{
    MainWindow, MainWindowInit, MainWindowMsg, NativeByteOrder, analyze_rgba_with_accent,
    apply_global_css, argb32_native_to_rgba, collect_visual_facts, selection_treatment_surface,
};

fn assert_realized_main_window_satisfies_the_fluent_visual_contract() {
    apply_global_css();
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(
            Arc::new(AcceptingPort),
            visual_fixture(),
        ))
        .detach();
    main.widget().set_default_size(1_360, 860);
    main.widget().present();
    main.emit(MainWindowMsg::OpenSettings);
    assert!(flush_gtk());

    let facts = collect_visual_facts(main.widget().upcast_ref(), (1_360, 860));
    let sidebar =
        find_by_css_class(main.widget().upcast_ref(), "sidebar").expect("realized Fluent sidebar");
    let allocation = sidebar.allocation();
    let (minimum, natural, _, _) = sidebar.measure(gtk::Orientation::Horizontal, -1);
    assert!(
        allocation.x() >= 0 && allocation.width() <= 280 && minimum <= 260,
        "sidebar must fit its 260px pane without left clipping: allocation={allocation:?}, minimum={minimum}, natural={natural}"
    );
    assert_eq!(
        (facts.requested_width, facts.requested_height),
        (1_360, 860)
    );
    assert!(facts.realized_width > 0 && facts.realized_height > 0);
    assert!(facts.command_bar);
    assert!(facts.dense_sidebar, "{facts:?}");
    assert!(facts.tab_strip);
    assert!(facts.pane_command_row);
    assert!(facts.terminal_canvas);
    assert!(facts.content_dialog);
    assert!(facts.embedded_icon_count >= 6);
    assert_eq!(facts.icon_logical_size, 16);
    assert!(facts.icon_texture_width >= facts.icon_logical_size);
    assert!(facts.icon_texture_height >= facts.icon_logical_size);
    assert!(facts.icon_backend.is_some());
    assert!(f64::from_bits(facts.effective_scale_bits) > 0.0);
    assert!(f64::from_bits(facts.effective_dpi_bits) > 0.0);
    assert!(f64::from_bits(facts.measured_cell_width_bits) > 0.0);
    assert!(f64::from_bits(facts.measured_cell_height_bits) > 0.0);
    assert!(facts.focus_or_selection_treatment);
    assert!(facts.contract_passes(), "{facts:?}");
    let pixels = wait_for_realized_pixels(main.widget());
    assert_eq!(
        (pixels.width, pixels.height),
        (facts.realized_width, facts.realized_height)
    );
    assert_eq!(pixels.dark_regions_passed, 4);
    assert!((2..=4).contains(&pixels.focus_or_selection_thickness_px));

    main.widget().close();
    assert!(flush_gtk());
}

#[test]
fn breakpoint_crossing_uses_typed_detach_without_gtk_warning() {
    if let Err(error) = gtk::init() {
        eprintln!("native adaptive shell regression skipped: {error}");
        return;
    }
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(
            Arc::new(AcceptingPort),
            visual_fixture(),
        ))
        .detach();
    main.widget().set_default_size(800, 600);
    main.widget().present();
    assert!(flush_gtk());
    let sidebar = find_by_css_class(main.widget().upcast_ref(), "sidebar").unwrap();
    let terminal = find_by_css_class(main.widget().upcast_ref(), "terminal-canvas").unwrap();
    let sidebar_identity = sidebar.as_ptr();
    let terminal_identity = terminal.as_ptr();

    for (width, class, overlay) in [
        (800, "shell-compact", true),
        (900, "shell-standard", false),
        (1_440, "shell-wide", false),
        (800, "shell-compact", true),
    ] {
        main.emit(MainWindowMsg::Allocated { width });
        assert!(flush_gtk(), "allocation {width} must quiesce");
        let background = find_by_css_class(main.widget().upcast_ref(), class)
            .unwrap_or_else(|| panic!("missing .{class} at width {width}"));
        assert!(background.is_visible());
        let parent = sidebar.parent().expect("sidebar owner");
        if overlay {
            assert!(parent.is::<gtk::Overlay>(), "compact owner at {width}");
        } else {
            let paned = parent
                .downcast::<gtk::Paned>()
                .expect("standard/wide paned owner");
            assert_eq!(paned.start_child().as_ref(), Some(&sidebar));
        }
        assert_eq!(sidebar.as_ptr(), sidebar_identity);
        assert_eq!(terminal.as_ptr(), terminal_identity);
    }

    main.emit(MainWindowMsg::Allocated { width: 900 });
    assert!(flush_gtk());
    let paned = sidebar
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Paned>().ok())
        .expect("standard sidebar owner");
    paned.set_position(268);
    main.emit(MainWindowMsg::Allocated { width: 1_440 });
    assert!(flush_gtk());
    assert_eq!(paned.position(), 268, "Wide preserves a user resize");
    main.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());
    assert_eq!(
        sidebar.width_request(),
        280,
        "Compact drawer keeps its bounded width"
    );
    let rail = find_by_css_class(main.widget().upcast_ref(), "compact-nav-rail")
        .expect("Compact navigation rail");
    assert_eq!(rail.width_request(), 48, "Compact rail owns the 48px token");
    main.emit(MainWindowMsg::Allocated { width: 900 });
    assert!(flush_gtk());
    assert_eq!(
        paned.position(),
        268,
        "Compact must not overwrite the resize"
    );
    main.emit(MainWindowMsg::Allocated { width: 1_440 });
    assert!(flush_gtk());
    paned.set_position(320);
    main.emit(MainWindowMsg::Allocated { width: 1_440 });
    assert!(flush_gtk());
    assert_eq!(paned.position(), 280, "Wide clamps navigation to its token");
    main.emit(MainWindowMsg::Allocated { width: 800 });
    assert!(flush_gtk());
    main.emit(MainWindowMsg::Allocated { width: 900 });
    assert!(flush_gtk());
    assert_eq!(
        paned.position(),
        320,
        "Wide clamping must retain the user's Standard logical width"
    );

    main.widget().close();
    assert!(flush_gtk());
    assert_realized_main_window_satisfies_the_fluent_visual_contract();
}

fn find_by_css_class(root: &gtk::Widget, class: &str) -> Option<gtk::Widget> {
    if root.has_css_class(class) {
        return Some(root.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        if let Some(found) = find_by_css_class(&widget, class) {
            return Some(found);
        }
        child = widget.next_sibling();
    }
    None
}

fn wait_for_realized_pixels(window: &gtk::ApplicationWindow) -> rshell_ui::SmokePngEvidence {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match realized_pixels(window) {
            Ok(evidence) => return evidence,
            Err(_) if std::time::Instant::now() < deadline => {
                assert!(flush_gtk());
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("realized Fluent pixel ranges did not settle: {error}"),
        }
    }
}

fn realized_pixels(
    window: &gtk::ApplicationWindow,
) -> Result<rshell_ui::SmokePngEvidence, &'static str> {
    let (rgba, width, height) = rendered_rgba(window.upcast_ref())?;
    let accent = selection_treatment_surface(window.upcast_ref())
        .ok_or("realized active tab unavailable")?;
    let (accent_rgba, accent_width, accent_height) = rendered_rgba(&accent)?;
    analyze_rgba_with_accent(
        &rgba,
        width,
        height,
        &accent_rgba,
        accent_width,
        accent_height,
    )
}

fn rendered_rgba(widget: &gtk::Widget) -> Result<(Vec<u8>, i32, i32), &'static str> {
    let width = widget.width();
    let height = widget.height();
    if width <= 0 || height <= 0 {
        return Err("realized widget allocation unavailable");
    }
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot
        .to_node()
        .ok_or("realized snapshot node unavailable")?;
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer.realize(None).expect("Cairo renderer");
    let viewport = gtk::graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&viewport));
    renderer.unrealize();
    let stride = width as usize * 4;
    let mut native = vec![0; stride * height as usize];
    texture.download(&mut native, stride);
    let rgba = argb32_native_to_rgba(&native, NativeByteOrder::current()).unwrap();
    Ok((rgba, width, height))
}

fn visual_fixture() -> AppViewModel {
    let pane = PaneId::new();
    let session = SessionId::new();
    let tab = TabId::new_v4();
    let mut catalog = ConnectionCatalog::default();
    let profile = ConnectionProfile::new("Visual fixture", "safe.example.test");
    catalog.connections.insert(profile.id, profile);
    let mut view = AppViewModel::from(AppBootstrapState {
        catalog,
        settings: AppSettings::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    view.workspace = WorkspaceState {
        tabs: vec![TabState {
            id: tab,
            title: "Visual fixture".into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        }],
        active_tab: Some(tab),
    };
    view.pane_launches.insert(pane, PaneLaunchTarget::Local);
    view.session_states.insert(session, SessionState::Connected);
    view.latest_frames
        .insert(session, Arc::new(nonempty_frame()));
    view
}

fn nonempty_frame() -> RenderFrame {
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
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from(
                "Visual fixture"
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
        title: "Visual fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    }
}

struct AcceptingPort;

impl UiCommandPort for AcceptingPort {
    fn try_send(&self, _command: UiCommand) -> Result<(), UiPortError> {
        Ok(())
    }
}

fn flush_gtk() -> bool {
    let context = gtk::glib::MainContext::default();
    for _ in 0..512 {
        if !context.iteration(false) {
            return true;
        }
    }
    false
}
