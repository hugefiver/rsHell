//! Real Windows pointer input; keep GTK callbacks non-panicking and restore injected state.
use super::frames::wait_for_frame;
use relm4::gtk::{self, prelude::*};
use std::{cell::Cell, ffi::c_void, panic, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EventKind {
    Press,
    Release,
}

#[derive(Clone, Copy, Debug)]
struct PointerEvent {
    kind: EventKind,
    button: u32,
    x: f64,
    y: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct PointerTrace {
    events: [Option<PointerEvent>; 2],
    count: usize,
    drag_motion: Option<(f64, f64)>,
}

impl PointerTrace {
    fn record(&mut self, event: PointerEvent) {
        if self.count < self.events.len() {
            self.events[self.count] = Some(event);
        }
        self.count = self.count.saturating_add(1);
    }

    fn saw(&self, kind: EventKind) -> bool {
        self.events.iter().flatten().any(|event| event.kind == kind)
    }

    fn record_motion(&mut self, x: f64, y: f64, state: gtk::gdk::ModifierType) {
        if self.saw(EventKind::Press)
            && !self.saw(EventKind::Release)
            && state.contains(gtk::gdk::ModifierType::BUTTON1_MASK)
        {
            self.drag_motion = Some((x, y));
        }
    }

    fn saw_drag_movement(&self) -> bool {
        self.drag_motion
            .is_some_and(|(x, y)| (x - 130.0).abs() <= 1.0 && (y - 10.0).abs() <= 1.0)
    }
}

fn validate_trace(trace: PointerTrace) -> Result<(), &'static str> {
    if trace.count != 2 {
        return Err("expected exactly one press and one release");
    }
    let [Some(press), Some(release)] = trace.events else {
        return Err("missing pointer event");
    };
    if press.kind != EventKind::Press || release.kind != EventKind::Release {
        return Err("pointer event order differs from press then release");
    }
    if press.button != 1 || release.button != 1 {
        return Err("pointer event button differs from left button");
    }
    if ![(press, 30.0), (release, 130.0)]
        .iter()
        .all(|(event, x)| (event.x - *x).abs() <= 1.0 && (event.y - 10.0).abs() <= 1.0)
    {
        return Err("pointer event canvas coordinates outside ±1 tolerance");
    }
    Ok(())
}

// Suspend a failed wait until its cleanup has run; never unwind across a GTK signal callback.
fn cleanup_before_report<T, C>(
    state: &mut C,
    action: impl FnOnce(&mut C) -> T,
    cleanup: impl FnOnce(&mut C) -> bool,
) -> T {
    let outcome = panic::catch_unwind(panic::AssertUnwindSafe(|| action(state)));
    let restored = cleanup(state);
    assert!(restored, "failed to restore original cursor position");
    match outcome {
        Ok(value) => value,
        Err(error) => panic::resume_unwind(error),
    }
}

pub(super) fn drag(window: &gtk::ApplicationWindow, canvas: &gtk::Widget) {
    let controllers = canvas.observe_controllers();
    let click = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::GestureClick>().ok())
        .unwrap();
    let motion = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::EventControllerMotion>().ok())
        .unwrap();
    let bounds = canvas.compute_bounds(window).unwrap();
    let (surface_x, surface_y) = window.surface_transform();
    let scale = window.scale_factor() as f32;
    let point = |x: f32| {
        (
            ((bounds.x() + x + surface_x as f32) * scale) as i32,
            ((bounds.y() + 10.0 + surface_y as f32) * scale) as i32,
        )
    };
    let hwnd = unsafe { gdk_win32_surface_get_handle(window.surface().unwrap().as_ptr()) };
    assert!(!hwnd.is_null());
    let mut original = Point { x: 0, y: 0 };
    assert_ne!(unsafe { GetCursorPos(&mut original) }, 0);
    let trace = Rc::new(Cell::new(PointerTrace::default()));
    let seen = trace.clone();
    let press = click.connect_pressed(move |g, _, x, y| {
        let mut events = seen.get();
        events.record(PointerEvent {
            kind: EventKind::Press,
            button: g.current_button(),
            x,
            y,
        });
        seen.set(events);
    });
    let seen = trace.clone();
    let release = click.connect_released(move |g, _, x, y| {
        let mut events = seen.get();
        events.record(PointerEvent {
            kind: EventKind::Release,
            button: g.current_button(),
            x,
            y,
        });
        seen.set(events);
    });
    let mut cursor = CursorRestore {
        click,
        handlers: [Some(press), Some(release)],
        motion,
        motion_handler: None,
        original: Some(original),
        injected_down: false,
    };
    let seen = trace.clone();
    cursor.motion_handler = Some(cursor.motion.connect_motion(move |controller, x, y| {
        let mut events = seen.get();
        events.record_motion(x, y, controller.current_event_state());
        seen.set(events);
    }));
    let observed = cleanup_before_report(
        &mut cursor,
        |guard| {
            assert_ne!(unsafe { SetForegroundWindow(hwnd) }, 0);
            // Stay outside GtkPaned's wider native separator hit region.
            move_pointer(hwnd, point(30.0));
            wait_for_frame(canvas, "pointer positioning painted", |_| true);
            guard.injected_down = true;
            unsafe { mouse_event(0x0002, 0, 0, 0, 0) };
            let seen = trace.clone();
            wait_for_frame(canvas, "actual native left-button press", move |_| {
                seen.get().saw(EventKind::Press)
            });
            move_pointer(hwnd, point(130.0));
            let seen = trace.clone();
            // Paint may precede native motion dispatch. Releasing then can overtake
            // the move, leaving only the release-generated Select at the port.
            wait_for_frame(canvas, "native drag movement", move |_| {
                seen.get().saw_drag_movement()
            });
            unsafe { mouse_event(0x0004, 0, 0, 0, 0) };
            let seen = trace.clone();
            wait_for_frame(canvas, "actual native left-button release", move |_| {
                seen.get().saw(EventKind::Release)
            });
            trace.get()
        },
        |guard| {
            let restored = guard.finish();
            eprintln!("POINTER_INPUT observed={:?}", trace.get());
            restored
        },
    );
    if let Err(reason) = validate_trace(observed) {
        panic!("native pointer trace invalid: {reason}: {observed:?}");
    }
    println!(
        "POINTER_INPUT native_press_release=true canvas_local=30,10..130,10 surface_transform={surface_x},{surface_y} cursor_restored=true"
    );
}

