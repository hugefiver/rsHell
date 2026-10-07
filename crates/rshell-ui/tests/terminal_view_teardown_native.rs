#[cfg(target_os = "macos")]
fn main() {
    println!(
        "TERMINAL_VIEW_TEARDOWN_NATIVE_SKIP platform=macos reason=windows_only_native_gui_integration"
    );
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn main() {
    println!("TERMINAL_VIEW_TEARDOWN_NATIVE_SKIP platform=non-windows");
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        cell::RefCell,
        ffi::c_void,
        rc::Rc,
        time::{Duration, Instant},
    };

    use gtk::prelude::*;
    use relm4::{Component, ComponentController};
    use rshell_core::{
        PaneId, SessionId, SessionUiCommand, TerminalInput, TerminalOverrides, TerminalSettingsV1,
        UiCommand,
    };
    use rshell_ui::{
        FontMetricEnvironment, FontMetricsService, MetricsChange, TerminalView, TerminalViewInit,
        TerminalViewOutput,
    };

    const DEADLINE: Duration = Duration::from_secs(4);

    pub(super) fn run() {
        gtk::init().expect("terminal teardown proof requires an available GTK display");
        let outputs = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&outputs);
        let terminal = TerminalView::builder()
            .launch(terminal_init())
            .connect_receiver(move |_, output| recorded.borrow_mut().push(output));
        let canvas = descendants(terminal.widget())
            .into_iter()
            .find_map(|widget| widget.downcast::<gtk::DrawingArea>().ok())
            .expect("terminal canvas");
        let search = descendants(terminal.widget())
            .into_iter()
            .find_map(|widget| widget.downcast::<gtk::SearchEntry>().ok())
            .expect("terminal search");
        let key = controller::<gtk::EventControllerKey>(&canvas);
        let im_context = key.im_context().expect("terminal IM context");
        let motion = controller::<gtk::EventControllerMotion>(&canvas);
        let scroll = controller::<gtk::EventControllerScroll>(&canvas);

        let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
        layout.append(terminal.widget());
        let focus_target = gtk::Button::with_label("focus target");
        layout.append(&focus_target);
        let window = gtk::Window::new();
        window.set_default_size(640, 360);
        window.set_child(Some(&layout));
        window.present();
        wait_for("terminal map", || canvas.is_mapped());
        foreground(&window);
        assert!(canvas.grab_focus());
        wait_for("terminal focus", || canvas.has_focus());
        outputs.borrow_mut().clear();

        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::Alt_L,
            gtk::gdk::ModifierType::ALT_MASK,
        ));
        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::from_name("x").expect("x key"),
            gtk::gdk::ModifierType::ALT_MASK,
        ));
        wait_for("left Alt input", || has_key_input(&outputs));
        assert!(
            take_key_alt(&outputs),
            "left Alt binding must remain active"
        );

        emit_key_released(&key, gtk::gdk::Key::Alt_L, gtk::gdk::ModifierType::empty());
        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::from_name("x").expect("x key"),
            gtk::gdk::ModifierType::ALT_MASK,
        ));
        wait_for("right Alt input", || has_key_input(&outputs));
        assert!(
            !take_key_alt(&outputs),
            "key release must clear stale left Alt state"
        );

        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::Alt_L,
            gtk::gdk::ModifierType::ALT_MASK,
        ));
        assert!(focus_target.grab_focus());
        wait_for("native focus leave", || focus_target.has_focus());
        assert!(canvas.grab_focus());
        wait_for("terminal refocus", || canvas.has_focus());
        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::from_name("x").expect("x key"),
            gtk::gdk::ModifierType::ALT_MASK,
        ));
        wait_for("focus-reset input", || has_key_input(&outputs));
        assert!(
            !take_key_alt(&outputs),
            "focus loss must reset physical Alt state"
        );

        im_context.emit_by_name::<()>("commit", &[&"界"]);
        wait_for("IME commit", || has_committed_text(&outputs, "界"));
        outputs.borrow_mut().clear();

        assert!(emit_key_pressed(
            &key,
            gtk::gdk::Key::from_name("f").expect("f key"),
            gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK,
        ));
        wait_for("search open", || search.is_visible());
        search.set_text("needle");
        wait_for("search delivery", || has_search(&outputs, "needle"));
        let search_key = controller::<gtk::EventControllerKey>(&search);
        assert!(emit_key_pressed(
            &search_key,
            gtk::gdk::Key::Escape,
            gtk::gdk::ModifierType::empty(),
        ));
        wait_for("search close", || {
            !search.is_visible() && canvas.has_focus()
        });
        outputs.borrow_mut().clear();

        assert!(scroll.emit_by_name::<bool>("scroll", &[&0.0f64, &1.0f64]));
        drain_pending();

        drop(terminal);
        drain_pending();
        assert!(focus_target.grab_focus());
        wait_for("post-shutdown focus leave", || focus_target.has_focus());
        assert!(
            !key.emit_by_name::<bool>(
                "key-pressed",
                &[
                    &gtk::gdk::Key::Return,
                    &0u32,
                    &gtk::gdk::ModifierType::empty()
                ],
            ),
            "a closed terminal receiver must let key input proceed",
        );
        assert!(
            !search_key.emit_by_name::<bool>(
                "key-pressed",
                &[
                    &gtk::gdk::Key::Return,
                    &0u32,
                    &gtk::gdk::ModifierType::empty(),
                ],
            ),
            "a closed terminal receiver must let search keys proceed",
        );
        key.emit_by_name::<()>(
            "key-released",
            &[
                &gtk::gdk::Key::Return,
                &0u32,
                &gtk::gdk::ModifierType::empty(),
            ],
        );
        im_context.emit_by_name::<()>("commit", &[&"after-shutdown"]);
        search.set_text("after-shutdown");
        motion.emit_by_name::<()>("motion", &[&1.0f64, &1.0f64]);
        assert!(!scroll.emit_by_name::<bool>("scroll", &[&0.0f64, &1.0f64]));
        drain_pending();

        window.close();
        wait_for("post-shutdown unmap", || !canvas.is_mapped());
        println!(
            "TERMINAL_VIEW_TEARDOWN_NATIVE_PASS active_key=true active_release=true active_focus=true active_ime=true active_search=true active_pointer=true closed_key_proceed=true late_callbacks_safe=true"
        );
    }

    fn terminal_init() -> TerminalViewInit {
        let settings = TerminalSettingsV1 {
            left_alt_as_meta: true,
            right_alt_as_meta: false,
            ..TerminalSettingsV1::default()
        };
        let profile = settings.resolve(&TerminalOverrides::default());
        let probe = gtk::Label::new(None);
        let context = probe.pango_context();
        let environment =
            FontMetricEnvironment::from_context(&context, f64::from(probe.scale_factor()))
                .expect("native metric environment");
        let metrics = match FontMetricsService::default()
            .measure(&context, &profile, environment)
            .expect("native metrics")
        {
            MetricsChange::Changed(metrics) | MetricsChange::Unchanged(metrics) => metrics,
        };
        TerminalViewInit {
            pane: PaneId::new(),
            session: SessionId::new(),
            profile,
            metrics,
            startup_probe: None,
        }
    }

    fn take_key_alt(outputs: &Rc<RefCell<Vec<TerminalViewOutput>>>) -> bool {
        let mut outputs = outputs.borrow_mut();
        let index = outputs
            .iter()
            .position(is_key_input)
            .expect("key input output");
        let output = outputs.remove(index);
        match output {
            TerminalViewOutput::Command(command) => match *command {
                UiCommand::Session {
                    command: SessionUiCommand::Input(TerminalInput::Key { modifiers, .. }),
                    ..
                } => modifiers.alt,
                other => panic!("expected key input, got {other:?}"),
            },
            other => panic!("expected command output, got {other:?}"),
        }
    }

    fn has_key_input(outputs: &Rc<RefCell<Vec<TerminalViewOutput>>>) -> bool {
        outputs.borrow().iter().any(is_key_input)
    }

    fn is_key_input(output: &TerminalViewOutput) -> bool {
        matches!(
            output,
            TerminalViewOutput::Command(command)
                if matches!(command.as_ref(), UiCommand::Session {
                    command: SessionUiCommand::Input(TerminalInput::Key { .. }), ..
                })
        )
    }

    fn has_committed_text(outputs: &Rc<RefCell<Vec<TerminalViewOutput>>>, expected: &str) -> bool {
        outputs.borrow().iter().any(|output| matches!(output,
            TerminalViewOutput::Command(command) if matches!(command.as_ref(),
                UiCommand::Session { command: SessionUiCommand::Input(TerminalInput::CommittedText(text)), .. } if text == expected)))
    }

    fn has_search(outputs: &Rc<RefCell<Vec<TerminalViewOutput>>>, expected: &str) -> bool {
        outputs.borrow().iter().any(|output| matches!(output,
            TerminalViewOutput::Command(command) if matches!(command.as_ref(),
                UiCommand::Session { command: SessionUiCommand::Search(query), .. } if query.needle == expected)))
    }

    fn controller<T: gtk::glib::object::IsA<gtk::EventController> + gtk::glib::object::ObjectType>(
        widget: &impl IsA<gtk::Widget>,
    ) -> T
    where
        gtk::glib::Object: gtk::glib::object::MayDowncastTo<T>,
    {
        let controllers = widget.observe_controllers();
        (0..controllers.n_items())
            .filter_map(|index| controllers.item(index))
            .find_map(|value| value.downcast::<T>().ok())
            .expect("required controller")
    }

    fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
        let mut output = Vec::new();
        let mut pending = vec![root.as_ref().clone()];
        while let Some(widget) = pending.pop() {
            let mut child = widget.first_child();
            while let Some(current) = child {
                output.push(current.clone());
                pending.push(current.clone());
                child = current.next_sibling();
            }
        }
        output
    }

    fn wait_for(label: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + DEADLINE;
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {label}");
            gtk::glib::MainContext::default().iteration(true);
        }
        drain_pending();
    }

    fn drain_pending() {
        let context = gtk::glib::MainContext::default();
        while context.iteration(false) {}
    }

    fn foreground(window: &gtk::Window) {
        let surface = window.surface().expect("native surface");
        let hwnd = unsafe { gdk_win32_surface_get_handle(surface.as_ptr()) };
        assert!(!hwnd.is_null());
        unsafe {
            SetForegroundWindow(hwnd);
        }
        wait_for("native window activation", || window.is_active());
    }

    fn emit_key_pressed(
        controller: &gtk::EventControllerKey,
        key: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) -> bool {
        controller.emit_by_name("key-pressed", &[&key, &0u32, &state])
    }

    fn emit_key_released(
        controller: &gtk::EventControllerKey,
        key: gtk::gdk::Key,
        state: gtk::gdk::ModifierType,
    ) {
        controller.emit_by_name::<()>("key-released", &[&key, &0u32, &state]);
    }

    #[link(name = "gtk-4")]
    unsafe extern "C" {
        fn gdk_win32_surface_get_handle(surface: *mut gtk::gdk::ffi::GdkSurface) -> *mut c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetForegroundWindow(window: *mut c_void) -> i32;
    }
}

#[cfg(target_os = "windows")]
fn main() {
    windows::run();
}
