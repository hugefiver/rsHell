//! Read-only, bounded Windows frame-failure geometry; no native input or display changes.
use relm4::gtk::{self, prelude::*};
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "windows_geometry_native.rs"]
mod native;
use native::*;

static REPORTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
pub(super) enum FailurePhase {
    FrameClock,
    AfterPaint,
}

#[derive(Clone, Copy, Default)]
struct Rect(i32, i32, i32, i32);
impl Rect {
    fn from_win(rect: WinRect) -> Self {
        Self(
            rect.left,
            rect.top,
            rect.right.saturating_sub(rect.left),
            rect.bottom.saturating_sub(rect.top),
        )
    }
}
fn rect(value: Option<Rect>) -> String {
    value.map_or_else(
        || "unavailable".into(),
        |r| format!("{},{},{},{}", r.0, r.1, r.2, r.3),
    )
}
fn size(value: Option<(i32, i32)>) -> String {
    value.map_or_else(|| "unavailable".into(), |(w, h)| format!("{w}x{h}"))
}

#[derive(Clone, Copy, Default)]
struct WidgetState {
    size: (i32, i32),
    scale: i32,
    mapped: bool,
    realized: bool,
}
impl WidgetState {
    fn from(widget: &gtk::Widget) -> Self {
        Self {
            size: (widget.width(), widget.height()),
            scale: widget.scale_factor(),
            mapped: widget.is_mapped(),
            realized: widget.is_realized(),
        }
    }
}
#[derive(Default)]
struct Snapshot {
    target: WidgetState,
    root: Option<(WidgetState, (i32, i32), bool, bool)>,
    surface: Option<(i32, i32, i32)>,
    gdk_monitor: Option<(Rect, i32)>,
    primary: Option<(i32, i32)>,
    virtual_screen: Option<Rect>,
    primary_workarea: Option<Rect>,
    monitor: Option<(Rect, Rect)>,
    window: Option<Rect>,
    client_local: Option<Rect>,
    client_screen: Option<Rect>,
}
impl Snapshot {
    fn capture(widget: &gtk::Widget) -> Self {
        let mut result = Self {
            target: WidgetState::from(widget),
            ..Self::default()
        };
        let primary = (unsafe { GetSystemMetrics(0) }, unsafe {
            GetSystemMetrics(1)
        });
        result.primary = (primary.0 > 0 && primary.1 > 0).then_some(primary);
        let virtual_screen = Rect(
            unsafe { GetSystemMetrics(76) },
            unsafe { GetSystemMetrics(77) },
            unsafe { GetSystemMetrics(78) },
            unsafe { GetSystemMetrics(79) },
        );
        result.virtual_screen =
            (virtual_screen.2 > 0 && virtual_screen.3 > 0).then_some(virtual_screen);
        let mut work = WinRect::default();
        if unsafe { SystemParametersInfoW(48, 0, (&mut work as *mut WinRect).cast(), 0) } != 0 {
            result.primary_workarea = Some(Rect::from_win(work));
        }
        let Some(root) = widget
            .root()
            .and_then(|root| root.downcast::<gtk::Window>().ok())
        else {
            return result;
        };
        result.root = Some((
            WidgetState::from(root.upcast_ref()),
            root.default_size(),
            root.is_maximized(),
            root.is_fullscreen(),
        ));
        let Some(surface) = root.surface() else {
            return result;
        };
        result.surface = Some((surface.width(), surface.height(), surface.scale_factor()));
        result.gdk_monitor = gtk::prelude::WidgetExt::display(&root)
            .monitor_at_surface(&surface)
            .map(|monitor| {
                let geometry = monitor.geometry();
                (
                    Rect(
                        geometry.x(),
                        geometry.y(),
                        geometry.width(),
                        geometry.height(),
                    ),
                    monitor.scale_factor(),
                )
            });
        // Only the surface owned by this target's GTK root is queried; never use foreground HWND.
        let hwnd = unsafe { gdk_win32_surface_get_handle(surface.as_ptr()) };
        if hwnd.is_null() {
            return result;
        }
        let mut win = WinRect::default();
        if unsafe { GetWindowRect(hwnd, &mut win) } != 0 {
            result.window = Some(Rect::from_win(win));
        }
        let monitor = unsafe { MonitorFromWindow(hwnd, 2) };
        if !monitor.is_null() {
            let mut info = MonitorInfo {
                size: std::mem::size_of::<MonitorInfo>() as u32,
                ..MonitorInfo::default()
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
                result.monitor = Some((Rect::from_win(info.monitor), Rect::from_win(info.work)));
            }
        }
        let mut client = WinRect::default();
        if unsafe { GetClientRect(hwnd, &mut client) } != 0 {
            result.client_local = Some(Rect::from_win(client));
            let mut top_left = Point {
                x: client.left,
                y: client.top,
            };
            let mut bottom_right = Point {
                x: client.right,
                y: client.bottom,
            };
            if unsafe { ClientToScreen(hwnd, &mut top_left) } != 0
                && unsafe { ClientToScreen(hwnd, &mut bottom_right) } != 0
            {
                result.client_screen = Some(Rect(
                    top_left.x,
                    top_left.y,
                    bottom_right.x.saturating_sub(top_left.x),
                    bottom_right.y.saturating_sub(top_left.y),
                ));
            }
        }
        result
    }

