use relm4::gtk;
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct WinRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(C)]
#[derive(Default)]
pub(super) struct MonitorInfo {
    pub size: u32,
    pub monitor: WinRect,
    pub work: WinRect,
    pub flags: u32,
}
#[repr(C)]
pub(super) struct Point {
    pub x: i32,
    pub y: i32,
}
#[link(name = "gtk-4")]
unsafe extern "C" {
    pub(super) fn gdk_win32_surface_get_handle(
        surface: *mut gtk::gdk::ffi::GdkSurface,
    ) -> *mut c_void;
}
#[link(name = "user32")]
unsafe extern "system" {
    pub(super) fn GetSystemMetrics(index: i32) -> i32;
    pub(super) fn SystemParametersInfoW(
        action: u32,
        param: u32,
        output: *mut c_void,
        flags: u32,
    ) -> i32;
    pub(super) fn GetWindowRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
    pub(super) fn GetClientRect(hwnd: *mut c_void, rect: *mut WinRect) -> i32;
    pub(super) fn ClientToScreen(hwnd: *mut c_void, point: *mut Point) -> i32;
    pub(super) fn MonitorFromWindow(hwnd: *mut c_void, flags: u32) -> *mut c_void;
    pub(super) fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
}
