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
