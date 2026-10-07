//! Bounded GTK primitives shared by the single Fluent native libtest entry.
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    AppBootstrapState, AppViewModel, TerminalProfile, UiCommand, UiCommandPort, UiPortError,
};
use rshell_ui::{MainWindow, MainWindowInit};
use std::{
    cell::Cell,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

#[path = "fluent_pixels.rs"]
mod image;
pub(crate) use image::{capture, is_accent, is_control, pixels};
#[path = "fluent_frames.rs"]
mod frames;
use frames::iterate_until;
pub(crate) use frames::wait_for_frame;
pub(crate) use frames::{FailurePhase, FrameFailure, wait_for_frame_with_failure_report};

pub(crate) fn launch(width: i32, height: i32) -> relm4::Controller<MainWindow> {
    launch_with_port(width, height, Arc::new(AcceptingPort))
}

pub(crate) fn launch_with_port(
    width: i32,
    height: i32,
    port: Arc<dyn UiCommandPort>,
) -> relm4::Controller<MainWindow> {
    let view = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    let main = MainWindow::builder()
        .launch(MainWindowInit::new(port, view).with_file_selection(Rc::new(FixtureSelection)))
        .detach();
    main.widget().set_default_size(width, height);
    main.widget().present();
    wait_for_frame(main.widget(), "main window allocation", |root| {
        let widgets = descendants(root);
        ["shell-compact", "shell-standard", "shell-wide"]
            .iter()
            .any(|class| widgets.iter().any(|widget| widget.has_css_class(class)))
    });
    let widgets = descendants(main.widget().upcast_ref());
    let actual_mode = ["shell-compact", "shell-standard", "shell-wide"]
        .into_iter()
        .find(|class| widgets.iter().any(|widget| widget.has_css_class(class)))
        .unwrap();
    let scale = main.widget().scale_factor();
    let dpi = main
        .widget()
        .pango_context()
        .font_map()
        .and_then(|map| map.downcast::<pangocairo::FontMap>().ok())
        .map(|map| pangocairo::prelude::PangoCairoFontMapExt::resolution(&map));
    println!(
        "FLUENT_ALLOCATION requested={width}x{height} realized={}x{} mode={actual_mode} scale={scale} pango_dpi={dpi:?}",
        main.widget().width(),
        main.widget().height()
    );
    main
}

// Only the historical cosmetic fixture uses this. State proof uses a recording port.
struct AcceptingPort;
impl UiCommandPort for AcceptingPort {
    fn try_send(&self, _command: UiCommand) -> Result<(), UiPortError> {
        Ok(())
    }
}

// Selection is synthetic, but the production Choose -> callback -> command binding runs.
// No file is read or created, and the recording port never retains the path.
struct FixtureSelection;
impl rshell_platform::FileSelectionService for FixtureSelection {
    fn select_file(
        &self,
        _: rshell_platform::FileSelectionRequest,
        complete: rshell_platform::FileSelectionCallback,
    ) {
        complete(Ok(Some(std::path::PathBuf::from("fluent-form-fixture"))));
    }
}

pub(crate) fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut output = Vec::new();
    let mut child = root.first_child();
    while let Some(widget) = child {
        output.push(widget.clone());
        output.extend(descendants(&widget));
        child = widget.next_sibling();
    }
    output
}

pub(crate) fn label(root: &gtk::Widget, text: &str) -> gtk::Label {
    descendants(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
        .find(|label| label.text() == text && label.is_mapped())
        .unwrap_or_else(|| panic!("missing fixed fixture label {text}"))
}

pub(crate) fn button(root: &gtk::Widget, text: &str) -> gtk::Button {
    label(root, text)
        .ancestor(gtk::Button::static_type())
        .unwrap()
        .downcast()
        .unwrap()
}

pub(crate) fn field_for_label(root: &gtk::Widget, text: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
        .filter(|label| label.text() == text && label.is_mapped())
        .find_map(|label| {
            let grid = label.parent()?.downcast::<gtk::Grid>().ok()?;
            let (column, row, span, _) = grid.query_child(&label);
            (column == 0 && span == 1)
                .then(|| grid.child_at(1, row))
                .flatten()
        })
        .unwrap_or_else(|| panic!("missing fixed fixture field {text}"))
}

fn allocated(widget: &gtk::Widget) -> bool {
    widget.is_mapped() && widget.width() > 0 && widget.height() > 0
}

pub(crate) fn focus_within(widget: &gtk::Widget) -> bool {
    widget
        .root()
        .and_then(|root| gtk::prelude::RootExt::focus(&root))
        .is_some_and(|focused| focused == *widget || focused.is_ancestor(widget))
}

pub(crate) fn native_key(
    widget: &gtk::Widget,
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) {
    let controllers = widget.observe_controllers();
    let handled = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .filter_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
        .any(|c| c.emit_by_name::<bool>("key-pressed", &[&key, &0u32, &modifiers]));
    if !handled && key == gtk::gdk::Key::Tab {
        // Non-boundary Tab propagates to GTK's native focus traversal.
        let direction = if modifiers.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
            gtk::DirectionType::TabBackward
        } else {
            gtk::DirectionType::TabForward
        };
        assert!(
            widget
                .root()
                .unwrap()
                .downcast::<gtk::Window>()
                .unwrap()
                .child_focus(direction)
        );
    } else {
        assert!(handled, "production key controller must handle fixture key");
    }
}

pub(crate) fn modal_ready(root: &gtk::Widget, class: &str) -> bool {
    descendants(root).into_iter().any(|modal| {
        modal.has_css_class(class)
            && allocated(&modal)
            && descendants(&modal).iter().any(|widget| {
                widget.has_css_class("modal-focus-first")
                    && allocated(widget)
                    && focus_within(widget)
            })
    })
}

pub(crate) fn close_window(window: &gtk::ApplicationWindow) {
    window.close();
    assert!(
        iterate_until(Instant::now() + Duration::from_secs(2), || !window
            .is_mapped()),
        "window did not unmap before deadline"
    );
}

pub(crate) fn assert_wait_deadline() {
    let dispatches = Rc::new(Cell::new(0));
    let idle_dispatches = dispatches.clone();
    let source = gtk::glib::idle_add_local(move || {
        idle_dispatches.set(idle_dispatches.get() + 1);
        gtk::glib::ControlFlow::Continue
    });
    let budget = Duration::from_millis(25);
    let started = Instant::now();
    let completed = iterate_until(started + budget, || false);
    source.remove();
    assert!(!completed, "unready condition must time out");
    assert!(
        dispatches.get() > 0,
        "negative case must dispatch the busy source"
    );
    assert!(started.elapsed() >= budget);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "pending source defeated deadline"
    );
    assert!(
        !iterate_until(Instant::now(), || true),
        "expired deadline must reject readiness"
    );
    println!("FLUENT_WAIT_DEADLINE_PASS continuous_pending=true expired_ready_rejected=true");
}
