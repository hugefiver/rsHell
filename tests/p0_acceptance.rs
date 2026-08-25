use std::collections::BTreeSet;

use std::process::Command;

const REQUIRED_REPORT_FIELDS: [&str; 11] = [
    "gtk",
    "local_terminal",
    "native_password",
    "native_key",
    "native_keyboard_interactive",
    "system_agent",
    "host_key",
    "vault",
    "imports",
    "tabs_splits",
    "cleanup",
];

const REQUIRED_ACTIONS: [&str; 25] = [
    "wait_window_realized",
    "new_tab",
    "open_connection_editor",
    "set_connection_field",
    "submit_connection",
    "select_connection",
    "connect",
    "respond_host_key",
    "respond_auth",
    "send_terminal_text",
    "paste_text_from_env",
    "resize_terminal",
    "wait_frame_contains",
    "split_horizontal",
    "split_vertical",
    "switch_tab",
    "search_terminal",
    "select_range",
    "copy_selection",
    "reconnect",
    "visual_checkpoint",
    "preview_import",
    "commit_import",
    "cancel_import",
    "close_all",
];

#[test]
fn static_p0_fixture_covers_each_exact_ui_action_name() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/smoke/p0-scenario.json")).unwrap();
    assert_eq!(fixture["version"], 1);
    let names = fixture["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|action| action["action"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(names, REQUIRED_ACTIONS.into_iter().collect());
}

#[test]
fn fixture_secret_fields_hold_environment_variable_names_not_values() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/smoke/p0-scenario.json")).unwrap();
    for action in fixture["actions"].as_array().unwrap() {
        let secret_field = action["action"] == "set_connection_field"
            && action["field"]["kind"] == "secret_from_env";
        let environment_action = matches!(
            action["action"].as_str(),
            Some("respond_auth" | "paste_text_from_env")
        );
        if secret_field || environment_action {
            let environment_name = if secret_field {
                &action["field"]["env_var"]
            } else {
                &action["env_var"]
            };
            assert!(environment_name.is_string());
            assert!(action["field"]["value"].is_null());
        }
    }
}

#[test]
fn p0_report_contract_has_fixed_fail_or_pass_surfaces() {
    assert_eq!(REQUIRED_REPORT_FIELDS.len(), 11);
    assert!(REQUIRED_REPORT_FIELDS.iter().all(|field| !field.is_empty()));
}

#[test]
fn powershell_harness_invokes_the_production_p0_driver() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");

    assert!(
        harness.contains("--smoke-p0"),
        "the QA entrypoint must invoke the production root smoke control plane"
    );
    assert!(
        harness.contains("try") && harness.contains("finally"),
        "the QA entrypoint must clean temporary resources even after failure"
    );
}

#[test]
fn visual_contract_uses_native_argb32_range_evidence_and_fatal_gtk_warnings() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let capture = include_str!("../crates/rshell-ui/src/main_window_smoke_capture.rs");
    let analyzer = include_str!("../crates/rshell-ui/src/visual_png.rs");
    for required in [
        "Assert-VisualContract",
        "dark_regions_passed",
        "focus_or_selection_thickness_px",
        "P0 PNG/report dimensions differ",
        "G_DEBUG = \"fatal-warnings\"",
    ] {
        assert!(
            harness.contains(required),
            "missing visual harness contract: {required}"
        );
    }
    assert!(capture.contains("argb32_native_to_rgba"));
    assert!(capture.contains("TextureExtManual"));
    assert!(analyzer.contains("NativeByteOrder"));
    assert!(analyzer.contains("analyze_rgba"));
    assert!(
        harness.find("Assert-VisualContract -Report").unwrap()
            < harness.find("Add-Phase \"gtk_production\"").unwrap(),
        "visual validation must fail before the GTK phase can pass"
    );
}

