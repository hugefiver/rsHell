# P0 Final-Review Corrections Implementation Plan

> **For agentic workers:** Use the subagent-driven-development skill to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Correct every verified P0 final-review blocker at base `61c0fb4dff250cdd698b22d3fd3b5474862b1074` and finish with fail-closed local, hosted, package, and same-identity acceptance evidence.

**Architecture:** Keep core policy, session runtime, platform ownership, storage lifecycle, and GTK presentation in their existing crates and communicate only through the existing typed ports. Introduce narrowly owned helpers for module sizing, one-shot authentication, import cleanup, Windows process trees, actor presentation, terminal input policy, report artifact names, and the terminal-engine gate; each helper has bounded lifetime and explicit failure semantics. Verification is layered from targeted RED/GREEN tests to workspace gates, real P0 smoke/package runs, three-platform hosted workflows, and two orchestrator-owned final reviews bound to one immutable committed range and artifact manifest.

**Tech Stack:** Rust 2024, Tokio, GTK4/Relm4, `portable-pty-psmux`, `windows-sys 0.61`, pinned `wezterm-term` revision `d69264df66fdcc928c7a30c673df108984fda821`, PowerShell 7, GitHub Actions, SHA-256.

**Spec:**
- `docs/superpowers/specs/2026-08-01-rshell-rebuild-design.md`
- `docs/superpowers/plans/2026-08-01-rshell-p0-rebuild.md`
- `docs/superpowers/specs/2026-08-24-fluent-task21-design.md`
- `docs/superpowers/plans/2026-08-24-fluent-task21.md`

**Global Constraints:**
- Base the corrective range on exact commit `61c0fb4dff250cdd698b22d3fd3b5474862b1074`.
- Never place secret bytes in DB/WAL/SHM, logs, `Debug`, reports, screenshots, argv, or fixtures.
- Use bounded channels, bounded waits, owned cancellation, and cleanup that reports rather than hides failure.
- Preserve core/UI/infrastructure boundaries and the sole UI egress `MainWindow::dispatch -> command_port::dispatch -> UiCommandPort::try_send`.
- Do not add P1 behavior.
- Every product Rust production module must remain at or below 250 pure production LOC, enforced by tests; narrowly vendored third-party source is governed by its pinned provenance and patch boundary instead.
- Do not add `libssh2`, OpenSSL, an old WezTerm revision, or a second terminal runtime.
- Preserve terminal defaults: `xterm-256color`, 120x36, 6000 scrollback, left/right Alt-as-meta enabled, CSI-u disabled, Kitty keyboard disabled, mouse reporting enabled, scroll-on-output enabled, scroll-on-keypress disabled.
- Do not weaken the 40 MiB/s, 16 ms, 100 MiB, five-sample, or SHA-256 terminal-engine criteria.
- Use PowerShell commands only and install no software.
- Git writes and final reviewer dispatch belong to the orchestrator under separate user authorization; this plan contains no commit or push command.
- Do not claim hosted evidence until the matching workflow run has completed successfully for the frozen review SHA.

---

## File Map

### Production code and tests

- `tests/production_module_limits.rs`: recursive workspace-wide pure-production-LOC contract.
- `crates/rshell-core/src/application/mod.rs`: register focused session command/event modules.
- `crates/rshell-core/src/application/sessions.rs`: remove after its responsibilities are split.
- `crates/rshell-core/src/application/session_commands.rs`: session command routing and pane/tab close behavior.
- `crates/rshell-core/src/application/session_events.rs`: session event forwarding, shutdown completion, and session error projection.
- `crates/rshell-session/src/actor.rs`: retain actor lifecycle and event loop only.
- `crates/rshell-session/src/actor_process.rs`: direct-child registry bookkeeping used by the actor.
- `crates/rshell-session/src/lib.rs`: register/re-export new session modules and interfaces.
- `crates/rshell-session/src/transport/local_runtime.rs`: PTY event/write/resize state and focused lifecycle delegation.
- `crates/rshell-session/src/transport/local_runtime/lifecycle.rs`: bounded shutdown, drop, reader join, and process-tree convergence.
- `crates/rshell-session/src/transport/mod.rs`: register the local-runtime lifecycle child module through its parent.
- `crates/rshell-session/src/auth.rs`: directly owned secret-bearing `AuthPlan` variants with no clonable shared secret.
- `crates/rshell-session/src/native_factory.rs`: one-shot native transport factory.
- `crates/rshell-session/src/ports.rs`: construct the one-shot factory while preserving application retry flow.
- `crates/rshell-session/src/transport/native_ssh.rs` and its focused child modules: consume authentication before a network attempt can be repeated.
- `crates/rshell-session/tests/auth.rs`, `ports.rs`, `native_ssh.rs`, `actor_lifecycle.rs`: no-retention, no-network reconnect, and actor behavior regressions.
- `crates/rshell-core/tests/application.rs`, `session_workflows.rs`: fresh-vault-read `RetryPane` regression.
- `crates/rshell-ui/tests/workspace_view_model.rs`: user reconnect/retry remains application `RetryPane`.
- `crates/rshell-storage/src/ports.rs`: export import cleanup ownership.
- `crates/rshell-storage/src/ports/imports.rs`: Tokio-clock preview creation and deterministic expiry.
- `crates/rshell-storage/src/ports/import_cleanup.rs`: 60-second weak-reference cleanup loop, cancellation, bounded shutdown, and drop fallback.
- `crates/rshell-storage/Cargo.toml`: Tokio `sync`, `time`, and test-time features.
- `crates/rshell-storage/tests/ports.rs`: paused-time TTL, cancellation, weak ownership, and no-vault-write tests.
- `src/bootstrap.rs`: own `ImportPreviewCleanup` for the process lifetime.
- `src/cleanup.rs`: stop cleanup before application/session/storage teardown and propagate failure.
- `crates/rshell-core/src/application/imports.rs`: remove expired core preview authority and publish the reconciled view.
- `crates/rshell-ui/tests/import_view_model.rs`: expired UI state regression.
- `Cargo.toml`: patch crates.io `portable-pty-psmux` to the narrowly vendored 0.9.6 source.
- `Cargo.lock`: bind the workspace to the patched path package and remove the registry checksum/source for that selected package.
- `third_party/portable-pty-psmux/Cargo.toml`, `LICENSE.md`, `README.md`, `examples/{bash,narrow,whoami,whoami_async}.rs`, `src/{cmdbuilder,serial,unix}.rs`, and `src/win/mod.rs`: preserve the exact unchanged 0.9.6 package/license baseline.
- `third_party/portable-pty-psmux/README.rshell-patch.md`: record upstream package/version/checksum and the creation-time Job-list patch boundary.
- `third_party/portable-pty-psmux/src/lib.rs`: add the Windows-only `SlavePty::spawn_command_in_job` borrowed-handle API.
- `third_party/portable-pty-psmux/src/win/procthreadattr.rs`: own the Job handle array in stable heap-backed storage and add `PROC_THREAD_ATTRIBUTE_JOB_LIST` to the existing STARTUPINFOEX list.
- `third_party/portable-pty-psmux/src/win/psuedocon.rs`: keep the attribute-list/storage owner alive until `CreateProcessW` returns, then destroy the list before releasing its Job-handle storage.
- `third_party/portable-pty-psmux/src/win/conpty.rs`: preserve the job attribute through passthrough fallback.
- `crates/rshell-platform/Cargo.toml`: enable the explicit Windows Job Object API feature.
- `crates/rshell-platform/src/lib.rs`: expose opaque Windows process-tree ownership.
- `crates/rshell-platform/src/process_tree.rs`: platform-facing Windows Job Object interface.
- `crates/rshell-platform/src/process_tree/windows.rs`: create/configure KILL_ON_JOB_CLOSE before spawn, expose a borrowed handle, inspect membership, terminate, close, and provide RAII.
- `crates/rshell-session/src/transport/pty.rs`: create the per-session job before spawn and call the patched creation-time PTY API.
- `crates/rshell-session/src/transport/local.rs`: expose exact-Job membership evidence for the concrete local transport on Windows.
- `crates/rshell-session/tests/local_pty.rs`: prove the app is outside the per-session job while an immediate descendant is inside and dies on bounded shutdown.
- `src/p0_smoke_cleanup.rs`, `src/cleanup.rs`: label the PID registry as direct-child evidence rather than process-tree proof.
- `crates/rshell-session/src/presentation.rs`: actor-owned viewport bounds, follow-bottom state, policy, and monotonic frame generation.
- `crates/rshell-session/src/engine.rs`: viewport-bounds and input interfaces implemented by the sole engine.
- `crates/rshell-session/src/render.rs`: render engine content without assigning publication identity.
- `crates/rshell-session/src/actor_io.rs`: apply presentation policy and publish stamped frames.
- `crates/rshell-session/src/message.rs`: carry `PresentationPolicy` in `SessionLaunch`.
- `crates/rshell-session/tests/support/mod.rs`: fixed-backend-generation fake and viewport bounds.
- `crates/rshell-session/tests/engine_contract.rs`, `actor_lifecycle.rs`: fixed-seqno, real long-output, follow-bottom, selection, scroll, and input-policy tests.
- `crates/rshell-ui/tests/terminal_view_model.rs`: prove fresh actor generations are accepted by the UI.
- `crates/rshell-session/src/wezterm_adapter.rs`: compile-proven WezTerm key/mouse encoding, negotiated terminal modes, and configured mouse gate.
- `crates/rshell-session/src/input.rs`: committed-text validation and core-to-WezTerm mapping only; active key and mouse encoding delegates to WezTerm.
- `crates/rshell-core/src/terminal/key_action.rs`: closed P0 key-action parser.
- `crates/rshell-core/src/terminal.rs`, `terminal/validation.rs`: export and validate typed key actions.
- `crates/rshell-core/src/protocol/commands.rs`: session-local clear-scrollback command.
- `crates/rshell-ui/src/terminal_view_message.rs`: carry pane/profile and physical Alt/focus events.
- `crates/rshell-ui/src/terminal_input.rs`: map GTK keys using physical Alt-side policy.
- `crates/rshell-ui/src/terminal_view_model.rs`: retain profile, resolve bindings, and emit typed commands.
- `crates/rshell-ui/src/terminal_view.rs`, `terminal_view_widgets.rs`: own Alt-side state and key-release/focus reset wiring.
- `crates/rshell-ui/src/pane_host_terminals.rs`, `pane_view_model.rs`: pass pane identity and merge global/profile bindings deterministically.
- `crates/rshell-core/tests/terminal_profiles.rs`, `crates/rshell-ui/tests/terminal_input.rs`, `terminal_view_model.rs`, `workspace_view_model.rs`: action validation, exact bytes, side-specific Alt, mouse, and sole-egress behavior.
- `src/p0_smoke_report.rs`: stable artifact-name serialization.
- `scripts/qa/p0-smoke.ps1`: preserve absolute I/O paths while finalizing report fields with leaf names only.
- `tests/p0_acceptance.rs`: report privacy, process evidence wording, workflow, and harness contracts.

### Engine gate, workflows, and documentation

- `crates/rshell-session/benches/throughput.rs`: remove the obsolete non-contract benchmark.
- `crates/rshell-session/benches/terminal_engine.rs`: optimized fail-closed correctness/performance executable.
- `crates/rshell-session/tests/fixtures/vt/canary.json`: exact deterministic trace and expected hash contract.
- `crates/rshell-session/TERMINAL_ENGINE.md`: measured five-sample GO/NO-GO decision for the sole adapter.
- `crates/rshell-session/Cargo.toml`: `terminal_engine` bench and `sha2 = "0.10.9"` dev dependency.
- `scripts/qa/terminal-engine-gate.ps1`: execute and validate the machine-stable gate protocol.
- `scripts/qa/workflow-contract.ps1`: require the engine gate and three-platform fail-closed wiring.
- `.github/workflows/ci.yml`: execute the gate in each Linux/macOS/Windows CI matrix job.
- `.github/workflows/release.yml`: keep release builds and package startup validation deterministic; performance measurement remains a CI-only gate.
- `README.md`: current local P0 verification commands and honest process-tree evidence semantics.
- `docs/superpowers/plans/2026-08-01-rshell-p0-rebuild.md`: update only Task22 commands/evidence affected by these corrections.

---

### Task 1: Enforce the global module cap and split the three violations