fn move_pointer(hwnd: *mut c_void, (x, y): (i32, i32)) {
    let mut point = Point { x, y };
    assert_ne!(unsafe { ClientToScreen(hwnd, &mut point) }, 0);
    assert_ne!(unsafe { SetCursorPos(point.x, point.y) }, 0);
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
struct CursorRestore {
    click: gtk::GestureClick,
    handlers: [Option<gtk::glib::SignalHandlerId>; 2],
    motion: gtk::EventControllerMotion,
    motion_handler: Option<gtk::glib::SignalHandlerId>,
    original: Option<Point>,
    injected_down: bool,
}
impl CursorRestore {
    fn finish(&mut self) -> bool {
        if let Some(id) = self.motion_handler.take() {
            self.motion.disconnect(id);
        }
        for handler in &mut self.handlers {
            if let Some(id) = handler.take() {
                self.click.disconnect(id);
            }
        }
        if self.injected_down {
            unsafe { mouse_event(0x0004, 0, 0, 0, 0) };
            self.injected_down = false;
        }
        if let Some(original) = &self.original {
            if unsafe { SetCursorPos(original.x, original.y) } == 0 {
                return false;
            }
            self.original = None;
        }
        true
    }
}
impl Drop for CursorRestore {
    fn drop(&mut self) {
        self.finish();
    }
}

#[link(name = "gtk-4")]
unsafe extern "C" {
    fn gdk_win32_surface_get_handle(surface: *mut gtk::gdk::ffi::GdkSurface) -> *mut c_void;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut Point) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn ClientToScreen(hwnd: *mut c_void, point: *mut Point) -> i32;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn mouse_event(flags: u32, dx: u32, dy: u32, data: u32, extra: usize);
}

#[cfg(test)]
#[path = "windows_pointer_tests.rs"]
mod tests;