#[test]
fn embedded_product_assets_and_package_contract_are_closed() {
    let labels = rshell_ui::ProductIcon::ALL
        .into_iter()
        .map(|icon| icon.metadata().accessible_label)
        .collect::<BTreeSet<_>>();
    let assets = rshell_ui::ProductIcon::ALL
        .into_iter()
        .map(|icon| icon.metadata().svg)
        .collect::<BTreeSet<_>>();
    assert_eq!(labels.len(), 16);
    assert_eq!(assets.len(), 16);
    assert!(assets.iter().all(|svg| svg.starts_with(b"<svg")));

    let package = include_str!("../scripts/qa/assert-package.ps1");
    let workflow = include_str!("../scripts/qa/workflow-contract.ps1");
    for marker in [
        "embedded_css_loaded",
        "embedded_icons_renderable",
        "embedded_icon_backend",
        "Assert-NoProductAssetPayload",
        "external-icon-payload",
        "runtime-icon-backends",
    ] {
        assert!(package.contains(marker));
        assert!(workflow.contains(marker));
    }
    for probe in [
        "external-icon-payload",
        "external-resource-directory",
        "external-icons-directory",
        "runtime-icon-backends",
    ] {
        let output = Command::new("pwsh")
            .args([
                "-NoProfile",
                "-File",
                "scripts/qa/assert-package.ps1",
                "-RegressionProbe",
                probe,
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("PowerShell package probe");
        assert!(
            output.status.success(),
            "package probe failed: {probe}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn hosted_workflows_fail_closed_and_package_smoke_uses_a_deterministic_shell() {
    let ci = include_str!("../.github/workflows/ci.yml");
    let release = include_str!("../.github/workflows/release.yml");
    let package = include_str!("../scripts/qa/assert-package.ps1");
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let bootstrap = include_str!("../src/bootstrap.rs");

    for command in [
        "cargo check --workspace --all-targets --all-features --locked",
        "cargo test --workspace --all-features --locked",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
    ] {
        assert!(ci.contains(command), "CI is missing exact gate: {command}");
    }
    for workflow in [ci, release] {
        assert!(workflow.contains("$gvsbuildAttempts = 3"));
        assert!(
            workflow.contains("for ($attempt = 1; $attempt -le $gvsbuildAttempts; $attempt++)")
        );
        assert!(workflow.contains("gvsbuild GTK build failed after 3 attempts"));
    }
    assert!(ci.contains("$missingBaselineStatus -and $missingBaselineStartupMode"));
    assert!(ci.contains("$missingBaselineStatus -xor $missingBaselineStartupMode"));
    assert!(!ci.contains("$env:RSHELL_SHELL = $workspaceShell.Source"));
    assert!(!ci.contains("Run bounded SSH surface smoke"));
    assert!(!ci.contains("pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode Ssh"));
    assert!(!ci.contains(
        "cargo test --locked -p rshell-session --test ssh_smoke system_openssh_agent_authenticates_against_local_server -- --ignored --exact --nocapture"
    ));
    assert!(!harness.contains("$agentEnvironment.RSHELL_QA_SYSTEM_AGENT_PUBLIC_KEY_PATH"));
    assert!(bootstrap.contains("ShellOverride::deterministic()"));
    assert!(package.contains("Get-Command -Name \"pwsh\" -ErrorAction Stop"));
    assert!(package.contains("$startInfo.Environment[\"RSHELL_SHELL\"] = $pwsh.Source"));
    assert!(package.contains("$startupAttempts = 2"));
    assert!(package.contains("if ($timedOut -and $attempt -lt $startupAttempts)"));
}

#[test]
fn hosted_gui_tests_use_linux_xvfb_and_a_supported_macos_runner() {
    let ci = include_str!("../.github/workflows/ci.yml");
    let release = include_str!("../.github/workflows/release.yml");
    let manifest = include_str!("../Cargo.toml");
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");

    for workflow in [ci, release] {
        assert!(workflow.contains("os: macos-26"));
        assert!(!workflow.contains("gtk4-broadwayd"));
    }
    assert!(ci.contains("$env:DISPLAY = ':98'"));
    assert!(ci.contains("Xvfb"));
    assert!(manifest.contains("name = \"actor_panic_gtk_survival_macos\""));
    assert!(manifest.contains("path = \"tests/actor_panic_gtk_survival_macos.rs\""));
    assert!(manifest.contains("harness = false"));
    assert!(harness.contains("Invoke-MainThreadGtkRegression"));
}

#[test]
fn hosted_native_credentials_and_macos_gtk_evidence_are_fail_closed() {
    let ci = include_str!("../.github/workflows/ci.yml");
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");

    for marker in [
        "libsecret-tools",
        "gnome-keyring-daemon --unlock --components=secrets",
        "secret-tool store",
        "secret-tool lookup",
        "secret-tool clear",
        "XDG_DATA_HOME",
        "Upload failed P0 smoke artifacts",
        "actions/upload-artifact@v4",
        "hashFiles('artifacts/p0-smoke/**') != ''",
    ] {
        assert!(ci.contains(marker), "hosted CI is missing {marker}");
    }
    assert!(harness.contains("$baseEnvironment.GTK_A11Y = \"none\""));

    for source in [
        include_str!("../crates/rshell-ui/tests/application_live_view.rs"),
        include_str!("../crates/rshell-ui/tests/icon_registry.rs"),
        include_str!("../crates/rshell-ui/tests/native_visual_contract.rs"),
        include_str!("../crates/rshell-ui/tests/native_widgets.rs"),
        include_str!("../crates/rshell-ui/tests/startup.rs"),
        include_str!("../crates/rshell-ui/tests/task18_native_widgets.rs"),
    ] {
        assert!(
            source.contains("#![cfg(not(target_os = \"macos\"))]"),
            "standard libtest GTK entry points must not initialize GTK off the macOS main thread"
        );
    }
    assert!(ci.contains("Run temporary keychain vault probe and P0 All smoke (macOS)"));
}

#[test]
fn portable_runtime_path_regression_runs_only_on_windows() {
    let script = include_str!("../scripts/qa/p0-smoke.ps1");

    assert!(
        script.contains("if ($case.Name -eq \"portable_paths\" -and (-not $platformIsWindows))")
    );
    assert!(script.contains("continue"));
}

#[test]
fn local_shell_readiness_uses_real_io_instead_of_a_platform_prompt() {
    let script = include_str!("../scripts/qa/p0-smoke.ps1");
    let input = include_str!("../crates/rshell-ui/src/main_window_smoke_input.rs");

    assert!(script.contains("-Name \"shell-profile-path\""));
    assert!(script.contains("$PROFILE.CurrentUserCurrentHost"));
    assert!(script.contains("$guiEnvironment.XDG_CONFIG_HOME = $guiXdgConfig"));
    assert!(script.contains("PowerShell.OnIdle"));
    assert!(script.contains("-MaxTriggerCount 1"));
    assert!(!script.contains("$global:RshellP0Ready"));
    assert!(script.contains("[Console]::WriteLine('P0-LOCAL-READY')"));
    assert!(script.contains("function global:prompt { 'P0> ' }"));
    assert!(script.contains("text = \"P0-LOCAL-READY\""));
    assert!(!script.contains("send_terminal_text\"; text = \"Write-Output P0-LOCAL-READY"));
    assert!(!script.contains("-join [char[]](80,48,45,76,79,67,65,76,45,82,69,65,68,89)"));
    assert!(!script.contains("text = \"PS \""));
    assert!(!script.contains("text = \"PowerShell \""));
    assert!(!script.contains("Set-PSReadLineOption"));
    assert!(input.contains("split_smoke_terminal_submission"));
    assert!(input.contains("TerminalViewMsg::Key"));
    assert!(input.contains("gdk::Key::Return"));
}

#[test]
fn powershell_harness_has_a_cross_platform_tool_and_shell_contract() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");

    for forbidden in [
        "cargo.exe",
        "pwsh.exe",
        "C:\\gtk-build",
        "C:\\Windows\\System32\\OpenSSH",
        "$env:TEMP",
    ] {
        assert!(
            !harness.contains(forbidden),
            "the cross-platform harness must not hardcode {forbidden}"
        );
    }
    for required in [
        "$platformIsWindows",
        "$platformIsLinux",
        "$platformIsMacOS",
        "Get-Command -Name \"cargo\" -ErrorAction Stop",
        "Get-Command -Name \"pwsh\" -ErrorAction Stop",
        "Get-Command -Name \"ssh-keygen\" -ErrorAction Stop",
        "Get-Command -Name \"ssh-add\" -ErrorAction Stop",
        "RSHELL_SHELL",
        "$guiEnvironment.CARGO_HOME = $cargoHome",
        "$guiEnvironment.RUSTUP_HOME = $rustupHome",
        "[System.IO.Path]::GetTempPath()",
        "[System.IO.Path]::PathSeparator",
        "P0 smoke does not support this operating system.",
        "^ssh_smoke-[0-9a-f]+$",
    ] {
        assert!(
            harness.contains(required),
            "the cross-platform harness is missing {required}"
        );
    }
}

#[test]
fn harness_finalizes_artifacts_only_after_cleanup_and_secret_scan() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let final_junit = harness
        .rfind("Write-Junit")
        .expect("harness must write final JUnit");
    let cleanup = harness
        .rfind("P0 temp cleanup verification failed")
        .expect("harness must verify temp cleanup");
    let secret_scan = harness
        .rfind("assert-no-secrets")
        .expect("harness must run the final secret scan");
    assert!(
        final_junit > cleanup && final_junit > secret_scan,
        "JSON/JUnit finalization must occur after cleanup and secret scanning"
    );
    assert!(
        harness.contains("failures",)
            && !harness.contains("WriteAttributeString(\"failures\", \"0\")"),
        "JUnit failures must be derived instead of hardcoded"
    );
}

#[test]
fn completed_children_are_retired_before_pid_reuse_checks() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let retire = harness
        .find("$script:ownedChildIds.Remove($Run.Process.Id)")
        .expect("completed child identities must be retired");
    let dispose = harness
        .find("$Run.Process.Dispose()")
        .expect("completed child process handles must be disposed");
    let final_check = harness
        .find("foreach ($ownedId in $script:ownedChildIds)")
        .expect("uncompleted child identities must be checked at finalization");
    assert!(retire < dispose && dispose < final_check);
}

#[test]
fn harness_owns_agent_and_vault_cleanup_ledgers_before_children_start() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    for marker in [
        "agent-cleanup-ledger",
        "vault-cleanup-ledger",
        "fail_before_fixture_ready",
        "fail_during_vault_probe",
    ] {
        assert!(
            harness.contains(marker),
            "parent cleanup contract is missing {marker}"
        );
    }
}

