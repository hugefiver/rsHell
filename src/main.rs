use relm4::RelmApp;
use rshell::app::RshellApp;
#[cfg(windows)]
use std::{
    ffi::{c_char, c_void},
    path::{Path, PathBuf},
};

fn main() {
    configure_process_dpi_awareness();
    configure_windows_portable_runtime();
    suppress_gio_warnings();
    RelmApp::new("io.github.hugefiver.rshell").run::<RshellApp>(());
}

#[cfg(windows)]
fn configure_process_dpi_awareness() {
    const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

    type SetProcessDpiAwarenessContext = unsafe extern "system" fn(isize) -> i32;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    unsafe {
        let user32 = LoadLibraryA(c"user32.dll".as_ptr());
        if user32.is_null() {
            return;
        }

        let proc = GetProcAddress(user32, c"SetProcessDpiAwarenessContext".as_ptr());
        if proc.is_null() {
            return;
        }

        let set_dpi_awareness: SetProcessDpiAwarenessContext = std::mem::transmute(proc);
        let _ = set_dpi_awareness(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[cfg(not(windows))]
fn configure_process_dpi_awareness() {}

#[cfg(windows)]
fn configure_windows_portable_runtime() {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let Some(app_dir) = exe_path.parent() else {
        return;
    };

    prepend_path_env("PATH", app_dir);

    let share_dir = app_dir.join("share");
    if share_dir.is_dir() {
        prepend_path_env("XDG_DATA_DIRS", &share_dir);
        unsafe {
            std::env::set_var("GTK_DATA_PREFIX", app_dir);
            std::env::set_var("GTK_EXE_PREFIX", app_dir);
        }
    }

    set_env_if_exists(
        "GSETTINGS_SCHEMA_DIR",
        &share_dir.join("glib-2.0").join("schemas"),
    );
    set_env_if_exists(
        "GDK_PIXBUF_MODULE_FILE",
        &app_dir
            .join("lib")
            .join("gdk-pixbuf-2.0")
            .join("2.10.0")
            .join("loaders.cache"),
    );
    set_env_if_exists(
        "GDK_PIXBUF_MODULEDIR",
        &app_dir
            .join("lib")
            .join("gdk-pixbuf-2.0")
            .join("2.10.0")
            .join("loaders"),
    );

    let fontconfig_dir = app_dir.join("etc").join("fonts");
    set_env_if_exists("FONTCONFIG_FILE", &fontconfig_dir.join("fonts.conf"));
    set_env_if_exists("FONTCONFIG_PATH", &fontconfig_dir);
}

#[cfg(not(windows))]
fn configure_windows_portable_runtime() {}

#[cfg(windows)]
fn prepend_path_env(name: &str, value: &Path) {
    if !value.exists() {
        return;
    }

    let mut entries = vec![PathBuf::from(value)];
    if let Some(existing) = std::env::var_os(name) {
        entries.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(entries) {
        unsafe {
            std::env::set_var(name, joined);
        }
    }
}

#[cfg(windows)]
fn set_env_if_exists(name: &str, path: &Path) {
    if path.exists() {
        unsafe {
            std::env::set_var(name, path);
        }
    }
}

fn suppress_gio_warnings() {
    #[cfg(windows)]
    unsafe {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "disabled:");
    }

    unsafe {
        std::env::set_var("G_MESSAGES_DEBUG", "");
    }
}
