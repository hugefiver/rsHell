use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use relm4::SimpleComponent;
use rshell_core::{
    AppBootstrapState, AppViewModel, TerminalProfile, UiCommand, UiCommandPort, UiPortError,
};
use rshell_ui::{
    ConnectionEditor, ConnectionSidebar, MainWindow, MainWindowInit, MainWindowMsg, PaneHost,
    SessionTabBar, TerminalView,
};

#[derive(Default)]
struct RecordingPort {
    commands: Mutex<Vec<UiCommand>>,
}

impl UiCommandPort for RecordingPort {
    fn try_send(&self, command: UiCommand) -> Result<(), UiPortError> {
        self.commands.lock().unwrap().push(command);
        Ok(())
    }
}

fn assert_component<T: SimpleComponent>() {}

#[test]
fn public_outputs_are_real_relm4_components() {
    assert_component::<MainWindow>();
    assert_component::<ConnectionSidebar>();
    assert_component::<ConnectionEditor>();
    assert_component::<TerminalView>();
    assert_component::<SessionTabBar>();
    assert_component::<PaneHost>();

    let port: Arc<dyn UiCommandPort> = Arc::new(RecordingPort::default());
    let view_model = AppViewModel::from(AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![TerminalProfile::default()],
    });
    let init = MainWindowInit::new(port, view_model);
    assert!(init.latest_view_stream().is_none());
    assert!(!format!("{init:?}").contains("Secret"));
    let _event_surface = MainWindowMsg::ReplaceViewModel;
}

#[test]
fn production_manifest_has_only_the_allowed_ui_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read rshell-ui manifest");
    let dependencies = dependency_names(&manifest);
    let allowed = [
        "gtk",
        "pangocairo",
        "relm4",
        "rshell-core",
        "rshell-platform",
    ];
    assert!(
        dependencies.iter().all(|name| allowed.contains(name)),
        "unexpected production dependency: {dependencies:?}"
    );
    for required in [
        "gtk",
        "pangocairo",
        "relm4",
        "rshell-core",
        "rshell-platform",
    ] {
        assert!(
            dependencies.contains(&required),
            "missing dependency {required}"
        );
    }
    for forbidden in [
        "rshell-storage",
        "rshell-session",
        "rusqlite",
        "keyring",
        "russh",
        "portable-pty",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "forbidden dependency {forbidden}"
        );
    }
}

#[test]
fn production_modules_remain_focused() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("read production source");
        let pure_lines = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with("//")
            })
            .count();
        assert!(
            pure_lines <= 250,
            "{} has {pure_lines} pure lines (limit 250)",
            file.display()
        );
    }
}

#[test]
fn production_sources_do_not_import_infrastructure_crates() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty());

    for file in files {
        let source = fs::read_to_string(&file).expect("read production source");
        for forbidden in [
            "rshell_storage",
            "rshell_session",
            "rusqlite",
            "keyring",
            "russh",
            "portable_pty",
            "ConnectionRepository",
            "CredentialPort",
            "SessionPort",
        ] {
            assert!(
                !source.contains(forbidden),
                "{} imports forbidden infrastructure symbol {forbidden}",
                file.display()
            );
        }
    }
}

#[test]
fn product_commands_leave_ui_through_one_command_port_adapter() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    let mut try_send_sites = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("read production source");
        if source.contains(".try_send(") {
            try_send_sites.push(file);
        }
    }
    assert_eq!(
        try_send_sites.len(),
        1,
        "unexpected command-port send sites"
    );
    assert!(try_send_sites[0].ends_with("command_port.rs"));

    let main_window = fs::read_to_string(src.join("main_window.rs")).expect("read MainWindow");
    let main_window_commands =
        fs::read_to_string(src.join("main_window_commands.rs")).expect("read MainWindow commands");
    assert!(main_window_commands.contains("dispatch(&self.command_port"));
    assert!(!main_window.contains(".try_send("));
    assert!(!main_window_commands.contains(".try_send("));
}

#[test]
fn production_shell_forbids_generic_widget_unparent() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let required = ["main_window_shell.rs", "main_window_layout.rs"];
    let future = ["navigation_drawer.rs"];

    for name in required {
        let path = src.join(name);
        let source = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!("read required shell owner {}: {error}", path.display())
        });
        assert_typed_shell_ownership(&path, &source);
    }
    for name in future {
        let path = src.join(name);
        if let Ok(source) = fs::read_to_string(&path) {
            assert_typed_shell_ownership(&path, &source);
        }
    }
}

#[test]
fn production_ui_has_no_blocking_or_unbounded_runtime_primitives() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("read production source");
        for forbidden in ["block_on(", "unbounded(", "Mutex", "RwLock"] {
            assert!(
                !source.contains(forbidden),
                "{} contains forbidden UI runtime primitive {forbidden}",
                file.display()
            );
        }
    }
}

#[test]
fn terminal_child_delivery_handles_disconnected_controllers() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in [src.join("pane_host.rs"), src.join("pane_host_terminals.rs")] {
        let source = fs::read_to_string(&file).expect("read pane terminal delivery source");
        assert!(
            !source.contains("terminal.emit("),
            "{} must handle a disconnected TerminalView sender without panicking",
            file.display()
        );
    }
}