#[test]
fn agent_cleanup_is_required_before_add_and_survives_a_lost_reply() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let required = harness
        .find("$agentCleanupRequired = $true")
        .expect("agent cleanup must become required durably");
    let add = harness
        .find("-Name \"agent-add-parent\"")
        .expect("agent add phase");
    assert!(required < add, "cleanup obligation must precede ssh-add");
    let lost_reply = harness
        .find("agent_add_lost_reply")
        .expect("lost-reply mutation probe");
    let cleanup = harness
        .find("agent_add_lost_reply_cleanup")
        .expect("lost-reply mutation cleanup");
    assert!(required < add && add < lost_reply && lost_reply < cleanup);
    assert!(
        harness.contains("The lost-reply agent key was not removed before completing cleanup.")
    );
    assert!(!harness.contains("$agentAddedByLostReply"));
    assert!(
        !harness.contains("$agentAdded -and"),
        "cleanup cannot depend on observing ssh-add success"
    );
}

#[test]
fn fixture_shutdown_exit_is_checked_in_normal_and_finally_paths() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    assert!(harness.contains("fixture_nonzero_shutdown"));
    assert!(harness.contains("$fixtureStopExit = Complete-CapturedChild"));
    assert!(harness.contains("$fixtureFinallyExit = Complete-CapturedChild"));
    assert!(harness.contains("fixture final assertions failed"));
}