    fn lines(&self, phase: FailurePhase) -> [String; 7] {
        let phase = match phase {
            FailurePhase::FrameClock => "frame_clock",
            FailurePhase::AfterPaint => "after_paint",
        };
        let widget = |w: WidgetState| {
            format!(
                "{}x{} scale={} mapped={} realized={}",
                w.size.0, w.size.1, w.scale, w.mapped, w.realized
            )
        };
        let root = self.root.map_or_else(
            || "unavailable".into(),
            |(w, default, maximized, fullscreen)| {
                format!(
                    "{} default={}x{} maximized={} fullscreen={}",
                    widget(w),
                    default.0,
                    default.1,
                    maximized,
                    fullscreen
                )
            },
        );
        let surface = self.surface.map_or_else(
            || "unavailable".into(),
            |(w, h, scale)| format!("{w}x{h} scale={scale}"),
        );
        let monitor = self.gdk_monitor.map_or_else(
            || "unavailable".into(),
            |(r, scale)| format!("{} scale={scale}", rect(Some(r))),
        );
        let native_monitor = self.monitor.map_or_else(
            || ("unavailable".into(), "unavailable".into()),
            |(r, work)| (rect(Some(r)), rect(Some(work))),
        );
        [
            format!(
                "WIN_FRAME_GEOMETRY phase={phase} units=gtk_gdk_logical_win32_physical_or_dpi_virtualized_unknown"
            ),
            format!("WIN_FRAME_GEOMETRY gtk_target={}", widget(self.target)),
            format!("WIN_FRAME_GEOMETRY gtk_root={root}"),
            format!("WIN_FRAME_GEOMETRY gdk_surface={surface} gdk_monitor_xywh={monitor}"),
            format!(
                "WIN_FRAME_GEOMETRY win32_primary_wh={} win32_virtual_xywh={}",
                size(self.primary),
                rect(self.virtual_screen)
            ),
            format!(
                "WIN_FRAME_GEOMETRY win32_primary_work_xywh={} win32_target_monitor_xywh={} win32_target_work_xywh={}",
                rect(self.primary_workarea),
                native_monitor.0,
                native_monitor.1
            ),
            format!(
                "WIN_FRAME_GEOMETRY win32_target_window_screen_xywh={} win32_target_client_local_xywh={} win32_target_client_screen_xywh={}",
                rect(self.window),
                rect(self.client_local),
                rect(self.client_screen)
            ),
        ]
    }
}

fn failure_lines(
    complete: bool,
    phase: FailurePhase,
    budget: &AtomicBool,
    sample: impl FnOnce() -> Snapshot,
) -> Option<[String; 7]> {
    if complete || budget.swap(true, Ordering::Relaxed) {
        return None;
    }
    Some(sample().lines(phase))
}
pub(super) fn report_if_failed(complete: bool, phase: FailurePhase, widget: &gtk::Widget) {
    if let Some(lines) = failure_lines(complete, phase, &REPORTED, || Snapshot::capture(widget)) {
        for line in lines {
            eprintln!("{line}");
        }
    }
}

#[cfg(test)]
#[path = "windows_geometry_tests.rs"]
mod tests;