#[test]
fn hosted_native_contracts_keep_selection_thread_and_geometry_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let live = fs::read_to_string(root.join("tests/application_live_view.rs")).unwrap();
    let live_test = &live[..live.find("#[derive(Clone, Copy)]").unwrap()];
    let selection = live_test
        .find("select_retained_connection(window.widget());")
        .expect("adaptive test must select through a fresh row lookup");
    let selected = live_test[selection..]
        .find("selected connection identity must settle before opening the editor")
        .expect("adaptive test must settle selection before opening the editor")
        + selection;
    let editor = live_test
        .find("ConnectionSidebarOutput::OpenCreate")
        .unwrap();
    assert!(selection < selected && selected < editor);
    let terminal_search = live_test
        .find("terminal_search.set_text(\"needle\");")
        .expect("adaptive test must establish terminal search before opening the editor");
    assert!(selection < terminal_search && terminal_search < editor);
    assert!(!live_test[editor..].contains("select_retained_connection"));
    assert!(!live_test[editor..].contains("canvas.grab_focus"));

    let draw = fs::read_to_string(root.join("tests/terminal_draw.rs")).unwrap();
    assert!(draw.contains("fn native_draw_contracts_run_serially_on_one_worker_thread()"));
    assert_eq!(draw.matches("#[test]").count(), 1);
    assert!(!draw.contains("Mutex"));

    let src = root.join("src");
    let main_window = fs::read_to_string(src.join("main_window.rs")).unwrap();
    assert!(main_window.contains("stable_sidebar_selection: Option<ConnectionId>"));
    let snapshots = fs::read_to_string(src.join("main_window_snapshots.rs")).unwrap();
    assert!(snapshots.contains("if let Some(connection) = self.stable_sidebar_selection"));
    assert!(!snapshots.contains("RefreshPresentation"));

    let probe = fs::read_to_string(src.join("startup_probe.rs")).unwrap();
    assert!(probe.contains("pub fn observe_terminal_geometry(&self, size: TerminalSize)"));
    assert!(probe.contains("self.observe_terminal_geometry(frame.size);"));
    let model = fs::read_to_string(src.join("pane_host_model.rs")).unwrap();
    assert!(model.contains("probe.observe_terminal_geometry(size);"));
    let pane_host = fs::read_to_string(src.join("pane_host.rs")).unwrap();
    assert!(pane_host.contains("SessionUiCommand::Resize(size)"));
    assert!(pane_host.contains("self.model.observe_terminal_geometry(*size);"));
    assert!(!pane_host.contains("TerminalViewMsg::RefreshGeometry"));
    let widgets = fs::read_to_string(src.join("terminal_view_widgets.rs")).unwrap();
    for required in [
        "canvas.add_tick_callback",
        "if !canvas.is_mapped()",
        "gtk::glib::ControlFlow::Continue",
        "gtk::glib::ControlFlow::Break",
        "initial_geometry_pending",
        "model.has_positive_emitted_geometry()",
        ".send(TerminalViewMsg::Resize",
        ".is_ok()",
    ] {
        assert!(
            widgets.contains(required),
            "missing geometry retry contract: {required}"
        );
    }
}

#[test]
fn product_icons_and_terminal_metrics_have_one_explicit_invalidation_path() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    for file in &files {
        let source = fs::read_to_string(file).expect("read production source");
        for forbidden in [".image()", ".button()", ".decode_texture()"] {
            assert!(
                !source.contains(forbidden),
                "{} contains implicit product icon call {forbidden}",
                file.display()
            );
        }
    }

    let icon_cache = fs::read_to_string(src.join("icon_cache.rs")).unwrap();
    assert!(icon_cache.contains("BTreeMap<IconCacheKey"));
    assert!(!icon_cache.contains("BTreeMap<ProductIcon"));
    let icon_render = fs::read_to_string(src.join("icon_render.rs")).unwrap();
    assert!(icon_render.contains("physical_size = request.physical_size()?"));
    assert!(icon_render.contains("connect_notify_local(Some(\"scale-factor\")"));

    let terminal_view = fs::read_to_string(src.join("terminal_view.rs")).unwrap();
    assert!(terminal_view.contains("FontMetricsService"));
    let metric_refresh = fs::read_to_string(src.join("terminal_view_metrics.rs")).unwrap();
    assert!(metric_refresh.contains("MetricsChange::Unchanged(_) => Ok(None)"));
    let pane_terminals = fs::read_to_string(src.join("pane_host_terminals.rs")).unwrap();
    assert!(pane_terminals.contains("TerminalViewMsg::UpdateProfile(profile.clone())"));
}

fn dependency_names(manifest: &str) -> Vec<&str> {
    let mut in_dependencies = false;
    let mut names = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && trimmed.starts_with('[') {
            break;
        }
        if in_dependencies
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && let Some((name, _)) = trimmed.split_once('=')
        {
            names.push(name.trim());
        }
    }
    names.sort_unstable();
    names
}

fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("read source directory") {
        let path = entry.expect("read source entry").path();
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn assert_typed_shell_ownership(path: &Path, source: &str) {
    for forbidden in [".unparent(", "WidgetExt::unparent"] {
        assert!(
            !source.contains(forbidden),
            "{} contains forbidden generic ownership escape {forbidden}",
            path.display()
        );
    }
}