#[test]
fn late_failure_invalidates_security_surfaces_and_has_a_real_junit_node() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    assert!(harness.contains("Set-LateFailure"));
    assert!(harness.contains("harness_finalization"));
    assert!(harness.contains("late_cleanup_or_security_failure"));
    assert!(harness.contains("RSHELL_QA_INJECT_LATE_FINALIZATION_FAILURE"));
    assert!(harness.contains("failureNodes.Count"));
}

#[test]
fn mode_all_declares_every_task20_regression_case() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    for case in [
        "option_like_host",
        "ports_0_65536",
        "resize_1x1_999x999",
        "wide_midpoint",
        "backpressure_10k",
        "unknown_host_reject",
        "changed_host_key",
        "wrong_password",
        "kbi_cancel",
        "vault_result_unknown",
        "database_finalize_failure",
        "backup_recovery",
        "openssh_wildcard_include_cycle",
        "repeated_shutdown_reconnect",
        "argv_injection",
        "secret_unchanged_clear",
        "actor_panic_gtk_survival",
        "eof_clean_exit",
        "latest_frame_wins",
        "portable_paths",
        "release_no_legacy_dependencies",
    ] {
        assert!(
            harness.contains(case),
            "Mode All is missing regression {case}"
        );
    }
}

#[test]
fn powershell_tui_exit_waits_for_a_fresh_post_alternate_screen_frame() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    let tui_exit_wait = harness
        .find("P0-TUI-EXITED")
        .expect("Mode All must wait for the TUI exit marker");
    let post_exit_marker = "P0-TUI-POST-EXIT-FRAME";
    let post_exit_tail = &harness[tui_exit_wait..];
    let post_exit_send = post_exit_tail
        .find("Write-Output P0-TUI-POST-EXIT-FRAME")
        .expect("the PowerShell scenario must request a new post-TUI terminal frame");
    assert!(
        post_exit_tail[post_exit_send..].contains(&format!("text = \"{post_exit_marker}\"")),
        "the PowerShell scenario must observe the new post-TUI terminal frame before changing panes"
    );
}