**Files:**
- Create: `tests/production_module_limits.rs`
- Create: `crates/rshell-core/src/application/session_commands.rs`
- Create: `crates/rshell-core/src/application/session_events.rs`
- Modify: `crates/rshell-core/src/application/mod.rs`
- Remove: `crates/rshell-core/src/application/sessions.rs`
- Create: `crates/rshell-session/src/actor_process.rs`
- Modify: `crates/rshell-session/src/actor.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Create: `crates/rshell-session/src/transport/local_runtime/lifecycle.rs`
- Modify: `crates/rshell-session/src/transport/local_runtime.rs`

**Interfaces:**
- Consumes: existing `CommandLoop` session methods, `SessionActor::{record_child_process, clear_stopped_child_process}`, and `join_reader_thread(&mut mpsc::Receiver<ReaderEvent>, JoinHandle<()>, Duration) -> Result<(), TransportError>` behavior at base HEAD.
- Produces: unchanged call-site behavior; `actor_process::{record_child_process(SessionId, &ChildProcessRegistry, Option<u32>), clear_stopped_child_process(SessionId, &ChildProcessRegistry) -> Result<(), SessionError>}`; preserved `local_runtime::join_reader_thread(&mut mpsc::Receiver<ReaderEvent>, JoinHandle<()>, Duration) -> Result<(), TransportError>`; one recursive `all_production_modules_stay_within_pure_loc_cap` contract.

- [ ] **Step 1: Write the recursive failing contract test**

  Traverse the root `src/` and every `crates/*/src/` directory without a manual allowlist. Use the repository's established definition: stop at the first inline `#[cfg(test)]`, then count nonblank lines that do not begin with `//` after trimming.

  ```rust
  use std::path::{Path, PathBuf};

  const PURE_PRODUCTION_LOC_LIMIT: usize = 250;

  fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
      let mut entries = std::fs::read_dir(directory)
          .expect("read production source directory")
          .collect::<Result<Vec<_>, _>>()
          .expect("collect production source entries");
      entries.sort_by_key(|entry| entry.path());
      for entry in entries {
          let path = entry.path();
          if path.is_dir() {
              collect_rust_files(&path, files);
          } else if path.extension().is_some_and(|extension| extension == "rs") {
              files.push(path);
          }
      }
  }

  fn workspace_rust_sources(root: &Path) -> Vec<PathBuf> {
      let mut files = Vec::new();
      collect_rust_files(&root.join("src"), &mut files);
      let mut crates = std::fs::read_dir(root.join("crates"))
          .expect("read workspace crates")
          .collect::<Result<Vec<_>, _>>()
          .expect("collect workspace crates");
      crates.sort_by_key(|entry| entry.path());
      for entry in crates {
          let source = entry.path().join("src");
          if source.is_dir() {
              collect_rust_files(&source, &mut files);
          }
      }
      files
  }

  fn pure_production_loc(source: &str) -> usize {
      source
          .split("#[cfg(test)]")
          .next()
          .unwrap_or(source)
          .lines()
          .filter(|line| {
              let line = line.trim();
              !line.is_empty() && !line.starts_with("//")
          })
          .count()
  }

  #[test]
  fn all_production_modules_stay_within_pure_loc_cap() {
      let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
      let files = workspace_rust_sources(root);
      assert!(!files.is_empty(), "workspace production source discovery returned zero files");
      let mut violations = Vec::new();
      for file in files {
          let source = std::fs::read_to_string(&file).expect("read production Rust source");
          let count = pure_production_loc(&source);
          if count > PURE_PRODUCTION_LOC_LIMIT {
              violations.push(format!(
                  "{} has {count} pure production lines (limit {PURE_PRODUCTION_LOC_LIMIT})",
                  file.strip_prefix(root).unwrap_or(&file).display()
              ));
          }
      }
      assert!(violations.is_empty(), "{}", violations.join("\n"));
  }
  ```

- [ ] **Step 2: Run RED and verify the exact known violations**

  Run:

  ```powershell
  cargo test --locked -p rshell --test production_module_limits -- --exact all_production_modules_stay_within_pure_loc_cap --nocapture
  ```

  Expected: FAIL and identify `crates/rshell-core/src/application/sessions.rs` at 269, `crates/rshell-session/src/actor.rs` at 255, and `crates/rshell-session/src/transport/local_runtime.rs` at 300 pure production lines. Discovery returning zero files is also a failure.

- [ ] **Step 3: Split by existing ownership without changing public behavior**

  Move command/respond/close pane/close tab methods into `session_commands.rs`; move forward/finish-shutdown/error helpers into `session_events.rs`. Replace `mod sessions;` with both focused modules. Move only direct-child registry operations out of `actor.rs`:

  ```rust
  pub(crate) fn record_child_process(
      id: SessionId,
      registry: &ChildProcessRegistry,
      process_id: Option<u32>,
  );

  pub(crate) fn clear_stopped_child_process(
      id: SessionId,
      registry: &ChildProcessRegistry,
  ) -> Result<(), SessionError>;
  ```

  Put shutdown/drop/reader-join implementation in the child module `local_runtime/lifecycle.rs`. Re-export `join_reader_thread` from `local_runtime.rs` so `transport/local_runtime_tests.rs` keeps its existing import. Do not alter lock ordering, actor reconnect ordering, PTY deadlines, error categories, or command-port routing.

- [ ] **Step 4: Run targeted GREEN**

  Run:

  ```powershell
  cargo test --locked -p rshell --test production_module_limits -- --exact all_production_modules_stay_within_pure_loc_cap --nocapture
  cargo test --locked -p rshell-core --test application
  cargo test --locked -p rshell-session --test actor_lifecycle
  cargo test --locked -p rshell-session transport::local_runtime_tests
  ```

  Expected: all commands PASS; the cap test scans root/core/platform/session/storage/UI production modules recursively and reports no file above 250.

- [ ] **Step 5: Integration check and boundary**

  Run `cargo check --workspace --all-targets --all-features --locked`. Expected: PASS with no changed public application/session behavior. This is an orchestrator-owned review boundary for “module contract and responsibility-only splits”; no Git write is performed by this plan.

---

### Task 2: Make native authentication one-shot and keep user retry fresh

**Files:**
- Create: `crates/rshell-session/src/native_factory.rs`
- Modify: `crates/rshell-session/src/auth.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/ports.rs`
- Test inline: `crates/rshell-session/src/native_factory.rs` (`#[cfg(test)]` unit module)
- Modify: `crates/rshell-session/src/transport/native_ssh.rs`
- Test: `crates/rshell-session/tests/auth.rs`
- Test: `crates/rshell-session/tests/ports.rs`
- Test: `crates/rshell-session/tests/native_ssh.rs`
- Test: `crates/rshell-session/tests/actor_lifecycle.rs`
- Test: `crates/rshell-core/tests/application.rs`
- Test: `crates/rshell-core/tests/session_workflows.rs`
- Test: `crates/rshell-ui/tests/workspace_view_model.rs`

**Interfaces:**
- Consumes: `AuthPlan::from_secret`, `TransportFactory::create(&TransportRequest)`, `CommandLoop::retry_pane -> relaunch -> prepare -> launch_secret`, and `CredentialPort::get`.
- Produces: secret variants containing owned `SecretString`; no `AuthPlan::duplicate`; `NativeFactory::new(ConnectionProfile, AuthPlan, KnownHostsVerifier) -> NativeFactory`; one successful `NativeFactory::create` followed by pre-network `SessionFailure::Authentication`; unchanged user-visible `PaneAction::{Reconnect, Retry} -> UiCommand::RetryPane` fresh-read route.

- [ ] **Step 1: Add RED tests for ownership, no-network reconnect, and fresh retry**

  Add these independent assertions:

  ```rust
  // Session factory ownership contract.
  let factory = NativeFactory::new(profile, auth, verifier);
  assert!(factory.has_pending_auth_for_test());
  let first = factory.create(&request).expect("initial native transport");
  assert!(!factory.has_pending_auth_for_test());
  assert_eq!(
      factory.create(&request).unwrap_err().failure(),
      SessionFailure::Authentication
  );
  drop(first);
  ```

  ```rust
  // Core retry contract; values are generated in memory and never logged or serialized.
  let first = SecretString::from(Uuid::new_v4().to_string());
  let second = SecretString::from(Uuid::new_v4().to_string());
  credential_port.set_secret(first);
  launch_native_pane(&app, pane).await;
  credential_port.set_secret(second);
  app.ui_port().try_send(UiCommand::RetryPane(pane)).unwrap();
  assert_eq!(credential_port.get_count(), 2);
  assert!(session_port.second_launch_received_replacement_secret());
  ```

  The native reconnect fixture must generate its credential at runtime, accept/authenticate the initial connection, count exactly one TCP accept, send actor-internal `SessionCommand::Reconnect`, observe `SessionFailure::Authentication`, and still count one accept. The source-level ownership regression in external `tests/auth.rs` must assert production `auth.rs` contains neither the shared-secret type spelling nor the duplication method spelling, so the assertion strings do not make the production-source check self-matching.

- [ ] **Step 2: Run RED and verify all three gaps**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test auth -- --nocapture
  cargo test --locked -p rshell-session --test ports -- --nocapture
  cargo test --locked -p rshell-session --test native_ssh native_reconnect_exhausts_auth_before_second_network_attempt -- --exact --nocapture
  cargo test --locked -p rshell-core --test application retry_pane_reads_fresh_native_credential -- --exact --nocapture
  cargo test --locked -p rshell-ui --test workspace_view_model reconnect_and_retry_use_application_retry_pane -- --exact --nocapture
  ```

  Expected: FAIL because `AuthPlan` shares `Arc<SecretString>`, `NativeFactory` duplicates it, actor reconnect can construct another network transport, and the new fresh-read assertions do not yet exist.

- [ ] **Step 3: Implement direct secret ownership and the one-shot factory**

  Use direct fields and an atomically consumed factory slot:

  ```rust
  pub enum AuthPlan {
      Password { host: String, password: SecretString },
      PublicKey {
          host: String,
          identity_file: PathBuf,
          passphrase: Option<SecretString>,
      },
      Agent { host: String },
      KeyboardInteractive { host: String },
  }

  pub(crate) struct NativeFactory {
      profile: ConnectionProfile,
      auth: Mutex<Option<AuthPlan>>,
      verifier: KnownHostsVerifier,
  }

  impl TransportFactory for NativeFactory {
      fn create(&self, _request: &TransportRequest)
          -> Result<Box<dyn SessionTransport>, TransportError>
      {
          let auth = self.auth.lock()
              .unwrap_or_else(|error| error.into_inner())
              .take()
              .ok_or_else(|| TransportError::new(SessionFailure::Authentication))?;
          NativeSshTransport::new(self.profile.clone(), auth, self.verifier.clone())
              .map(|transport| Box::new(transport) as Box<dyn SessionTransport>)
      }
  }
  ```

  Consume the transport's `Option<AuthPlan>` before opening TCP so a repeated connect call also fails before network. Keep `Debug` redacted. Drop the local authentication plan immediately after the native library has accepted its borrowed secret. Do not copy secret bytes into fixture state, counters, panic text, or error values.

- [ ] **Step 4: Preserve the application-owned fresh credential path**

  Keep both `PaneAction::Reconnect` and failed-pane `PaneAction::Retry` mapped to `UiCommand::RetryPane`. Confirm the route creates a new actor and performs `CredentialPort::get` during `prepare`; do not redirect user reconnect to `SessionUiCommand::Reconnect`. Keep actor-internal reconnect as a defensive protocol path whose one-shot native factory fails closed.

- [ ] **Step 5: Run targeted GREEN and integration checks**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test auth
  cargo test --locked -p rshell-session --test ports
  cargo test --locked -p rshell-session --test native_ssh
  cargo test --locked -p rshell-session --test actor_lifecycle
  cargo test --locked -p rshell-core --test application
  cargo test --locked -p rshell-core --test session_workflows
  cargo test --locked -p rshell-ui --test workspace_view_model
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; the network accept counter remains one on actor reconnect, the factory retains no auth after first create, retry reads the vault twice, and all secret-bearing `Debug` checks remain redacted. This is the orchestrator-owned “native auth lifetime” review boundary.

---

### Task 3: Own and cancel the 60-second import-preview cleanup loop

**Files:**
- Create: `crates/rshell-storage/src/ports/import_cleanup.rs`
- Modify: `crates/rshell-storage/src/ports.rs`
- Modify: `crates/rshell-storage/src/ports/imports.rs`
- Modify: `crates/rshell-storage/Cargo.toml`
- Test: `crates/rshell-storage/tests/ports.rs`
- Modify: `src/bootstrap.rs`
- Modify: `src/cleanup.rs`
- Modify: `crates/rshell-core/src/application/imports.rs`
- Test: `crates/rshell-core/tests/application.rs`
- Test: `crates/rshell-ui/tests/import_view_model.rs`

**Interfaces:**
- Consumes: `Arc<ImportPortAdapter>`, `ImportPortAdapter::cleanup_expired`, `PREVIEW_TTL = 15 minutes`, `BootstrappedApplication::shutdown`, and `ImportError::PreviewExpired`.
- Produces: `ImportPreviewCleanup::start(&Arc<ImportPortAdapter>) -> ImportPreviewCleanup`, `ImportPreviewCleanup::start_with_interval(&Arc<ImportPortAdapter>, Duration) -> Result<ImportPreviewCleanup, ImportCleanupError>`, `ImportPreviewCleanup::shutdown(self) -> Result<(), ImportCleanupError>`, and core removal/publication of expired preview IDs.

- [ ] **Step 1: Add paused-time RED tests**

  Use Tokio paused time and the production 15-minute TTL:

  ```rust
  #[tokio::test(start_paused = true)]
  async fn periodic_cleanup_expires_secret_preview_without_another_port_call() {
      let adapter = Arc::new(ImportPortAdapter::new(repository, coordinator));
      let cleanup = ImportPreviewCleanup::start_with_interval(
          &adapter,
          Duration::from_secs(60),
      ).unwrap();
      let preview = adapter.preview(ImportSourceKind::LegacyRshellJson, file.path()).await.unwrap();
      tokio::time::advance(Duration::from_secs(14 * 60 + 59)).await;
      assert_eq!(adapter.pending_count(), 1);
      tokio::time::advance(Duration::from_secs(1)).await;
      tokio::task::yield_now().await;
      assert_eq!(adapter.pending_count(), 0);
      assert_eq!(vault.call_counts().put, 0);
      assert_eq!(adapter.cancel(preview.id).await, Err(ImportError::PreviewExpired));
      cleanup.shutdown().await.unwrap();
  }
  ```

  Add separate tests proving: explicit shutdown stops future ticks; dropping the guard cancels/aborts the task; the task's `Weak` does not keep the adapter alive; and `PreviewExpired` from commit and cancel removes the ID from `AppViewModel.pending_imports` before publishing the failure/view.

- [ ] **Step 2: Run RED**

  Run:

  ```powershell
  cargo test --locked -p rshell-storage --test ports periodic_cleanup -- --nocapture
  cargo test --locked -p rshell-core --test application expired_import_preview_is_removed_from_core_view -- --exact --nocapture
  cargo test --locked -p rshell-ui --test import_view_model expired_preview_clears_dialog_state -- --exact --nocapture
  ```

  Expected: FAIL because expiry is only lazy, the process owns no cleanup task, and core retains stale preview authority after storage reports expiry.

- [ ] **Step 3: Implement the bounded cleanup owner**

  Change `PendingPreview.created` to `tokio::time::Instant` so paused-time tests control it. Keep per-call cleanup and add this task shape:

  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum ImportCleanupError {
      #[error("import cleanup interval must be non-zero")]
      InvalidInterval,
      #[error("import cleanup task did not stop before its deadline")]
      Timeout,
      #[error("import cleanup task did not join cleanly")]
      Join,
  }

  pub struct ImportPreviewCleanup {
      cancel: Option<tokio::sync::oneshot::Sender<()>>,
      task: tokio::task::JoinHandle<()>,
  }

  impl ImportPreviewCleanup {
      pub fn start(adapter: &Arc<ImportPortAdapter>) -> Self {
          Self::start_with_interval(adapter, Duration::from_secs(60))
              .expect("the fixed import cleanup interval is non-zero")
      }

      pub fn start_with_interval(
          adapter: &Arc<ImportPortAdapter>,
          interval: Duration,
      ) -> Result<Self, ImportCleanupError>;

      pub async fn shutdown(mut self) -> Result<(), ImportCleanupError>;
  }
  ```

  `start_with_interval` rejects `Duration::ZERO`; fixed `start` uses the valid 60-second constant. The spawned loop captures only `Weak<ImportPortAdapter>`, uses `tokio::time::interval` with the initial immediate tick consumed before entering the select loop, and selects between the next 60-second tick and a one-shot cancel. On each tick, upgrade weakly, call `cleanup_expired`, drop the temporary `Arc`, and exit if upgrade fails. `shutdown` sends once and waits at most five seconds; timeout/join failure returns the exact `ImportCleanupError` variant above. `Drop` sends cancellation and calls `abort()` as a last-resort nonblocking fallback.

- [ ] **Step 4: Wire composition-root ownership and core authority cleanup**

  Add `import_cleanup: ImportPreviewCleanup` to `BootstrappedApplication` after `ApplicationService::start` succeeds and add `BootstrapError::ImportCleanup`. In both normal and P0 shutdown, stop it first, then continue application/session/storage cleanup even if it failed, and return `BootstrapError::ImportCleanup` when it is the first lifecycle error without skipping later cleanup. In core, use one helper for commit/cancel failure:

  ```rust
  fn reconcile_expired_preview(&mut self, preview: ImportPreviewId, error: ImportError) {
      if error == ImportError::PreviewExpired {
          self.view_model.pending_imports.remove(&preview);
          self.publish_view();
      }
  }
  ```

  Add Tokio `sync` and `time` to storage dependencies and `test-util` to storage dev dependencies. Do not add a detached task or an unbounded channel.

- [ ] **Step 5: Run targeted GREEN and lifecycle integration**

  Run:

  ```powershell
  cargo test --locked -p rshell-storage --test ports
  cargo test --locked -p rshell-core --test application
  cargo test --locked -p rshell-ui --test import_view_model
  cargo test --locked -p rshell bootstrap::tests -- --nocapture
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; no port call is needed after the 15-minute deadline, no vault write occurs while idle, adapter and task ownership converge, and shutdown reports cleanup failure instead of hiding it. This is the orchestrator-owned “import preview lifetime” review boundary.

---

### Task 4: Own the Windows PTY process tree with a kill-on-close Job Object

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create from pinned 0.9.6 source: `third_party/portable-pty-psmux/Cargo.toml`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/LICENSE.md`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/README.md`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/examples/bash.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/examples/narrow.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/examples/whoami.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/examples/whoami_async.rs`
- Create: `third_party/portable-pty-psmux/README.rshell-patch.md`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/src/cmdbuilder.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/src/serial.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/src/unix.rs`
- Create from pinned 0.9.6 source and modify: `third_party/portable-pty-psmux/src/lib.rs`
- Create from pinned 0.9.6 source and modify: `third_party/portable-pty-psmux/src/win/procthreadattr.rs`
- Create from pinned 0.9.6 source and modify: `third_party/portable-pty-psmux/src/win/psuedocon.rs`
- Create from pinned 0.9.6 source and modify: `third_party/portable-pty-psmux/src/win/conpty.rs`
- Preserve from pinned 0.9.6 source: `third_party/portable-pty-psmux/src/win/mod.rs`
- Modify: `crates/rshell-platform/Cargo.toml`
- Create: `crates/rshell-platform/src/process_tree.rs`
- Create: `crates/rshell-platform/src/process_tree/windows.rs`
- Modify: `crates/rshell-platform/src/lib.rs`
- Modify: `crates/rshell-session/src/transport/pty.rs`
- Modify: `crates/rshell-session/src/transport/local.rs`
- Modify: `crates/rshell-session/src/transport/local_runtime.rs`
- Modify: `crates/rshell-session/src/transport/local_runtime/lifecycle.rs`
- Test: `crates/rshell-session/tests/local_pty.rs`
- Test fixture: `crates/rshell-session/tests/fixtures/pty_echo.rs`
- Modify: `src/p0_smoke_cleanup.rs`
- Modify: `src/cleanup.rs`
- Test: `tests/p0_acceptance.rs`

**Interfaces:**
- Consumes: pinned `portable-pty-psmux = 0.9.6` ConPTY path, whose `psuedocon.rs` currently builds a one-entry `STARTUPINFOEXW` attribute list, sets `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`, and calls `CreateProcessW` with `EXTENDED_STARTUPINFO_PRESENT`; existing Unix process-group cleanup; `LocalRuntime::shutdown`; immediate-descendant fixture output.
- Produces: patched Windows-only `SlavePty::spawn_command_in_job(&self, command: CommandBuilder, job: BorrowedHandle<'_>) -> Result<Box<dyn Child + Send + Sync>, anyhow::Error>`; vendored `ProcThreadAttributeList { data: Vec<u8>, job_handles: Option<Box<[HANDLE; 1]>> }`, whose heap allocation remains stable while the owner moves; `WindowsProcessJob::new() -> Result<Self, PlatformError>`; `WindowsProcessJob::as_borrowed_handle(&self) -> BorrowedHandle<'_>`; `WindowsProcessJob::contains_process(&self, process_id: u32) -> Result<bool, PlatformError>`; `WindowsProcessJob::terminate(&mut self) -> Result<(), PlatformError>`; `LocalPtyTransport::process_tree_contains(&self, process_id: u32) -> Result<bool, TransportError>` on Windows; creation-time containment before user code runs; explicit direct-child evidence semantics.

- [ ] **Step 1: Add RED contracts for creation-time containment**

  Add a source/dependency acceptance test that requires the selected package manifest to resolve under `third_party/portable-pty-psmux`, requires version `0.9.6`, and requires the patched source to set both `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` and `PROC_THREAD_ATTRIBUTE_JOB_LIST` before the sole contained `CreateProcessW` call. The contract must also prove that `ProcThreadAttributeList` owns `job_handles: Option<Box<[HANDLE; 1]>>`, `set_job` derives `lpValue` from that field rather than a stack-local `HANDLE`/array, and `psuedocon.rs` calls `CreateProcessW` before explicitly dropping the attribute-list owner. It must reject `AssignProcessToJobObject` in rsHell's PTY spawn path as proof of initial containment.

  Extend `pty_echo` with a mode whose first user-code action spawns a long-lived descendant before emitting any readiness marker. On Windows, the integration test must assert all of the following through the concrete local transport's membership probe:

  ```text
  rsHell test/app PID is not a member of the per-session Job
  direct PTY child PID is a member of the per-session Job
  immediate descendant PID is a member of the same per-session Job
  direct child and immediate descendant are alive before shutdown
  both are dead after bounded shutdown/drop
  ```

  Add a creation-failure injection at the vendored `spawn_command_in_job` boundary. Failure to create/configure the Job, add either attribute, or create the process must return no runtime and no child PID.

- [ ] **Step 2: Run Windows RED**

  Run on Windows:

  ```powershell
  cargo test --locked -p rshell --test p0_acceptance windows_pty_uses_creation_time_job_list_attribute -- --exact --nocapture
  cargo test --locked -p rshell-session --test local_pty immediate_descendant_is_contained_before_first_user_marker -- --exact --nocapture
  cargo test --locked -p rshell-session pty::tests::creation_time_containment_failure_returns_no_runtime -- --exact --nocapture
  ```

  Expected: FAIL because crates.io 0.9.6 has only the pseudoconsole attribute, the application can only observe a child after `CreateProcessW`, and no creation-time Job API/membership proof exists.

- [ ] **Step 3: Vendor only the pinned PTY crate and add the compile-proven creation API**

  Copy the exact selected `portable-pty-psmux-0.9.6` package source and `LICENSE.md` into `third_party/portable-pty-psmux`; do not import repository history, files not included by its manifest, or another version. Record crates.io checksum `793e46fb3212b514f6eb694e26a64aeaca64b47a2d66b810351b44628e307a0e` and the four changed source files in `README.rshell-patch.md`. Add this root override while leaving the session's exact `version = "=0.9.6"` requirement intact:

  Keep vendored code outside workspace lint/test membership while compiling it as the patched dependency:

  ```toml
  [workspace]
  exclude = ["third_party/portable-pty-psmux"]

  [patch.crates-io]
  portable-pty-psmux = { path = "third_party/portable-pty-psmux" }
  ```

  Extend the Windows-only `SlavePty` trait with the exact borrowed-handle method in **Interfaces**. The ConPTY implementation must call one shared spawn helper; ordinary `spawn_command` supplies no Job, while rsHell calls `spawn_command_in_job`. In `ProcThreadAttributeList`, define `PROC_THREAD_ATTRIBUTE_JOB_LIST = 0x0002000D`, initialize `job_handles` to `None`, and reject every second `set_job` call so registered storage is never replaced:

  ```rust
  pub struct ProcThreadAttributeList {
      data: Vec<u8>,
      job_handles: Option<Box<[HANDLE; 1]>>,
  }

  pub fn set_job(&mut self, job: BorrowedHandle<'_>) -> Result<(), Error> {
      ensure!(self.job_handles.is_none(), "JOB_LIST already configured");
      self.job_handles = Some(Box::new([job.as_raw_handle() as HANDLE]));
      let attribute_list = self.as_mut_ptr();
      let handles = self.job_handles.as_mut().expect("job storage was just initialized");
      let result = unsafe {
          UpdateProcThreadAttribute(
              attribute_list,
              0,
              PROC_THREAD_ATTRIBUTE_JOB_LIST,
              handles.as_mut_ptr().cast(),
              std::mem::size_of::<HANDLE>(),
              std::ptr::null_mut(),
              std::ptr::null_mut(),
          )
      };
      ensure!(result != 0, "UpdateProcThreadAttribute JOB_LIST failed");
      Ok(())
  }
  ```

  `UpdateProcThreadAttribute` stores the supplied pointer rather than copying the handle array, so a stack-local `HANDLE` is forbidden. `Box<[HANDLE; 1]>` gives the array a stable address even if `ProcThreadAttributeList` moves. When a Job is supplied, allocate `ProcThreadAttributeList::with_capacity(2)`, call `set_pty` and `set_job`, then call `CreateProcessW` once with `EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT`. Keep `attrs` in scope across that call and execute `drop(attrs)` only after `CreateProcessW` returns. `Drop for ProcThreadAttributeList` must call `DeleteProcThreadAttributeList` while `job_handles` is still a live field; Rust drops the fields only after `Drop::drop` returns, so the heap-backed array outlives both process creation and attribute-list destruction. The passthrough-mode retry in `conpty.rs` must call the same contained helper with the same borrowed Job; it must never fall back to uncontained `spawn_command`.

  Extend `windows_pty_uses_creation_time_job_list_attribute` with ordered source assertions for `job_handles` assignment -> `handles.as_mut_ptr()` -> `UpdateProcThreadAttribute`, and `CreateProcessW` -> `drop(attrs)`. Also reject the former `let mut handle = ...; ... addr_of_mut!(handle)` shape. The Windows compile command in Step 6 proves the owner and borrowed-handle API type-check together; this source contract proves the storage owner remains alive for the entire OS call and attribute-list teardown.

- [ ] **Step 4: Create/configure the per-session Job before process creation**

  Enable `Win32_System_JobObjects` in `windows-sys 0.61`. `WindowsProcessJob::new` calls `CreateJobObjectW` and `SetInformationJobObject(JobObjectExtendedLimitInformation)` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; the non-Clone guard owns that handle. It exposes only a borrowed handle to the patched PTY call. `contains_process` opens the target for `PROCESS_QUERY_LIMITED_INFORMATION` and calls `IsProcessInJob` against this exact Job. `terminate` calls `TerminateJobObject`, then closes once; `Drop` performs the same fail-safe close.

  In `spawn_pty_runtime`, perform this Windows order:

  ```text
  create per-session Job -> set KILL_ON_JOB_CLOSE -> open PTY
  -> CreateProcessW with PSEUDOCONSOLE + JOB_LIST in the same STARTUPINFOEX list
  -> return contained child -> drop slave -> clone reader -> take writer
  -> spawn reader -> publish LocalRuntime that owns the Job
  ```

  The Job-list attribute causes Windows to associate the process during `CreateProcessW`, before its initial thread executes user code; no suspended/resume race and no post-spawn assignment is accepted. Do not set `JOB_OBJECT_LIMIT_BREAKAWAY_OK` or `JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK`. Do not call `AssignProcessToJobObject` for either the child or `GetCurrentProcess`; specifically assert the rsHell app/test PID is outside the per-session Job. Missing Job creation/configuration, attribute-list capacity/update failure, `CreateProcessW` failure, reader/writer failure, or reader-thread failure must terminate/close the Job, kill/wait any returned direct child, close PTY handles, and return `SessionFailure::Pty`. `LocalRuntime::shutdown` and `Drop` terminate/close the Job before reader convergence even if the direct child already exited. Preserve Unix process-group behavior.

- [ ] **Step 5: Correct P0 evidence semantics**

  Rename serialized `session_child_count` to `direct_session_child_count` and helper wording to `direct_session_children_are_stopped`. Keep it as a registry count; never describe it as tree proof. Require the Windows/Unix real descendant fixture command as the process-tree acceptance evidence, while application shutdown evidence proves only that each transport reported bounded cleanup success.

- [ ] **Step 6: Run compile-proven GREEN on Windows and cross-platform integration**

  Run:

  ```powershell
  cargo metadata --locked --format-version 1
  cargo check --locked -p rshell-session --all-targets --target x86_64-pc-windows-msvc
  cargo test --locked -p rshell-platform
  cargo test --locked -p rshell-session --test local_pty -- --nocapture
  cargo test --locked -p rshell-session --test system_ssh
  cargo test --locked -p rshell --test p0_acceptance
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; Cargo resolves the exact vendored 0.9.6 package, the patched borrowed-handle call compiles, source assertions prove heap-backed Job-list storage outlives `CreateProcessW` and `DeleteProcThreadAttributeList`, app PID is outside the Job, direct/immediate-descendant PIDs are inside before shutdown, both die on bounded teardown, creation failure publishes no runtime, and direct-PID evidence is not presented as tree evidence. This is the orchestrator-owned “creation-time PTY process-tree ownership” review boundary.

---

### Task 5: Publish actor-owned monotonic frames with follow-bottom policy

**Files:**
- Create: `crates/rshell-session/src/presentation.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/engine.rs`
- Modify: `crates/rshell-session/src/render.rs`
- Modify: `crates/rshell-session/src/message.rs`
- Modify: `crates/rshell-session/src/actor.rs`
- Modify: `crates/rshell-session/src/actor_io.rs`
- Modify: `crates/rshell-session/src/ports.rs`
- Test support: `crates/rshell-session/tests/support/mod.rs`
- Test: `crates/rshell-session/tests/actor_lifecycle.rs`
- Test: `crates/rshell-session/tests/engine_contract.rs`
- Test: `crates/rshell-ui/tests/terminal_view_model.rs`

**Interfaces:**
- Consumes: `RenderFrame`, `Viewport`, `ResolvedTerminalProfile::{scroll_on_output, scroll_on_keypress}`, `FrameClock`, and latest-only `watch` publication.
- Produces: `ViewportBounds { first_stable_row: i64, bottom_top_stable_row: i64 }`; `TerminalEngine::viewport_bounds(&self) -> ViewportBounds`; `PresentationPolicy { scroll_on_output: bool, scroll_on_keypress: bool }`; actor-owned strictly increasing frame generations independent of backend seqno.

- [ ] **Step 1: Add fixed-generation and real-output RED tests**

  Extend the fake engine so every backend frame starts with the same `generation` value. Drive output, selection, nonzero scroll, resize, and scroll-on-keypress input; after each accepted presentation mutation, wait for the watch frame and assert its generation is greater than the previous actor-published generation. Feed each frame through `TerminalViewModel::apply_frame` and assert `FrameUpdate.accepted`.

  Add a real WezTerm long-output test that writes more rows than the viewport and asserts:

  ```text
  initial frame: viewport_top == bottom_top_stable_row
  user scroll up: follow_bottom == false and historical top is visible
  output with scroll_on_output=false: historical top is preserved
  output with scroll_on_output=true: viewport snaps to new bottom
  nonzero scroll back to bottom: follow_bottom == true
  key with scroll_on_keypress=true: viewport snaps to bottom and publishes a fresh frame
  ```

- [ ] **Step 2: Run RED**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test actor_lifecycle presentation_mutations_publish_monotonic_generations -- --exact --nocapture
  cargo test --locked -p rshell-session --test engine_contract long_output_exposes_clamped_viewport_bounds -- --exact --nocapture
  cargo test --locked -p rshell-ui --test terminal_view_model fixed_backend_seqno_frames_are_accepted_when_actor_generation_advances -- --exact --nocapture
  ```

  Expected: FAIL because generation is terminal seqno, viewport has no upper bound/follow state, and policy fields do not reach the actor.

- [ ] **Step 3: Define bounds, policy, and actor presentation state**

  Add these exact shapes:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct ViewportBounds {
      pub first_stable_row: i64,
      pub bottom_top_stable_row: i64,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct PresentationPolicy {
      pub scroll_on_output: bool,
      pub scroll_on_keypress: bool,
  }

  pub(crate) struct PresentationState {
      viewport: Viewport,
      selection: Option<SelectionRange>,
      follow_bottom: bool,
      generation: u64,
      policy: PresentationPolicy,
  }
  ```

  `WezTermAdapter` calculates bounds from stable screen rows and viewport height. Clamp top into `[first_stable_row, bottom_top_stable_row]`. A nonzero user scroll sets `follow_bottom` exactly when the clamped top equals bottom. Output follows when already following; if not following, `scroll_on_output=true` snaps and resumes following while `false` preserves the clamped historical top. Resize preserves historical top unless following. Input snaps/publishes only when `scroll_on_keypress=true`.

- [ ] **Step 4: Stamp publication identity in the actor**

  Remove terminal seqno as public frame identity from `render::snapshot`; initialize its generation to zero. In `publish_frame`, render, increment using `checked_add`, mutate the unshared or copy-on-write frame, then publish:

  ```rust
  let mut frame = self.engine.render(self.presentation.viewport(), self.presentation.selection())?;
  let generation = self.presentation.next_generation()?;
  Arc::make_mut(&mut frame).generation = generation;
  self.frames.send_replace(Some(frame.clone()));
  ```

  Generation exhaustion returns a structured `EngineError` rather than wrapping. Accepted selection and nonzero scroll always mark dirty. Policy-induced viewport changes mark dirty. Keep maximum frame publication at 60 Hz and latest-only watch semantics.

- [ ] **Step 5: Run GREEN and burst integration**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test actor_lifecycle
  cargo test --locked -p rshell-session --test engine_contract
  cargo test --locked -p rshell-ui --test terminal_view_model
  cargo test --locked -p rshell-session --test actor_lifecycle ten_thousand_output_burst_is_latest_only_and_rate_limited -- --exact --nocapture
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; frame generations are strictly monotonic, long-output viewports are nonempty and bounded, scrollback preservation follows policy, and the burst remains latest-only and rate-limited. This is the orchestrator-owned “presentation identity and viewport” review boundary.

---

### Task 6: Compile-prove and use pinned WezTerm keyboard/mouse modes

**Files:**
- Modify: `crates/rshell-session/src/wezterm_adapter.rs`
- Modify: `crates/rshell-session/src/engine.rs`
- Modify: `crates/rshell-session/src/input.rs`
- Test: `crates/rshell-session/tests/wezterm_keyboard_api.rs`
- Test: `crates/rshell-session/tests/engine_contract.rs`

**Interfaces:**
- Consumes: core `KeyCode`, `KeyModifiers`, and `TerminalMouseEvent`; pinned `wezterm_term::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind}`; `Terminal::{key_down, mouse_event}`; `TerminalConfiguration::{enable_csi_u_key_encoding, enable_kitty_keyboard}`; terminal-negotiated DECCKM, Kitty keyboard flags, mouse tracking, and mouse encoding; configured `ResolvedTerminalProfile::mouse_reporting` policy.
- Produces: `WezTermAdapter::encode_key(KeyCode, KeyModifiers) -> Result<Vec<u8>, EngineError>`; `WezTermAdapter::encode_mouse(TerminalMouseEvent) -> Result<Vec<u8>, EngineError>`; pinned application-cursor/CSI-u/Kitty semantics; `mouse_reporting_allowed: bool` kept separate from negotiated `Terminal::is_mouse_grabbed()` and the engine-owned negotiated mouse encoding.

- [ ] **Step 1: Perform the bounded API-discovery check before implementation**

  Before changing runtime code, add a compile-only integration test that imports only the pinned public API and type-checks the intended call surface:

  ```rust
  use wezterm_term::{
      KeyCode as WezKeyCode, KeyModifiers as WezKeyModifiers, MouseEvent as WezMouseEvent,
      Terminal,
  };

  fn compile_key_down(terminal: &mut Terminal, key: WezKeyCode, modifiers: WezKeyModifiers) {
      let _ = terminal.key_down(key, modifiers);
  }

  fn compile_mouse_event(terminal: &mut Terminal, event: WezMouseEvent) {
      let _ = terminal.mouse_event(event);
  }

  #[test]
  fn pinned_wezterm_exposes_the_required_keyboard_api() {
      let function: fn(&mut Terminal, WezKeyCode, WezKeyModifiers) = compile_key_down;
      let mouse_function: fn(&mut Terminal, WezMouseEvent) = compile_mouse_event;
      let _ = function;
      let _ = mouse_function;
  }
  ```

  Confirm Cargo selected exactly the pinned revision and compile the test without running external searches or installing/switching dependencies:

  ```powershell
  $metadata = cargo metadata --locked --format-version 1 | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $wezterm = @($metadata.packages | Where-Object { $_.name -eq "wezterm-term" })
  if ($wezterm.Count -ne 1 -or $wezterm[0].source -notmatch 'd69264df66fdcc928c7a30c673df108984fda821') {
      throw "Expected exactly the selected wezterm-term revision."
  }
  cargo test --locked -p rshell-session --test wezterm_keyboard_api --no-run
  ```

  The pinned source contract being proved is: `TerminalState::key_down` delegates to termwiz `KeyCode::encode`; plain `Char('a')` with no modifier remains `b"a"` even after Kitty flag 1; `Ctrl+Tab` is the pinned modified-key case that emits `b"\x1b[9;5u"`; and `TerminalState::mouse_event` chooses SGR only from negotiated mouse encoding. Stop this task if the public calls or these behavioral bytes do not compile/run against the pinned revision; report the exact mismatch and do not guess a call or alter the pin.

- [ ] **Step 2: Add behavioral RED tests with exact bytes**

  Add tests for:

  ```rust
  // Default and DECCKM application-cursor behavior.
  assert_eq!(engine.encode_input(key(KeyCode::ArrowUp, none())).unwrap(), b"\x1b[A");
  engine.advance(b"\x1b[?1h").unwrap();
  assert_eq!(engine.encode_input(key(KeyCode::ArrowUp, none())).unwrap(), b"\x1bOA");

  // Default xterm versus configured CSI-u.
  assert_eq!(default_engine.encode_input(key(KeyCode::Character('['), ctrl())).unwrap(), b"\x1b");
  assert_eq!(csi_u_engine.encode_input(key(KeyCode::Character('['), ctrl())).unwrap(), b"\x1b[91;5u");

  // Kitty flag 1 does not CSI-u encode an unmodified character in this pin.
  kitty_engine.advance(b"\x1b[>1u").unwrap();
  assert_eq!(kitty_engine.advance(b"\x1b[?u").unwrap().outbound, b"\x1b[?1u");
  assert_eq!(kitty_engine.encode_input(key(KeyCode::Character('a'), none())).unwrap(), b"a");
  assert_eq!(kitty_engine.encode_input(key(KeyCode::Tab, ctrl())).unwrap(), b"\x1b[9;5u");
  ```

  For mouse behavior, first prove policy and negotiation separately: policy `true` with no DECSET produces `frame.mouse_reporting == false` and `UnsupportedMouse`; policy `false` remains false/unsupported even after negotiation. For the positive SGR case, feed both `b"\x1b[?1002h"` (button-motion tracking) and `b"\x1b[?1006h"` (SGR encoding), then pass a left-button press at zero-based cell `(3, 1)` with no modifiers through `Terminal::mouse_event` and assert exact `b"\x1b[<0;4;2M"`. A `1002`-only test must not claim SGR encoding.

- [ ] **Step 3: Run RED**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test engine_contract keyboard_modes_follow_terminal_state_and_profile -- --exact --nocapture
  cargo test --locked -p rshell-session --test engine_contract configured_mouse_policy_can_disable_dynamic_reporting -- --exact --nocapture
  cargo test --locked -p rshell-session --test engine_contract mouse_sgr_requires_tracking_and_sgr_negotiation -- --exact --nocapture
  ```

  Expected: FAIL because the manual xterm encoder ignores DECCKM/CSI-u/Kitty, the manual mouse encoder can emit SGR without negotiated 1006, and configured policy is not distinct from runtime mode.

- [ ] **Step 4: Route keys through the pinned engine API**

  Map every core P0 key variant and modifier bit to the public pinned WezTerm variants. Reject core `super_key` with existing `EngineError::UnsupportedInput`. Call `self.terminal.key_down(mapped_key, mapped_modifiers)`, then drain `SharedWriter::take()` as the encoded result. Map core mouse kind/button/cell/modifiers to pinned WezTerm `MouseEvent`, call `self.terminal.mouse_event(mapped_event)`, and drain the same writer. `DefaultTerminalEngine::encode_input` sends `CommittedText` directly as UTF-8 but delegates keys and accepted mouse events to `WezTermAdapter`; remove the active hand-written SGR encoder so negotiated 1006 remains authoritative.

  Store only the configured policy in `RshellTerminalConfig`; expose `mouse_reporting_allowed()`. Compute the effective permission without conflating the two inputs:

  ```rust
  self.adapter.mouse_reporting_allowed()
      && self.adapter.terminal().is_mouse_grabbed()
  ```

  Use that conjunction for `RenderFrame.mouse_reporting` and for deciding whether to call `encode_mouse`; once called, let `Terminal::mouse_event` choose X10/UTF-8/SGR from negotiated state. Keep defaults byte-compatible. No second keyboard/mouse encoder or terminal adapter remains active.

- [ ] **Step 5: Run compile-proven GREEN**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test engine_contract
  cargo check --locked -p rshell-session --all-targets
  cargo clippy --locked -p rshell-session --all-targets -- -D warnings
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS and exact bytes above, including plain Kitty `a`, modified `Ctrl+Tab` CSI-u, and SGR only after both 1002 and 1006; the compiler proves calls against the pinned revision. This is the orchestrator-owned “terminal backend input modes” review boundary.

---

### Task 7: Make key bindings and side-specific UI input policy operational

**Files:**
- Create: `crates/rshell-core/src/terminal/key_action.rs`
- Modify: `crates/rshell-core/src/terminal.rs`
- Modify: `crates/rshell-core/src/terminal/validation.rs`
- Modify: `crates/rshell-core/src/protocol/commands.rs`
- Modify: `crates/rshell-session/src/message.rs`
- Modify: `crates/rshell-session/src/ports.rs`
- Modify: `crates/rshell-session/src/engine.rs`
- Modify: `crates/rshell-session/src/actor_io.rs`
- Modify: `crates/rshell-ui/src/terminal_view_message.rs`
- Modify: `crates/rshell-ui/src/terminal_input.rs`
- Modify: `crates/rshell-ui/src/terminal_view_model.rs`
- Modify: `crates/rshell-ui/src/terminal_view.rs`
- Modify: `crates/rshell-ui/src/terminal_view_widgets.rs`
- Modify: `crates/rshell-ui/src/pane_host_terminals.rs`
- Modify: `crates/rshell-ui/src/pane_view_model.rs`
- Test: `crates/rshell-core/tests/terminal_profiles.rs`
- Test: `crates/rshell-session/tests/actor_lifecycle.rs`
- Test: `crates/rshell-ui/tests/terminal_input.rs`
- Test: `crates/rshell-ui/tests/terminal_view_model.rs`
- Test: `crates/rshell-ui/tests/workspace_view_model.rs`

**Interfaces:**
- Consumes: `ResolvedTerminalProfile` settings, `AppSettings::key_bindings`, exact core key/modifier chords, `TerminalViewOutput::Command`, and Task 5 monotonic publication.
- Produces: `TerminalSendSequence::{Vt220Delete, Delete127, Backspace8}`; `TerminalKeyAction::{Send(TerminalSendSequence), ClearScrollback, NewTab, SplitVertical}`; `parse_terminal_key_action(&str) -> Result<TerminalKeyAction, SettingsValidationError>`; `SessionUiCommand::ClearScrollback`; side-aware `PhysicalAltState`; binding commands that use the existing sole UI egress.

- [ ] **Step 1: Add RED tests for the closed action set and routing**

  Test that settings validation accepts exactly:

  ```text
  send: followed by exactly ESC [ 3 ~, DEL (0x7f), or backspace (0x08)
  clear_scrollback
  new_tab
  split_vertical
  ```

  Reject blank send payloads, printable send text, NUL, every other byte sequence, and every other action string with `SettingsValidationCode::InvalidAction`. This keeps persisted bindings limited to the three existing legacy terminal-control sequences rather than creating a secret-bearing macro store. Assert profile/connection bindings shadow an app binding with the same exact chord; nonshadowed app bindings remain available.

  Add UI tests proving configured actions emit:

  ```rust
  TerminalKeyAction::Send(sequence)
      => UiCommand::Session { session, command: SessionUiCommand::Input(TerminalInput::CommittedText(sequence.as_str().to_owned())) };
  TerminalKeyAction::ClearScrollback
      => UiCommand::Session { session, command: SessionUiCommand::ClearScrollback };
  TerminalKeyAction::NewTab => UiCommand::NewLocalTab;
  TerminalKeyAction::SplitVertical
      => UiCommand::Split { pane, axis: SplitAxis::Vertical };
  ```

  Verify every command leaves `TerminalView` as `TerminalViewOutput::Command` and reaches the existing PaneHost/MainWindow command path; add no direct port send.

- [ ] **Step 2: Add RED tests for physical Alt side and focus reset**

  Simulate `Alt_L` and `Alt_R` press/release separately. With left disabled/right enabled, left+`x` produces no meta and right+`x` produces meta; reverse the booleans and assertions. With both defaults enabled, retain the existing ESC-prefixed behavior. Focus loss clears both pressed flags. A disabled configured mouse policy must keep wheel input local even after the terminal requests mouse reporting.

- [ ] **Step 3: Run RED**

  Run:

  ```powershell
  cargo test --locked -p rshell-core --test terminal_profiles key_actions_are_closed_and_validated -- --exact --nocapture
  cargo test --locked -p rshell-ui --test terminal_input physical_alt_side_respects_resolved_profile -- --exact --nocapture
  cargo test --locked -p rshell-ui --test terminal_view_model configured_binding_routes_through_ui_command -- --exact --nocapture
  cargo test --locked -p rshell-session --test actor_lifecycle clear_scrollback_publishes_fresh_frame -- --exact --nocapture
  ```

  Expected: FAIL because action strings are not operational/closed, profile is discarded by `TerminalViewModel`, Alt sides are collapsed, and clear-scrollback has no command path.

- [ ] **Step 4: Implement typed actions and exact precedence**

  Add:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum TerminalSendSequence {
      Vt220Delete,
      Delete127,
      Backspace8,
  }

  impl TerminalSendSequence {
      pub const fn as_str(&self) -> &'static str {
          match self {
              Self::Vt220Delete => "\u{1b}[3~",
              Self::Delete127 => "\u{7f}",
              Self::Backspace8 => "\u{8}",
          }
      }
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum TerminalKeyAction {
      Send(TerminalSendSequence),
      ClearScrollback,
      NewTab,
      SplitVertical,
  }
  ```

  Parse/validate in core so storage/imported settings and UI saves share one contract. During profile resolution for a pane, start with resolved profile/connection bindings, then append app bindings only when their exact `(KeyCode, KeyModifiers)` chord is absent. Existing Ctrl+Shift+C/V/F built-ins remain reserved to preserve copy/paste/search; configured bindings resolve after those and before default terminal key mapping.

  Add `ClearScrollback` through core protocol, session mapping, actor, and engine. The engine clears scrollback; actor clamps presentation bounds, marks dirty, and publishes a new generation.

- [ ] **Step 5: Track physical Alt and retain operational profile in UI**

  Extend `TerminalViewInit` with `pane: PaneId`; retain `ResolvedTerminalProfile` in `TerminalViewModel`. Add:

  ```rust
  #[derive(Default)]
  pub(crate) struct PhysicalAltState {
      left_pressed: bool,
      right_pressed: bool,
  }
  ```

  Wire `EventControllerKey::connect_key_pressed` and `connect_key_released`; consume `Alt_L`/`Alt_R` state transitions without sending terminal input. Wire `EventControllerFocus` leave to clear state. For non-Alt keys, set core `modifiers.alt` only when the physically held side is enabled by `left_alt_as_meta`/`right_alt_as_meta`. Preserve aggregate ALT behavior when both settings are true. Keep IM committed text separate from key events.

- [ ] **Step 6: Run GREEN and sole-egress integration**

  Run:

  ```powershell
  cargo test --locked -p rshell-core --test terminal_profiles
  cargo test --locked -p rshell-session --test actor_lifecycle
  cargo test --locked -p rshell-ui --test terminal_input
  cargo test --locked -p rshell-ui --test terminal_view_model
  cargo test --locked -p rshell-ui --test workspace_view_model
  cargo test --locked -p rshell-ui --test component_dependencies
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; defaults remain unchanged, every configured policy has behavioral coverage, unsupported actions are rejected before persistence/use, and command egress remains singular. This is the orchestrator-owned “operational terminal settings” review boundary.

---

### Task 8: Serialize only stable smoke artifact names

**Files:**
- Modify: `src/p0_smoke_report.rs`
- Modify: `scripts/qa/p0-smoke.ps1`
- Test: `tests/p0_acceptance.rs`

**Interfaces:**
- Consumes: absolute internal `SmokeReport::{requested_png_path, png_path}` used for file I/O and `$artifactPng` used by the harness.
- Produces: JSON `requested_png_path`/`png_path` containing only a validated UTF-8 leaf such as `microsoft-windows-10-0-26100-all.png`; `artifact_path_invalid` failure for a missing/invalid serialized name; unchanged actual PNG/JUnit/package paths.

- [ ] **Step 1: Add absolute-path RED regressions**

  Serialize reports constructed with both `C:\Users\alice\work\artifacts\private.png` and `/home/alice/work/artifacts/private.png`. Assert both JSON fields equal `private.png`; assert the JSON string contains none of `C:\Users\alice`, `/home/alice`, workspace root, parent traversal, or another separator. Add cases for missing leaf and non-UTF-8 (Unix-gated) that fail the report with `png_error == "artifact_path_invalid"` rather than serializing a lossy/absolute value.

  Add a P0 acceptance assertion that the PowerShell finalizer assigns a validated leaf variable, not `$artifactPng`, to report fields.

- [ ] **Step 2: Run RED**

  Run:

  ```powershell
  cargo test --locked -p rshell p0_smoke_report::tests::absolute_paths_serialize_as_stable_artifact_names -- --exact --nocapture
  cargo test --locked -p rshell --test p0_acceptance smoke_report_finalizer_keeps_absolute_paths_out_of_json -- --exact --nocapture
  ```

  Expected: FAIL because Rust uses `Path::display()` and PowerShell overwrites fields with the possibly absolute `$artifactPng`.

- [ ] **Step 3: Implement fail-closed artifact-name projection**

  Keep actual path objects for I/O. Project to a serialized name through one helper that handles both slash styles, requires UTF-8, rejects empty/`.`/`..`, rejects rooted or multi-component output, and returns only the final leaf:

  ```rust
  fn artifact_name(path: &Path) -> Result<String, &'static str> {
      let text = path.to_str().ok_or("artifact_path_invalid")?;
      if text.is_empty() || text.ends_with('/') || text.ends_with('\\') {
          return Err("artifact_path_invalid");
      }
      let leaf = text
          .rsplit(|character| matches!(character, '/' | '\\'))
          .next()
          .ok_or("artifact_path_invalid")?;
      if leaf.is_empty()
          || matches!(leaf, "." | "..")
          || leaf.chars().any(|character| matches!(character, '/' | '\\' | ':'))
          || Path::new(leaf).is_absolute()
          || Path::new(leaf).components().count() != 1
      {
          return Err("artifact_path_invalid");
      }
      Ok(leaf.to_owned())
  }
  ```

  If either supplied PNG path cannot produce a valid name, set both serialized path fields to `None`, set `png_error` to `Some("artifact_path_invalid")`, and make completion fail. In PowerShell:

  ```powershell
  $artifactPngName = [System.IO.Path]::GetFileName($artifactPng)
  if ([string]::IsNullOrWhiteSpace($artifactPngName) -or
      [System.IO.Path]::IsPathRooted($artifactPngName) -or
      $artifactPngName -match '[\\/]') {
      throw "The P0 PNG artifact name is invalid."
  }
  $pendingReport.png_path = $artifactPngName
  $pendingReport.requested_png_path = $artifactPngName
  ```

- [ ] **Step 4: Run GREEN and preserve harness/package behavior**

  Run:

  ```powershell
  cargo test --locked -p rshell p0_smoke_report::tests -- --nocapture
  cargo test --locked -p rshell --test p0_acceptance
  pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode Unit
  pwsh -NoProfile -File scripts/qa/assert-package.ps1 -RegressionProbe incomplete-report
  ```

  Expected: PASS; the harness still writes PNG/JSON/JUnit to its owned locations and package checks remain fail-closed, while report JSON contains no workspace/user path. This is the orchestrator-owned “smoke report path privacy” review boundary.

---

### Task 9: Implement and record the exact terminal-engine GO/NO-GO gate

**Files:**
- Remove: `crates/rshell-session/benches/throughput.rs`
- Create: `crates/rshell-session/benches/terminal_engine.rs`
- Create: `crates/rshell-session/tests/fixtures/vt/canary.json`
- Create: `crates/rshell-session/TERMINAL_ENGINE.md`
- Modify: `crates/rshell-session/Cargo.toml`
- Modify: `crates/rshell-session/tests/engine_contract.rs`
- Create: `scripts/qa/terminal-engine-gate.ps1`

**Interfaces:**
- Consumes: sole `DefaultTerminalEngine`, optimized `cargo bench`, pinned WezTerm adapter, Task 5 viewport correctness, and Task 6 input behavior.
- Produces: machine-stable `RSHELL_TERMINAL_ENGINE_GATE version=1` output; exact 100 MiB x five throughput decision; 120x40 full-dirty p95; a gate-generated and render-verified 1000-row SHA-256; measured `TERMINAL_ENGINE.md`; nonzero exit on any miss.

- [ ] **Step 1: Add RED assertions for the missing exact gate**

  First add an acceptance assertion that requires the `terminal_engine` bench target, gate script, fixture contract, and decision record, then run:

  ```powershell
  cargo test --locked -p rshell --test p0_acceptance terminal_engine_gate_contract_is_exact -- --exact --nocapture
  pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1
  ```

  Expected: FAIL because the exact bench/script/record do not exist and the old ignored 50 MiB/20 MiB/s test cannot satisfy the contract. Do not insert a precomputed or guessed scrollback hash to make this RED pass.

- [ ] **Step 2: Add the deterministic fixture and measurement executable**

  Create the fixture initially with an explicitly unrecorded hash state:

  ```json
  {
    "version": 1,
    "throughput_bytes": 104857600,
    "throughput_samples": 5,
    "minimum_mib_per_second": 40.0,
    "frame_cols": 120,
    "frame_rows": 40,
    "maximum_frame_p95_ms": 16.0,
    "scrollback_rows": 1000,
    "line_format": "scrollback-{index:04}",
    "input_separator": "CRLF",
    "input_trailing_crlf": true,
    "canonicalization": "trim ASCII spaces from each rendered row and join rows with LF",
    "sha256": null
  }
  ```

  Here `null` is the deliberate first-run RED state, not a value accepted by local/hosted gates. Replace the ignored legacy test with fixture correctness tests and a harness-free executable that has two modes: normal verification, which exits nonzero when `sha256` is null/mismatched, and `--record-candidate`, which may print a candidate only after exact row-by-row render equality and all unchanged performance thresholds pass.

- [ ] **Step 3: Implement exact measurements and CRLF render correctness**

  In `terminal_engine.rs`:

  - Build one deterministic byte vector of exactly `100 * 1024 * 1024 = 104857600` bytes by repeating a fixed ANSI/Unicode record and truncating, without platform line endings.
  - Create a fresh 120x40 engine for each of five samples, process the entire vector, calculate MiB/s from exact bytes and elapsed monotonic time, sort a copy, and use the third value as median.
  - Warm a 120x40 engine, then take 1000 observations. Each observation writes and renders 40 complete 120-column dirty rows. Sort durations and use nearest-rank index `ceil(0.95 * n) - 1`; require `< 16.0 ms`.
  - Build exactly 1000 expected labels, `scrollback-0000` through `scrollback-0999`. Construct terminal input by appending explicit bytes `b"\r\n"` after every label, including the final label, because WezTerm newline mode is off: CR returns to column zero and LF advances the row. Never use Rust/PowerShell/platform line-ending helpers for this input.
  - Feed those exact CRLF bytes once. Starting at `viewport_bounds().first_stable_row`, render consecutive 40-row windows through `bottom_top_stable_row`, deduplicate overlapping stable-row IDs, discard only the final blank cursor row caused by the trailing CRLF, trim ASCII right-padding from each nonblank rendered row, and require the resulting vector to equal the 1000 expected labels exactly and in order. This equality check, not a presumed digest, is the correctness oracle.
  - Join those verified rendered labels with byte `LF` and no final `LF`, compute SHA-256 with `sha2 = "0.10.9"`, and print the digest. Normal mode requires exact equality with the non-null fixture digest. Candidate mode permits a null fixture only to emit the verified digest and measurements, reports `decision=CANDIDATE`, and never reports GO.

  Print one line for each key below using invariant decimal formatting. The five sample fields, median field, and p95 field contain the actual finite nonnegative decimal measured by that invocation; all other values are exact literals:

  ```text
  RSHELL_TERMINAL_ENGINE_GATE version=1
  backend=wezterm-term@d69264df66fdcc928c7a30c673df108984fda821
  throughput_bytes=104857600
  throughput_sample_1_mib_s
  throughput_sample_2_mib_s
  throughput_sample_3_mib_s
  throughput_sample_4_mib_s
  throughput_sample_5_mib_s
  throughput_median_mib_s
  frame_120x40_observations=1000
  frame_120x40_p95_ms
  scrollback_rows=1000
  scrollback_sha256
  decision=GO
  ```

  The eight measured-key lines are shown without an equals/value solely as field-name schema. Runtime appends the actual value. Format numeric measurements with six fractional digits so they match `^(throughput_sample_[1-5]_mib_s|throughput_median_mib_s|frame_120x40_p95_ms)=[0-9]+\.[0-9]{6}$`; require the digest line to match `^scrollback_sha256=[0-9a-f]{64}$`. Reject missing/duplicate fields, missing equals signs, NaN, infinity, negative values, and recompute the median from emitted samples.

- [ ] **Step 4: Create the fail-closed PowerShell gate**

  `terminal-engine-gate.ps1` runs:

  ```powershell
  cargo bench -p rshell-session --bench terminal_engine --locked
  ```

  Capture combined output, preserve the cargo exit code, require one header and one occurrence of every stable key, parse all numbers with invariant culture, recompute the five-sample median, and fail if bytes/sample count/median/p95/hash/backend/decision differ from the non-null fixture contract. Reject `decision=CANDIDATE` in normal mode. Also require `TERMINAL_ENGINE.md` to contain the command, selected sole adapter, all five recorded samples, median, p95, the same fixture digest, and GO/NO-GO decision.

  Run:

  ```powershell
  pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1
  ```

  Expected before recording: FAIL closed because `sha256` is null and no measured record exists. Do not convert that failure to GO in the script.

- [ ] **Step 5: Generate the verified hash and five-sample record, profile only if required, then run GREEN**

  At an orchestrator-owned review boundary, require the implementation-only source tree to have been frozen by a separately authorized orchestrator commit before recording; capture that SHA with read-only `git rev-parse HEAD` and require no tracked changes. Then run candidate mode without changing source files:

  ```powershell
  cargo bench -p rshell-session --bench terminal_engine --locked -- --record-candidate
  ```

  Expected: only after the 1000 rendered rows equal the generated expected vector and all unchanged thresholds pass, output five actual throughput samples, median, frame p95, a 64-lowercase-hex `scrollback_sha256`, and `decision=CANDIDATE`. If it instead reports NO-GO, retain the failing log, use its section timings to isolate parser throughput versus frame snapshot allocation, inspect `WezTermAdapter::input`, `render::snapshot`, row/cell allocation, and repeated buffer construction, and make only the smallest allocation/reuse optimization consistent with immutable published frames. Any optimization invalidates the captured implementation SHA: rerun targeted tests, obtain a new separately authorized implementation boundary, recapture the SHA, and rerun the same candidate command. Do not reduce trace size, sample count, minimum throughput, frame dimensions, observation count, maximum p95, or hash requirement, and do not add a second adapter.

  Once the unchanged gate emits CANDIDATE, copy that emitted digest into `canary.json`; copy the command, selected adapter, platform/toolchain, all five emitted samples, median, p95, digest, and measured source identity into `TERMINAL_ENGINE.md`. Then run normal mode through the script and require `decision=GO`. The plan intentionally states no digest literal: the first exact CRLF render run is its source.

- [ ] **Step 6: Bind measured evidence without a self-referential commit identity**

  Verify the implementation commit captured before candidate recording did not change during measurement and label it `Measured implementation commit`. A later fixture/decision-record-only orchestrator commit is not claimed as the measured SHA. Final Task 11 reruns normal mode on the ultimate review HEAD and binds that output by artifact hash.

- [ ] **Step 7: Run targeted GREEN**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test engine_contract
  pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS, exact hash match, five samples present, median at least 40 MiB/s, p95 below 16 ms, and one sole backend. This is the orchestrator-owned “terminal engine GO decision” review boundary.

---

### Task 10: Wire fail-closed workflows and update only affected verification docs

**Files:**
- Modify: `scripts/qa/workflow-contract.ps1`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `tests/p0_acceptance.rs`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-08-01-rshell-p0-rebuild.md` (Task22 sections only)

**Interfaces:**
- Consumes: `scripts/qa/terminal-engine-gate.ps1`, real descendant test, renamed direct-child evidence, workspace commands, Mode All, no-secret scan, and package assertion.
- Produces: local/hosted fail-closed command contract on Linux x86_64, macOS arm64, and Windows x86_64; Task22 documentation matching actual changed commands/evidence only.

- [ ] **Step 1: Add workflow-contract RED assertions**

  Require both workflow files to invoke exactly:

  ```powershell
  pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  ```

  CI must run it once in each existing matrix job after workspace quality gates and before P0 All. Release must not run the timing-sensitive performance gate; it remains responsible for deterministic build, package, startup, dependency and payload validation. Require the exact Windows descendant test to remain discoverable under workspace tests and prevent the exact legacy serialized key `"session_child_count"` from returning while allowing `"direct_session_child_count"`.

- [ ] **Step 2: Run RED**

  Run:

  ```powershell
  pwsh -NoProfile -File scripts/qa/workflow-contract.ps1
  cargo test --locked -p rshell --test p0_acceptance workflow_and_cleanup_evidence_contracts_are_fail_closed -- --exact --nocapture
  ```

  Expected: FAIL because neither hosted workflow executes the engine gate and docs/evidence still describe the older contract.

- [ ] **Step 3: Wire CI and release without weakening existing gates**

  Add the engine gate to the shared CI matrix path, not an OS-conditional path, and name the step exactly `Run terminal engine gate`. Keep it absent from Release so noisy runner timing cannot block deterministic package publication. Preserve all current fmt/check/test/clippy, real vault, Mode All, redaction, startup, package, and cleanup steps. Do not use `continue-on-error`; immediately exit on a nonzero CI gate. Hosted output must retain the stable gate lines so Task 11 can bind logs to `headSha`.

- [ ] **Step 4: Update README and only Task22's affected passages**

  Add the engine gate to local command order, state that `direct_session_child_count == 0` is direct-PID evidence only, and name the real descendant fixture as tree proof. In `2026-08-01-rshell-p0-rebuild.md`, edit only Task22 command/evidence/handoff text affected by:

  - workspace `--all-targets --all-features --locked` commands;
  - terminal-engine gate;
  - Windows Job Object descendant evidence;
  - report artifact-relative naming;
  - same-SHA hosted/review evidence.

  Do not alter earlier completed design/task decisions or add P1 scope.

- [ ] **Step 5: Run GREEN and documentation contract checks**

  Run:

  ```powershell
  pwsh -NoProfile -File scripts/qa/workflow-contract.ps1
  cargo test --locked -p rshell --test p0_acceptance
  cargo test --locked -p rshell --test production_module_limits
  ```

  Expected: PASS; static workflow checks prove fail-closed wiring but are not represented as hosted execution evidence. This is the orchestrator-owned “workflow and Task22 contract” review boundary.

---

### Task 11: Run complete gates and freeze one artifact/review identity

**Files:**
- Generate, untracked: `artifacts/final-review/$reviewHead/terminal-engine.log`
- Generate, untracked: `artifacts/final-review/$reviewHead/workflow-contract.log`
- Generate, untracked: `artifacts/final-review/$reviewHead/p0-smoke/*.json`
- Generate, untracked: `artifacts/final-review/$reviewHead/p0-smoke/*.png`
- Generate, untracked: `artifacts/final-review/$reviewHead/p0-smoke/*.junit.xml`
- Download, untracked: `artifacts/final-review/$reviewHead/hosted/*`
- Generate, untracked: `artifacts/final-review/$reviewHead/candidate-identity.json`
- Generate, untracked: `artifacts/final-review/$reviewHead/hosted-runs.json`
- Generate, untracked: `artifacts/final-review/$reviewHead/artifact-hashes.json`
- No tracked file changes are allowed after the review identity is frozen.

**Interfaces:**
- Consumes: all prior task outputs, base SHA `61c0fb4dff250cdd698b22d3fd3b5474862b1074`, separately authorized orchestrator commits/push, GitHub Actions API, `GITHUB_TOKEN`, Oracle-high, and Reviewer-high.
- Produces: complete local command receipts; real Mode All/no-secret/package evidence; successful CI and Release runs whose `head_sha` equals the frozen review HEAD; artifact SHA-256 manifest; two acceptance receipts naming the identical base range, HEAD, and manifest hash.

- [ ] **Step 1: Run all local gates fail-fast before any hosted claim**

  All separately authorized product/test/doc commits must already be complete. In one PowerShell session, capture the candidate identity, create its evidence directory, run each command separately, and preserve full output/exit code. The two stable script protocols are also written to identity-bound logs:

  ```powershell
  $baseSha = "61c0fb4dff250cdd698b22d3fd3b5474862b1074"
  $reviewHead = (git rev-parse HEAD).Trim()
  if ($LASTEXITCODE -ne 0 -or $reviewHead -notmatch '^[0-9a-f]{40}$') { throw "Review HEAD is invalid." }
  $evidenceRoot = "artifacts/final-review/$reviewHead"
  [void](New-Item -ItemType Directory -Force -Path $evidenceRoot)
  $localGateStarted = [DateTimeOffset]::UtcNow
  [ordered]@{
      base = $baseSha
      head = $reviewHead
      local_gate_started_utc = $localGateStarted.ToString('O')
  } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceRoot "candidate-identity.json") -Encoding utf8NoBOM
  cargo fmt --all -- --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo check --workspace --all-targets --all-features --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo test --workspace --all-features --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1 *>&1 |
      Tee-Object -FilePath (Join-Path $evidenceRoot "terminal-engine.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & pwsh -NoProfile -File scripts/qa/workflow-contract.ps1 *>&1 |
      Tee-Object -FilePath (Join-Path $evidenceRoot "workflow-contract.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot artifacts/p0-smoke
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  ```

  Expected: every command exits 0; Mode All has no skipped action and every fixed surface passes; JSON paths are leaf-only; PNG/JUnit exist; cleanup reports zero actors and direct children without claiming tree proof; the no-secret scan checks actual scenario secret bytes.

- [ ] **Step 2: Prove dependency and process-tree contracts explicitly**

  Run:

  ```powershell
  cargo test --locked -p rshell-session --test local_pty immediate_descendant_is_contained_before_first_user_marker -- --exact --nocapture
  $tree = cargo tree --workspace --all-features --locked --edges normal,build
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  if ($tree -match '(?i)libssh2|openssl|05343b3') { throw "Forbidden legacy terminal/SSH dependency detected." }
  $metadata = cargo metadata --locked --format-version 1 | ConvertFrom-Json
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $weztermPackages = @($metadata.packages | Where-Object { $_.name -eq 'wezterm-term' })
  if ($weztermPackages.Count -ne 1 -or $weztermPackages[0].source -notmatch 'd69264df66fdcc928c7a30c673df108984fda821') {
      throw "The dependency graph does not contain exactly the selected wezterm-term revision."
  }
  $portablePackages = @($metadata.packages | Where-Object { $_.name -eq 'portable-pty-psmux' })
  $portableManifest = (Resolve-Path -LiteralPath 'third_party/portable-pty-psmux/Cargo.toml').Path
  if ($portablePackages.Count -ne 1 -or
      $portablePackages[0].version -ne '0.9.6' -or
      $portablePackages[0].manifest_path -ne $portableManifest -or
      -not [string]::IsNullOrEmpty($portablePackages[0].source)) {
      throw "The dependency graph does not contain exactly the vendored portable-pty-psmux 0.9.6 patch."
  }
  ```

  Expected: on Windows the app PID is outside the per-session Job while the immediate descendant is a member and dies after bounded shutdown; on every host no legacy dependency marker exists, Cargo selects only the vendored PTY 0.9.6 patch and the pinned WezTerm revision, and no post-spawn assignment is claimed as creation-time proof.

- [ ] **Step 3: Verify and freeze the already committed review identity**

  Run read-only identity checks and require HEAD to equal the candidate captured before Step 1. No Git write is allowed between Step 1 and either final receipt:

  ```powershell
  $baseSha = "61c0fb4dff250cdd698b22d3fd3b5474862b1074"
  $reviewHead = (git rev-parse HEAD).Trim()
  if ($LASTEXITCODE -ne 0 -or $reviewHead -notmatch '^[0-9a-f]{40}$') { throw "Review HEAD is invalid." }
  $evidenceRoot = "artifacts/final-review/$reviewHead"
  $candidate = Get-Content -LiteralPath (Join-Path $evidenceRoot "candidate-identity.json") -Raw | ConvertFrom-Json
  if ($candidate.base -ne $baseSha -or $candidate.head -ne $reviewHead) { throw "HEAD changed after local gates." }
  git status --short
  git diff --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  git diff --cached --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $trackedChanges = @(
      git diff --name-only
      git diff --cached --name-only
  )
  $trackedChanges = @($trackedChanges | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
  if ($trackedChanges.Count -ne 0) { throw "Tracked working-tree changes invalidate the review identity." }
  $rangeNames = @(git diff --name-only "$baseSha..$reviewHead")
  if ($LASTEXITCODE -ne 0 -or $rangeNames.Count -eq 0) { throw "Committed corrective range is empty or unreadable." }
  $unknown = @(git ls-files --others --exclude-standard)
  $unexpected = @($unknown | Where-Object {
      $_ -notlike 'artifacts/p0-smoke/*' -and
      $_ -notlike "artifacts/final-review/$reviewHead/*"
  })
  if ($unexpected.Count -ne 0) { throw "Unexpected untracked files invalidate the review identity: $($unexpected -join ', ')" }
  ```

  Any tracked edit, new commit, gate-affecting artifact change, or unexpected untracked code after this point invalidates all later receipts.

- [ ] **Step 4: Obtain same-SHA hosted CI and Release evidence**

  Only after a separately authorized push to the branch that triggers both CI and Release, query by exact SHA. Derive the repository from `origin`, use a 30-minute deadline and 15-second polling interval, and fail rather than waiting forever:

  ```powershell
  $baseSha = "61c0fb4dff250cdd698b22d3fd3b5474862b1074"
  $reviewHead = (git rev-parse HEAD).Trim()
  $evidenceRoot = "artifacts/final-review/$reviewHead"
  $candidate = Get-Content -LiteralPath (Join-Path $evidenceRoot "candidate-identity.json") -Raw | ConvertFrom-Json
  if ($candidate.base -ne $baseSha -or $candidate.head -ne $reviewHead) { throw "Hosted query identity differs from local gates." }
  $origin = (git remote get-url origin).Trim()
  if ($origin -notmatch 'github\.com[:/](?<repo>[^/]+/[^/.]+)(?:\.git)?$') { throw "GitHub origin is unavailable." }
  $repository = $matches.repo
  if ([string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) { throw "GITHUB_TOKEN is required for hosted evidence." }
  $headers = @{
      Authorization = "Bearer $env:GITHUB_TOKEN"
      Accept = "application/vnd.github+json"
      'X-GitHub-Api-Version' = "2022-11-28"
  }
  $ciUri = "https://api.github.com/repos/$repository/actions/workflows/ci.yml/runs?head_sha=$reviewHead&event=push&per_page=100"
  $releaseUri = "https://api.github.com/repos/$repository/actions/workflows/release.yml/runs?head_sha=$reviewHead&event=push&per_page=100"

  function Wait-WorkflowRun {
      param([string]$Uri, [string]$WorkflowName)
      $deadline = [DateTimeOffset]::UtcNow.AddMinutes(30)
      do {
          $response = Invoke-RestMethod -Uri $Uri -Headers $headers
          $runs = @($response.workflow_runs | Where-Object { $_.head_sha -eq $reviewHead } |
              Sort-Object created_at -Descending)
          if ($runs.Count -gt 0 -and $runs[0].status -eq 'completed') {
              if ($runs[0].conclusion -ne 'success') {
                  throw "$WorkflowName failed for $reviewHead."
              }
              return $runs[0]
          }
          Start-Sleep -Seconds 15
      } while ([DateTimeOffset]::UtcNow -lt $deadline)
      throw "$WorkflowName did not complete for $reviewHead before the bounded deadline."
  }

  function Assert-WorkflowJobs {
      param($Run, [string[]]$ExpectedNames, [hashtable]$RequiredSteps)
      $jobsUri = "https://api.github.com/repos/$repository/actions/runs/$($Run.id)/jobs?per_page=100"
      $jobs = @((Invoke-RestMethod -Uri $jobsUri -Headers $headers).jobs)
      foreach ($name in $ExpectedNames) {
          $matching = @($jobs | Where-Object { $_.name -eq $name })
          if ($matching.Count -ne 1 -or $matching[0].conclusion -ne 'success') {
              throw "Hosted job $name is missing or unsuccessful."
          }
          foreach ($stepName in @($RequiredSteps[$name])) {
              $steps = @($matching[0].steps | Where-Object { $_.name -eq $stepName })
              if ($steps.Count -ne 1 -or $steps[0].conclusion -ne 'success') {
                  throw "Hosted step $name / $stepName is missing or unsuccessful."
              }
          }
      }
  }

  $ciRun = Wait-WorkflowRun -Uri $ciUri -WorkflowName "CI"
  $releaseRun = Wait-WorkflowRun -Uri $releaseUri -WorkflowName "Release"
  Assert-WorkflowJobs -Run $ciRun -ExpectedNames @(
      'Linux x86_64', 'macOS arm64', 'Windows x86_64'
  ) -RequiredSteps @{
      'Linux x86_64' = @('Run terminal engine gate', 'Run Secret Service vault probe and P0 All smoke (Linux)')
      'macOS arm64' = @('Run terminal engine gate', 'Run temporary keychain vault probe and P0 All smoke (macOS)')
      'Windows x86_64' = @('Run terminal engine gate', 'Run Credential Manager vault probe and P0 All smoke (Windows)')
  }
  Assert-WorkflowJobs -Run $releaseRun -ExpectedNames @(
      'Build linux-x86_64', 'Build macos-arm64', 'Build windows-x86_64', 'Release'
  ) -RequiredSteps @{
      'Build linux-x86_64' = @('Package (Linux/macOS)', 'Upload artifact (Unix)')
      'Build macos-arm64' = @('Package (Linux/macOS)', 'Upload artifact (Unix)')
      'Build windows-x86_64' = @('Package (Windows)', 'Upload artifact (Windows)')
      'Release' = @('Download all artifacts', 'Update Nightly')
  }
  [ordered]@{
      head = $reviewHead
      ci_run_id = $ciRun.id
      ci_head_sha = $ciRun.head_sha
      release_run_id = $releaseRun.id
      release_head_sha = $releaseRun.head_sha
  } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidenceRoot "hosted-runs.json") -Encoding utf8NoBOM
  ```

  The Task 10 workflow step names must match this contract exactly. A queued/in-progress/missing run is `BLOCKED`, never PASS.

- [ ] **Step 5: Download, hash, and locally recheck hosted artifacts**

  Download the exact Release run artifacts and both run-log archives into `artifacts/final-review/$reviewHead/hosted`. Expand each GitHub artifact wrapper ZIP into a directory named for the API artifact, and require the contained release package with its exact target filename:

  ```powershell
  $baseSha = "61c0fb4dff250cdd698b22d3fd3b5474862b1074"
  $reviewHead = (git rev-parse HEAD).Trim()
  $evidenceRoot = "artifacts/final-review/$reviewHead"
  $candidate = Get-Content -LiteralPath (Join-Path $evidenceRoot "candidate-identity.json") -Raw | ConvertFrom-Json
  $hostedRuns = Get-Content -LiteralPath (Join-Path $evidenceRoot "hosted-runs.json") -Raw | ConvertFrom-Json
  if ($candidate.base -ne $baseSha -or $candidate.head -ne $reviewHead -or $hostedRuns.head -ne $reviewHead) {
      throw "Artifact download identity differs from local/hosted gates."
  }
  $origin = (git remote get-url origin).Trim()
  if ($origin -notmatch 'github\.com[:/](?<repo>[^/]+/[^/.]+)(?:\.git)?$') { throw "GitHub origin is unavailable." }
  $repository = $matches.repo
  $headers = @{
      Authorization = "Bearer $env:GITHUB_TOKEN"
      Accept = "application/vnd.github+json"
      'X-GitHub-Api-Version' = "2022-11-28"
  }
  $ciRun = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/actions/runs/$($hostedRuns.ci_run_id)" -Headers $headers
  $releaseRun = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/actions/runs/$($hostedRuns.release_run_id)" -Headers $headers
  if ($ciRun.head_sha -ne $reviewHead -or $releaseRun.head_sha -ne $reviewHead) { throw "Hosted run SHA changed." }
  $hostedRoot = Join-Path $evidenceRoot "hosted"
  [void](New-Item -ItemType Directory -Force -Path $hostedRoot)
  $artifactUri = "https://api.github.com/repos/$repository/actions/runs/$($releaseRun.id)/artifacts?per_page=100"
  $artifacts = @((Invoke-RestMethod -Uri $artifactUri -Headers $headers).artifacts |
      Where-Object { -not $_.expired })
  $expectedArtifacts = @(
      'rshell-x86_64-unknown-linux-gnu',
      'rshell-aarch64-apple-darwin',
      'rshell-x86_64-pc-windows-msvc'
  )
  foreach ($name in $expectedArtifacts) {
      $artifact = @($artifacts | Where-Object { $_.name -eq $name })
      if ($artifact.Count -ne 1) { throw "Hosted artifact $name is missing or ambiguous." }
      $wrapper = Join-Path $hostedRoot "$name.github-artifact.zip"
      Invoke-WebRequest -Uri $artifact[0].archive_download_url -Headers $headers -OutFile $wrapper
      $destination = Join-Path $hostedRoot $name
      Expand-Archive -LiteralPath $wrapper -DestinationPath $destination
  }
  $expectedPackages = @(
      (Join-Path $hostedRoot 'rshell-x86_64-unknown-linux-gnu/rshell-x86_64-unknown-linux-gnu.tar.gz'),
      (Join-Path $hostedRoot 'rshell-aarch64-apple-darwin/rshell-aarch64-apple-darwin.tar.gz'),
      (Join-Path $hostedRoot 'rshell-x86_64-pc-windows-msvc/rshell-x86_64-pc-windows-msvc.zip')
  )
  foreach ($package in $expectedPackages) {
      if (-not (Test-Path -LiteralPath $package -PathType Leaf)) { throw "Hosted package is missing: $package" }
  }
  foreach ($run in @($ciRun, $releaseRun)) {
      $logArchive = Join-Path $hostedRoot "$($run.name)-$($run.id)-logs.zip"
      $logUri = "https://api.github.com/repos/$repository/actions/runs/$($run.id)/logs"
      Invoke-WebRequest -Uri $logUri -Headers $headers -OutFile $logArchive
      Expand-Archive -LiteralPath $logArchive -DestinationPath (Join-Path $hostedRoot "$($run.name)-$($run.id)-logs")
  }
  ```

  On the current Windows host, run the real downloaded package check:

  ```powershell
  pwsh -NoProfile -File scripts/qa/assert-package.ps1 -Target x86_64-pc-windows-msvc -Package "artifacts/final-review/$reviewHead/hosted/rshell-x86_64-pc-windows-msvc/rshell-x86_64-pc-windows-msvc.zip"
  pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot "artifacts/final-review/$reviewHead"
  ```

  Copy only P0 artifacts written during this candidate's local run, identified by timestamp and a shared stem; do not absorb stale files from the pre-existing `artifacts/` directory. Then generate a manifest excluding itself:

  ```powershell
  $p0Source = "artifacts/p0-smoke"
  $started = [DateTimeOffset]::Parse($candidate.local_gate_started_utc)
  $freshJson = @(Get-ChildItem -LiteralPath $p0Source -Filter '*.json' -File |
      Where-Object { $_.LastWriteTimeUtc -ge $started.UtcDateTime })
  if ($freshJson.Count -ne 1) { throw "Expected exactly one fresh Mode All JSON report." }
  $stem = $freshJson[0].BaseName
  $freshPng = Join-Path $p0Source "$stem.png"
  $freshJunit = Join-Path $p0Source "$stem.junit.xml"
  foreach ($file in @($freshPng, $freshJunit)) {
      if (-not (Test-Path -LiteralPath $file -PathType Leaf) -or
          (Get-Item -LiteralPath $file).LastWriteTimeUtc -lt $started.UtcDateTime) {
          throw "Fresh Mode All PNG/JUnit evidence is missing."
      }
  }
  $p0Destination = Join-Path $evidenceRoot "p0-smoke"
  [void](New-Item -ItemType Directory -Force -Path $p0Destination)
  Copy-Item -LiteralPath $freshJson[0].FullName -Destination $p0Destination
  Copy-Item -LiteralPath $freshPng -Destination $p0Destination
  Copy-Item -LiteralPath $freshJunit -Destination $p0Destination
  $manifestPath = Join-Path $evidenceRoot "artifact-hashes.json"
  $hashes = @(Get-ChildItem -LiteralPath $evidenceRoot -Recurse -File |
      Where-Object { $_.FullName -ne [System.IO.Path]::GetFullPath($manifestPath) } |
      Sort-Object FullName |
      ForEach-Object {
          $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
          [pscustomobject]@{
              path = [System.IO.Path]::GetRelativePath($evidenceRoot, $_.FullName).Replace('\', '/')
              sha256 = $hash.Hash.ToLowerInvariant()
          }
      })
  if ($hashes.Count -lt 7) { throw "Identity-bound evidence set is incomplete." }
  $hashes | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
  $manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  ```

  Expected: package assertion/no-secret scan PASS; manifest includes terminal-engine log, JSON, PNG, JUnit, and all three hosted packages/log artifacts.

- [ ] **Step 6: Obtain final Oracle-high and Reviewer-high acceptance on one identity**

  The orchestrator, not a planning or implementation subagent, dispatches Oracle-high and Reviewer-high only after Steps 1-5 are immutable. Build the exact identity block from the already populated variables, serialize it, and include it unchanged in both requests:

  ```powershell
  $baseSha = "61c0fb4dff250cdd698b22d3fd3b5474862b1074"
  $reviewHead = (git rev-parse HEAD).Trim()
  $evidenceRoot = "artifacts/final-review/$reviewHead"
  $hostedRuns = Get-Content -LiteralPath (Join-Path $evidenceRoot "hosted-runs.json") -Raw | ConvertFrom-Json
  $manifestPath = Join-Path $evidenceRoot "artifact-hashes.json"
  $manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $reviewIdentity = [ordered]@{
      base = $baseSha
      head = $reviewHead
      committed_range = "$baseSha..$reviewHead"
      artifact_manifest = "artifacts/final-review/$reviewHead/artifact-hashes.json"
      artifact_manifest_sha256 = $manifestSha256
      hosted_ci_head_sha = $hostedRuns.ci_head_sha
      hosted_release_head_sha = $hostedRuns.release_head_sha
  }
  $reviewIdentityJson = $reviewIdentity | ConvertTo-Json -Compress
  ```

  Each receipt must repeat the exact serialized values from `$reviewIdentityJson`, not merely refer to a filename or “current HEAD.”

  Both reviews inspect the full committed range and the same artifact manifest, and explicitly accept or reject every blocker and global constraint. Do not dispatch either reviewer during planning. A timeout, partial response, different SHA, different manifest hash, or older-range verdict is not acceptance.

- [ ] **Step 7: Enforce invalidation and final completion rule**

  If either review requests a product/test/doc correction, if any commit changes, if any gate is rerun and changes a bound artifact, or if hosted runs target another SHA: discard both receipts, perform the separately authorized correction, rerun all Task 11 gates/hosted evidence/hashing, and obtain both reviews again. Completion requires two explicit passes with identical base, head, hosted SHA, and manifest SHA; no hosted or final-review PASS is claimed before that condition.

---

## Dependency Order and Review Boundaries

1. Task 1 establishes the global LOC guard before any corrective code grows.
2. Tasks 2-4 correct independent secret, import-lifetime, and process-tree ownership blockers against that guard.
3. Task 5 establishes actor presentation identity/policy consumed by Tasks 6-7.
4. Task 6 compile-proves and uses the pinned backend key/mouse API.
5. Task 7 makes the UI/core settings and actions operational through the existing command port.
6. Task 8 fixes report privacy before real smoke artifacts are generated.
7. Task 9 measures the corrected engine and records the unchanged GO/NO-GO thresholds.
8. Task 10 wires the validated gate/evidence semantics into workflows and affected docs.
9. Task 11 runs local and hosted acceptance, freezes artifacts, and obtains same-identity final reviews.

The orchestrator may create semantic commits at the named review boundaries under separate authorization. No task may use a commit to excuse a red test or partial cleanup; each boundary is independently testable before the orchestrator performs a Git write.

## Requirement Coverage Matrix

| Verified blocker | Corrective task(s) | Primary proof |
|---|---:|---|
| Global <=250 pure-production-LOC | 1, all later tasks | Recursive `production_module_limits` test over every product `src/**/*.rs` |
| Native reconnect secret reuse | 2 | no `Arc<SecretString>`, one-shot factory, one TCP accept, two fresh vault reads on RetryPane |
| 15-minute preview / 60-second cleanup | 3 | paused-time expiry without port call, weak task ownership, bounded shutdown/drop, zero vault writes |
| Windows PTY process tree | 4 | vendored 0.9.6 `PROC_THREAD_ATTRIBUTE_JOB_LIST`, KILL_ON_JOB_CLOSE before user code, app outside Job, immediate descendant inside/dead on Windows |
| Monotonic frames and follow-bottom | 5 | fixed backend generation accepted, bounded real long-output viewport/policy tests |
| Operational terminal settings | 5-7 | compile-proven WezTerm bytes, typed binding actions, side-specific Alt, mouse/scroll behavior |
| Smoke report path privacy | 8 | Windows/POSIX absolute-path serialization regression and harness finalizer contract |
| Exact terminal-engine gate | 9-10 | 100 MiB x five median, 120x40 p95, explicit-CRLF 1000-row render equality and generated SHA-256, measured record, fail-closed workflows |
| Complete verification and final acceptance | 10-11 | local gates, real Mode All/no-secret/package, 3-platform CI/release, same-identity dual review |

## Plan Self-Review Result

- **Spec coverage:** Every verified blocker and global constraint maps to at least one task and an executable proof in the matrix above.
- **Interface consistency:** `PresentationPolicy` is created in Task 5 and consumed by later input work; `TerminalKeyAction` and `ClearScrollback` are introduced together in Task 7; `WindowsProcessJob` is created before `spawn_command_in_job` and then owned only by PTY runtime; configured mouse policy remains distinct from WezTerm-negotiated mode; `ImportPreviewCleanup` is owned only by composition root.
- **Failure semantics:** Authentication exhaustion, import-task shutdown, Job creation/attribute/process-creation/termination failures, frame generation overflow, invalid report names, missing or mismatched engine digest/threshold evidence, missing hosted runs, and identity drift all fail closed.
- **Scope:** Changes are limited to the nine verified blockers, affected Task22 documentation, and their test/verification surfaces; no P1 feature or second runtime is introduced.
- **Incomplete-marker/content scan:** Runtime-measured values are defined as parsed outputs copied exactly into evidence; no implementation decision is deferred and no incomplete marker remains.
- **Agent-executable QA:** Every test/gate has an exact PowerShell-compatible command and observable PASS/FAIL condition; user visual confirmation is not an acceptance dependency.

**Plan review receipt status:** `waiting for receipt`. Per the request, this planning session does not dispatch a plan critic, Oracle, or Reviewer; formal review and implementation execution are orchestrator-owned.