#[test]
fn regression_cases_are_exact_commands_not_labels_after_an_aggregate_suite() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    assert!(harness.contains("Invoke-RegressionCase"));
    assert!(harness.contains("-Arguments $arguments"));
    assert!(!harness.contains("-Name \"task20-regression-matrix\""));
    assert!(harness.contains("resize_extremes_emit_real_1x1_and_999x999_commands"));
    assert!(harness.contains("actor_panic_keeps_realized_main_window_alive"));
    assert!(
        harness.contains(
            "wide_midpoint_selection_normalizes_to_the_stable_wide_cell_and_frame_overlay"
        )
    );
    assert!(
        harness.contains("stale_and_equal_frames_are_dropped_and_dirty_rows_track_stable_content")
    );
    assert!(!harness.contains("cursor_width_uses_the_wide_cell_under_the_cursor"));
    assert!(
        !harness
            .contains("session_binding_forwards_events_latest_frame_and_interaction_to_same_actor")
    );
}

#[test]
fn mode_all_builds_and_inspects_the_release_dependency_surface() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    assert!(
        harness.contains("\"build\", \"--release\"")
            && harness.contains("\"tree\", \"--locked\"")
            && harness.contains("release dependency scan found a removed dependency"),
        "release_no_legacy_dependencies must be backed by a real release build and dependency scan"
    );
}

#[test]
fn regression_harness_rejects_zero_multiple_and_failed_exact_test_results() {
    let harness = include_str!("../scripts/qa/p0-smoke.ps1");
    assert!(harness.contains("if ($RegressionCaseProbe) {"));
    assert!(harness.contains("[Console]::Error.WriteLine($_.Exception.Message)"));
    for probe in ["zero", "one", "multiple", "failure"] {
        let output = invoke_harness_probe("-RegressionParserProbe", probe);
        assert!(
            output.status.success(),
            "regression parser probe {probe} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = invoke_harness_probe("-RegressionCaseProbe", "p0_task20_missing_exact_test");
    assert!(
        !output.status.success(),
        "a missing exact test must fail the harness instead of adding a passed phase"
    );
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.contains(
            "P0 regression exact-test discovery did not yield exactly one matching test."
        ),
        "the harness must fail specifically because discovery found zero exact tests: {output}"
    );
}

fn invoke_harness_probe(argument: &str, value: &str) -> std::process::Output {
    Command::new("pwsh")
        .args([
            "-NoProfile",
            "-File",
            "scripts/qa/p0-smoke.ps1",
            "-Mode",
            "Unit",
            argument,
            value,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("PowerShell must launch the Task20 smoke harness")
}

#[test]
fn task20_root_production_modules_remain_focused() {
    for (path, source) in [
        ("src/bootstrap.rs", include_str!("../src/bootstrap.rs")),
        ("src/cleanup.rs", include_str!("../src/cleanup.rs")),
        ("src/main.rs", include_str!("../src/main.rs")),
        ("src/p0_smoke.rs", include_str!("../src/p0_smoke.rs")),
        (
            "src/p0_smoke_action_fields.rs",
            include_str!("../src/p0_smoke_action_fields.rs"),
        ),
        (
            "src/p0_smoke_actions.rs",
            include_str!("../src/p0_smoke_actions.rs"),
        ),
        (
            "src/p0_smoke_cleanup.rs",
            include_str!("../src/p0_smoke_cleanup.rs"),
        ),
        (
            "src/p0_smoke_contract.rs",
            include_str!("../src/p0_smoke_contract.rs"),
        ),
        (
            "src/p0_smoke_contract_binding.rs",
            include_str!("../src/p0_smoke_contract_binding.rs"),
        ),
        (
            "src/p0_smoke_contract_evidence.rs",
            include_str!("../src/p0_smoke_contract_evidence.rs"),
        ),
        (
            "src/p0_smoke_evidence.rs",
            include_str!("../src/p0_smoke_evidence.rs"),
        ),
        (
            "src/p0_smoke_report.rs",
            include_str!("../src/p0_smoke_report.rs"),
        ),
        (
            "src/p0_smoke_report_steps.rs",
            include_str!("../src/p0_smoke_report_steps.rs"),
        ),
        (
            "src/p0_smoke_report_terminal.rs",
            include_str!("../src/p0_smoke_report_terminal.rs"),
        ),
        (
            "src/p0_smoke_runtime.rs",
            include_str!("../src/p0_smoke_runtime.rs"),
        ),
        (
            "src/p0_smoke_scenario.rs",
            include_str!("../src/p0_smoke_scenario.rs"),
        ),
        (
            "src/p0_smoke_status.rs",
            include_str!("../src/p0_smoke_status.rs"),
        ),
    ] {
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        let pure_lines = production
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with("//")
            })
            .count();
        assert!(
            pure_lines <= 250,
            "{path} has {pure_lines} pure production lines (limit 250)"
        );
    }
}

#[test]
fn ci_powershell_here_string_remains_inside_the_yaml_run_block() {
    let lines = include_str!("../.github/workflows/ci.yml")
        .lines()
        .collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|line| line.trim() == "$inner = @'")
        .expect("Linux vault smoke must define its inner PowerShell script");
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == "'@")
        .map(|offset| start + 1 + offset)
        .expect("inner PowerShell here-string must terminate");
    assert!(end > start + 1, "inner PowerShell script must not be empty");
    for (index, line) in lines[start + 1..=end].iter().enumerate() {
        assert!(
            line.starts_with("          "),
            "ci.yml line {} escapes the run block scalar: {line:?}",
            start + index + 2
        );
    }
}
