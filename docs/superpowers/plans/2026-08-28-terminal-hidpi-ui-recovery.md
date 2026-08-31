# Terminal Recovery, HiDPI, and Adaptive Fluent UI Implementation Plan

> **For agentic workers:** Use the subagent-driven-development skill to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver safe terminal interruption and deterministic display recovery, measured HiDPI-correct terminal geometry and icons, an adaptive terminal-first Fluent GTK shell, and identity-bound native/hosted evidence without changing SSH, storage, credential, or import semantics.

**Architecture:** Extend the existing core protocol, session actor, Alacritty adapter, and GTK presentation through typed commands and immutable recovery/geometry evidence while preserving `MainWindow::dispatch -> command_port::dispatch -> UiCommandPort::try_send` as the only UI command egress. Keep measurement, recovery tracking, adaptive layout, modal coordination, overflow policy, and smoke reporting in focused modules, each at or below 250 pure production LOC. Prove pure decisions first, then native GTK/PTY behavior, then real Windows ConPTY, package, hosted three-platform, and same-identity review evidence.

**Tech Stack:** Rust 2024, GTK4/gtk-rs 0.10.3, Relm4 0.10.1, Pango/Pangocairo 0.21.5, Alacritty Terminal 0.26.0, Tokio, portable-pty-psmux 0.9.6, Cairo/GSK/GdkPixbuf, PowerShell 7, Cargo workspace tests, and GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-28-terminal-hidpi-ui-recovery-design.md`

**Global Constraints:**
- GTK4/Relm4, the core-owned application boundary, and the single UI command adapter remain.
- SSH authentication, host-key verification, credential storage, journaling, imports, and reconnect security semantics do not change.
- Ctrl+C without Shift/Alt/Super is a safety interrupt and emits exactly one ETX byte, independent of Kitty/CSI-u state.
- A single Ctrl+C must not automatically destroy a TUI that catches the interrupt and continues.
- Session exit, failure, crash, and disconnect must publish a mode-clean final presentation.
- Product terminal geometry must not contain a fixed 9×18 assumption.
- The same measured metrics drive rendering, cursor, selection, hit-testing, rows/columns, and PTY pixel dimensions.
- Icons remain source-controlled embedded SVG/internal vectors; no external icon payload is introduced.
- Product icons are rendered for the effective physical target and cached by icon/backend/physical size.
- Background widgets are insensitive while a modal overlay is open; focus is contained and restored.
- Production modules remain at or below the repository's 250 pure-LOC limit.
- Secrets, paths, endpoints, and terminal text do not enter public diagnostics, reports, screenshots, or Debug output beyond existing redacted contracts.

---

## Execution guard and baseline preservation

- Planning base is `c7be3bf2ccfbd635075d0ef5b1e89c94271fdd14`. Before each implementation task, run `git status --short` and preserve the pre-existing untracked `artifacts/`, approved spec, and excluded debug journal. Never reset, restore, clean, checkout, delete, copy from, or overwrite those paths.
- This plan changes no dependency version and adds no crate. `Cargo.toml`, every crate manifest, and `Cargo.lock` are read-only contract inputs.
- Do not add shell-profile injection, prompt parsing, a framework/backend rewrite, P1 SSH features, security/persistence changes, browser tooling, platform-specific WinUI, external icon payloads, or synthetic claims of physical monitor scale.
- Runtime probes become source-controlled tests or fixtures only. Debug-only binaries, temporary capture files, and journal output are not copied into the repository.
- Every product command still leaves the UI through `command_port::dispatch`; component messages may change, but no second `.try_send(` site is permitted.
- A task is not green until its targeted tests, `cargo test --locked --test production_module_limits`, and `cargo test -p rshell-ui --test component_dependencies --locked` pass.
- Suggested commits are review boundaries only. The orchestrator may execute them solely when the conversation already contains explicit user authorization for Git writes; otherwise stop after the tests and report the exact paths that would be committed.

## File map and responsibility boundaries

### Design, fixtures, and stable contracts

- Modify `DESIGN.md` — make the approved recovery, metric, breakpoint, modal, hierarchy, accessibility, and evidence rules the current design authority.
- Modify `resources/style.css` — implement the 4px rhythm, 13/14px root type, modal scrim, adaptive shell, recovery notice, overflow, focus, and state selectors without gradients, shadows, or translucent effects.
- Modify `crates/rshell-ui/tests/theme_contract.rs` — lock the new tokens, breakpoints, contrast/state markers, and prohibited effects.
- Create `crates/rshell-session/tests/support/display_recovery.rs` and modify `crates/rshell-session/tests/support/mod.rs` — one permanent OSC/1049/Kitty/mouse/application-cursor/hidden-cursor/UTF-8 byte fixture and helpers.
- Create `crates/rshell-session/tests/fixtures/interrupt_tui.rs` — real PTY fixture that can either survive ETX in alternate screen or exit without restoring modes.

### Core/session recovery boundary

- Create `crates/rshell-core/src/protocol/session.rs`; modify `crates/rshell-core/src/protocol.rs` and `crates/rshell-core/src/protocol/commands.rs` — move session commands/events out of the full module and add typed interrupt/reset/recovery types.
- Modify `crates/rshell-core/src/render.rs` — add `TerminalDisplayModes` while retaining the compatible title/alternate/mouse frame fields.
- Create `crates/rshell-session/src/alacritty_display.rs` — the only Alacritty mode-inspection and display-recovery adapter.
- Create `crates/rshell-session/src/actor_commands.rs` and `crates/rshell-session/src/actor_termination.rs`; modify `actor.rs`, `actor_io.rs`, `engine.rs`, `message.rs`, `ports.rs`, `manager.rs`, `presentation.rs`, `alacritty_adapter.rs`, `alacritty_event.rs`, `render.rs`, and `lib.rs` — keep command handling, transport output, recovery tracking, and terminal-state completion separate and under the cap.
- Modify `crates/rshell-core/src/application/model.rs` and `crates/rshell-core/src/application/session_events.rs` — retain or clear the current recovery notice by authoritative session identity.
- Create `crates/rshell-ui/src/pane_status.rs`; modify `pane_view_model.rs`, `pane_host.rs`, `pane_host_model.rs`, and `pane_host_render.rs` — expose one mutually exclusive terminal/recovery/disconnected presentation and the Reset display action.

### Measured geometry and physical rendering

- Create `crates/rshell-ui/src/terminal_metrics.rs` — `FontMetricsService`, Pango sampling, cache key, fallback, and measured evidence.
- Create `crates/rshell-ui/src/terminal_view_geometry.rs` and `terminal_view_metrics.rs` — move model geometry methods and GTK invalidation wiring out of near-cap files.
- Modify `terminal_input.rs`, `terminal_geometry.rs`, `terminal_view_model.rs`, `terminal_view_message.rs`, `terminal_view.rs`, `terminal_view_widgets.rs`, `terminal_renderer.rs`, `terminal_paint.rs`, `terminal_render_cache.rs`, `pane_host_terminals.rs`, and `main_window_smoke_resize.rs` — consume one measured metric/environment value and suppress duplicate PTY resizes.
- Create `crates/rshell-ui/src/icon_registry_metadata.rs` and `icon_render.rs`; modify `icon_registry.rs`, `icon_cache.rs`, `icon_vector.rs`, `icon_vector_data.rs`, and `lib.rs` — key physical textures by icon/backend/physical size and keep metadata/rendering focused.

### Adaptive shell, modal, and overflow composition

- Create `crates/rshell-ui/src/adaptive_layout.rs` and `main_window_shell.rs`; modify `main_window.rs`, `main_window_layout.rs`, `main_window_events.rs`, `main_window_dialog_events.rs`, and `main_window_snapshots.rs` — pure width mode plus controller-preserving GTK composition.
- Create `crates/rshell-ui/src/modal_host.rs` and `modal_focus.rs`; modify editor/settings/import/interaction root, widget, render, and binding modules — shared scrim, size, fixed header/footer, scrolling body, focus containment, Escape, and return.
- Create `crates/rshell-ui/src/tab_overflow.rs`, `pane_action_layout.rs`, and `navigation_drawer.rs`; modify `session_tab_bar.rs`, `pane_host_render.rs`, connection sidebar modules, `icon_registry_metadata.rs`, and add `resources/icons/more.svg` plus `resources/icons/navigation.svg` — horizontal tab scroller/list, priority actions, and compact drawer.

### Typed smoke, QA, package, and hosted evidence

- Modify the existing `smoke_driver_*`, `main_window_smoke_*`, `visual_contract.rs`, and `visual_png.rs` focused modules; create `smoke_driver_recovery.rs`, `smoke_driver_visual_matrix.rs`, and `main_window_smoke_matrix.rs` — additive interrupt/DPI/layout/modal/accessibility evidence and multiple real screenshot checkpoints.
- Modify root `src/p0_smoke_actions.rs`, `p0_smoke_scenario.rs`, `p0_smoke_report.rs`, `p0_smoke_report_terminal.rs`, `p0_smoke_report_visual.rs`, `p0_smoke_contract.rs`, related binding/evidence modules, `tests/fixtures/smoke/p0-scenario.json`, and `tests/p0_acceptance.rs` — stable JSON mapping and fail-closed matrix assessment without weakening the eleven existing P0 surfaces.
- Modify `scripts/qa/p0-smoke.ps1`, `workflow-contract.ps1`, `assert-package.ps1`, and affected startup evidence in `crates/rshell-ui/src/startup_probe.rs`, `crates/rshell-ui/tests/startup.rs`, and `src/main.rs` — isolated evidence root, fatal GTK warnings, package resource checks, workflow bindings, cleanup, and no-secret scans.
- Verify `.github/workflows/ci.yml` and `.github/workflows/release.yml` as contract inputs; change only the exact P0/package step if a new fail-closed contract test proves the existing step cannot carry required evidence.

## Exact interface chain

1. `TerminalViewModel::key_pressed` maps safety Ctrl+C to `SessionUiCommand::Interrupt` before configured bindings or terminal encoding; Ctrl+Shift+C remains copy.
2. `SessionUiCommand::{Interrupt, ResetDisplay}` map one-for-one through `SessionPortAdapter` to `SessionCommand::{Interrupt, ResetDisplay}`; `Interrupt` writes only `[0x03]` to `SessionTransport::write`.
3. `TerminalEngine::{display_modes, recover_display}` is implemented only by `DefaultTerminalEngine -> AlacrittyAdapter`; recovery returns primary screen, clears enhanced modes/title, preserves primary text/scrollback, and never writes recovery escape bytes to the child transport.
4. `DisplayRecoveryTracker` records the interrupt generation/modes, observes the next authoritative frame, and emits `SessionUiEvent::RecoveryChanged(Option<DisplayRecoveryNotice>)`; core retains it by session and pane UI exposes `PaneAction::ResetDisplay`.
5. `FontMetricsService` samples the resolved Pango context on the GTK main thread and returns `MeasuredFontMetrics`; `TerminalViewModel` owns it and its last `TerminalSize`, so render/cursor/selection/hit-test/rows/columns/pixels/DPI share one source and duplicate resize commands are suppressed.
6. `IconCacheKey { icon, backend, physical_size }` selects SVG/internal-vector output at `ceil(logical_size * effective_scale)` physical pixels while GTK allocation remains logical.
7. `ShellLayout::for_width` is pure; `MainWindowShell::apply` reparents existing controller widgets without recreating reducer state. `ModalHost`, `TabOverflowModel`, and `PaneActionLayout` own only presentation decisions and continue to emit existing component messages.
8. Typed smoke evidence flows `SmokeAction -> MainWindow smoke route -> SmokeCounters -> SmokeReport -> root report mapper -> p0_smoke_contract -> PowerShell JSON/JUnit`, with leaf-only screenshot names and no terminal text, endpoints, paths, or secrets.

### Task 1: Make design tokens and discovery fixtures permanent

**Files:**
- Modify: `DESIGN.md`
- Modify: `resources/style.css`
- Modify: `crates/rshell-ui/tests/theme_contract.rs`
- Create: `crates/rshell-session/tests/support/display_recovery.rs`
- Modify: `crates/rshell-session/tests/support/mod.rs`
- Create: `crates/rshell-session/tests/fixtures/interrupt_tui.rs`
- Modify: `crates/rshell-session/tests/engine_contract.rs`

**Interfaces:**
- Consumes: approved spec sections 0-3, 5.1, 6, 7, and the current `DefaultTerminalEngine::input/snapshot` test boundary.
- Produces: design markers `Terminal recovery authority`, `terminal-line-spacing | 0 logical px`, `Compact < 900`, `Standard 900–1439`, `Wide >= 1440`; `display_recovery::MODE_SEQUENCE`; `display_recovery::assert_frames_equivalent`; real fixture modes `survive` and `exit_dirty`.

- [ ] **Step 1: Write the failing design/token contract**

  Extend `theme_contract.rs` with exact source/embedded checks:

  ```rust
  #[test]
  fn design_records_recovery_hidpi_and_adaptive_authority() {
      let design = include_str!("../../../DESIGN.md");
      for marker in [
          "Terminal recovery authority",
          "terminal-line-spacing | 0 logical px",
          "Compact | `< 900`",
          "Standard | `900–1439`",
          "Wide | `>= 1440`",
          "max width of `min(680px, window width - 48px)`",
          "Display mode not restored",
      ] {
          assert!(design.contains(marker), "missing design marker: {marker}");
      }
      let css = rshell_ui::embedded_theme_css();
      for marker in [
          ".modal-scrim", ".display-recovery-notice", ".compact-nav-rail",
          ".tab-overflow", ".pane-action-overflow", "font-size: 13px",
      ] {
          assert!(css.contains(marker), "missing CSS contract: {marker}");
      }
      for forbidden in ["gradient(", "box-shadow:", "backdrop-filter", "opacity: 0.4"] {
          assert!(!css.contains(forbidden), "prohibited effect: {forbidden}");
      }
  }
  ```

- [ ] **Step 2: Run the RED contract**

  Run:

  ```powershell
  cargo test -p rshell-ui --test theme_contract --locked design_records_recovery_hidpi_and_adaptive_authority -- --exact
  ```

  Expected: FAIL because the current design remains Task21 authority with fixed 232px/2px rhythm and the new adaptive/recovery selectors do not exist.

- [ ] **Step 3: Lock whole/two-chunk parser equivalence and real fixture modes**

  Define the permanent byte fixture without journal/debug dependencies:

  ```rust
  pub const MODE_SEQUENCE: &[u8] = concat!(
      "\u{1b}]0;rshell-recovery-fixture\u{7}",
      "\u{1b}[?1049h", "\u{1b}[>1u", "\u{1b}[?1000h", "\u{1b}[?1006h",
      "\u{1b}[?1h", "\u{1b}[?25l", "fixture-界-e\u{301}"
  ).as_bytes();

  pub fn two_chunk_splits(bytes: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
      (0..=bytes.len()).map(|index| bytes.split_at(index))
  }
  ```

  In `engine_contract.rs`, feed the sequence whole and at every split, then compare rows, cursor, title, alternate screen, mouse state, and replacement-character count. The test must assert zero `U+FFFD` and identical frames. Implement `interrupt_tui.rs` so `survive` enters all modes, reads raw bytes, prints `interrupt=03;survived=true`, and remains in alternate screen; `exit_dirty` exits immediately after one ETX without sending restoration sequences.

- [ ] **Step 4: Run the permanent parser/fixture checks**

  Run:

  ```powershell
  cargo test -p rshell-session --test engine_contract --locked whole_and_every_two_chunk_split_are_identical -- --exact
  cargo test -p rshell-session --test engine_contract --locked mode_fixture_contains_no_replacement_character -- --exact
  cargo test -p rshell-session --test engine_contract --locked --no-run
  ```

  Expected: PASS. These tests preserve the discovery conclusion that split parsing is not the reproduced defect; the fixture compiles but is not retained as a debug executable.

- [ ] **Step 5: Update the design authority and base CSS states**

  Replace Task21 authority text with the approved spec path and document the exact recovery distinction: Ctrl+C always sends one ETX; a surviving TUI is not reset; a dirty surviving frame shows `Display mode not restored`; terminal termination recovers automatically. Record the three breakpoints, 48/240/280px navigation widths, 4px rhythm with 2px border-only exception, 13/14px UI type, zero terminal line-spacing token, opaque scrim, 680px modal cap, fixed header/footer/scroll body, overflow rules, accessibility ratios, and screenshot metadata.

  Add CSS selectors for every required state using opaque colors and 80/100/120ms transitions only. Keep `.terminal-canvas` background fixed and do not encode terminal cell dimensions in CSS.

  Extend the contract test to require default, hover, focus, pressed, disabled, pending, success, and error selectors for each changed command/sidebar/tab/pane/modal primitive where that state applies. Reuse the existing color parser to require primary operational text >=4.5:1 and focus treatment >=3:1; reject animation durations outside 80/100/120ms and any transition that changes sensitivity or delays command dispatch. GTK's `gtk-enable-animations` setting remains authoritative because no product timer gates input.

- [ ] **Step 6: Run GREEN design and boundary checks**

  Run:

  ```powershell
  cargo test -p rshell-ui --test theme_contract --locked
  cargo test -p rshell-session --test engine_contract --locked
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  git diff --check -- DESIGN.md resources/style.css crates/rshell-ui/tests/theme_contract.rs crates/rshell-session/tests/support/display_recovery.rs crates/rshell-session/tests/support/mod.rs crates/rshell-session/tests/fixtures/interrupt_tui.rs crates/rshell-session/tests/engine_contract.rs
  ```

  Expected: all tests PASS, no production module exceeds 250 pure LOC, exactly one UI `.try_send(` site remains, and no whitespace error is reported.

- [ ] **Step 7: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  git add DESIGN.md resources/style.css crates/rshell-ui/tests/theme_contract.rs crates/rshell-session/tests/support/display_recovery.rs crates/rshell-session/tests/support/mod.rs crates/rshell-session/tests/fixtures/interrupt_tui.rs crates/rshell-session/tests/engine_contract.rs
  git commit -m "test: lock terminal recovery design fixtures" -m "Record adaptive tokens and preserve parser and PTY recovery discoveries as permanent contracts."
  ```

### Task 2: Add typed safety interrupt and deterministic engine reset

**Files:**
- Create: `crates/rshell-core/src/protocol/session.rs`
- Modify: `crates/rshell-core/src/protocol.rs`
- Modify: `crates/rshell-core/src/protocol/commands.rs`
- Modify: `crates/rshell-core/src/render.rs`
- Modify: `crates/rshell-core/tests/protocol_secrets.rs`
- Create: `crates/rshell-session/src/alacritty_display.rs`
- Create: `crates/rshell-session/src/actor_commands.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/engine.rs`
- Modify: `crates/rshell-session/src/message.rs`
- Modify: `crates/rshell-session/src/ports.rs`
- Modify: `crates/rshell-session/src/actor.rs`
- Modify: `crates/rshell-session/src/actor_io.rs`
- Modify: `crates/rshell-session/src/alacritty_adapter.rs`
- Modify: `crates/rshell-session/src/alacritty_event.rs`
- Modify: `crates/rshell-session/src/render.rs`
- Modify: `crates/rshell-session/tests/engine_contract.rs`
- Modify: `crates/rshell-session/tests/actor_lifecycle.rs`
- Modify: `crates/rshell-session/tests/ports.rs`
- Modify: `crates/rshell-ui/src/terminal_view_keys.rs`
- Modify: `crates/rshell-ui/tests/terminal_view_model.rs`

**Interfaces:**
- Consumes: `UiCommand::Session`, `SessionTransport::write(&[u8])`, `TerminalEngine`, `AlacrittyAdapter`, and the fixture from Task 1.
- Produces: `SessionUiCommand::{Interrupt, ResetDisplay}`; `SessionCommand::{Interrupt, ResetDisplay}`; `TerminalDisplayModes`; `DisplayRecovery`; `TerminalEngine::display_modes(&self) -> TerminalDisplayModes`; `TerminalEngine::recover_display(&mut self) -> Result<DisplayRecovery, EngineError>`.

- [ ] **Step 1: Write RED command, wire-byte, and engine recovery tests**

  Add the exact public contracts:

  ```rust
  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
  pub struct TerminalDisplayModes {
      pub alternate_screen: bool,
      pub enhanced_keyboard: bool,
      pub mouse_reporting: bool,
      pub application_cursor: bool,
      pub cursor_hidden: bool,
      pub stale_title: bool,
  }

  impl TerminalDisplayModes {
      pub const fn has_residue(self) -> bool {
          self.alternate_screen || self.enhanced_keyboard || self.mouse_reporting
              || self.application_cursor || self.cursor_hidden || self.stale_title
      }
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct DisplayRecovery {
      pub before: TerminalDisplayModes,
      pub after: TerminalDisplayModes,
      pub changed: bool,
  }
  ```

  Add `#[serde(default)] pub display_modes: TerminalDisplayModes` to `RenderFrame`. During the compatibility migration, `RenderFrame::alternate_screen` and `RenderFrame::mouse_reporting` remain serialized and must exactly mirror `display_modes.alternate_screen` and `display_modes.mouse_reporting`; one core protocol test rejects disagreement.

  Add tests that: (a) unmodified Ctrl+C becomes `SessionUiCommand::Interrupt`; (b) Ctrl+Shift+C emits no terminal command; (c) an actor with negotiated Kitty input writes exactly `vec![0x03]`, one write, and never `b"\x1b[99;5u"`; (d) `recover_display` after `MODE_SEQUENCE` yields all-false modes, title `rsHell`, primary text/scrollback preserved, no alternate content overlap, and zero `U+FFFD`.

- [ ] **Step 2: Run the RED tests**

  Run:

  ```powershell
  cargo test -p rshell-ui --test terminal_view_model --locked safety_ctrl_c_bypasses_configured_and_negotiated_encoding -- --exact
  cargo test -p rshell-session --test actor_lifecycle --locked interrupt_writes_exactly_one_etx -- --exact
  cargo test -p rshell-session --test engine_contract --locked display_recovery_preserves_primary_and_clears_modes -- --exact
  ```

  Expected: FAIL to compile because the typed commands, mode snapshot, and recovery operation do not exist.

- [ ] **Step 3: Split the near-cap protocol and actor command modules**

  Move `SessionUiCommand`, `SessionUiEvent`, and their redacted `Debug` implementations from `protocol/commands.rs` into `protocol/session.rs`, re-export them unchanged from `protocol.rs`, and add the two variants:

  ```rust
  pub enum SessionUiCommand {
      Interrupt,
      ResetDisplay,
      Input(TerminalInput),
      Mouse(TerminalMouseEvent),
      Paste(SecretString),
      Resize(TerminalSize),
      Scroll(i32),
      Search(SearchQuery),
      Select(SelectionRange),
      CopySelection,
      ClearScrollback,
      Respond { interaction: InteractionId, response: InteractionResponse },
      Reconnect,
      Shutdown,
  }
  ```

  Mirror the variants in `SessionCommand`, map them exhaustively in `ports::map_command`, and move `SessionActor::handle_command` into `actor_commands.rs`. Do not add another channel or UI port.

- [ ] **Step 4: Implement the unconditional ETX branch before all encoding**

  In `TerminalViewModel::resolve_key`, check the exact modifier tuple before configured bindings:

  ```rust
  if character == Some('c')
      && key_modifiers.control
      && !key_modifiers.shift
      && !key_modifiers.alt
      && !key_modifiers.super_key
  {
      return Ok(Some(self.command(SessionUiCommand::Interrupt)));
  }
  ```

  In `SessionActor::handle_command`, never call `encode_input` for interrupt:

  ```rust
  SessionCommand::Interrupt => match transport.write(&[0x03]).await {
      Ok(()) => ActorControl::Continue,
      Err(error) => ActorControl::Failure(error.failure()),
  },
  ```

  This branch is valid for local PTY, System OpenSSH PTY, and native SSH because all three already implement `SessionTransport::write`.

- [ ] **Step 5: Implement the focused Alacritty display contract**

  Add the required trait methods with no default implementation:

  ```rust
  pub trait TerminalEngine: Send {
      fn display_modes(&self) -> TerminalDisplayModes;
      fn recover_display(&mut self) -> Result<DisplayRecovery, EngineError>;
  }
  ```

  All pre-existing trait methods and signatures remain byte-for-byte unchanged around these two additions.

  `alacritty_display.rs` is the only file allowed to inspect pinned `TermMode`/keyboard-stack APIs. Compile-prove those APIs against `alacritty_terminal = 0.26.0`; return to primary screen, clear every enhanced keyboard level, mouse modes, application cursor, hidden cursor, and title, while preserving the primary grid/history and processor UTF-8 state. Internal recovery mutates the emulator only: discard emulator-generated recovery responses and never write recovery bytes to the PTY/SSH transport. `EventSink::reset_title()` sets exactly `rsHell`.

- [ ] **Step 6: Wire Reset display to the same engine operation**

  `SessionCommand::ResetDisplay` calls `engine.recover_display()`, resets actor selection/viewport to valid primary bounds, marks one frame dirty, and returns `ActorControl::Continue`; a platform error fails the session closed. Do not call this branch from `Interrupt`.

- [ ] **Step 7: Run GREEN safety and compatibility checks**

  Run:

  ```powershell
  cargo test -p rshell-core --test protocol_secrets --locked
  cargo test -p rshell-session --test engine_contract --locked
  cargo test -p rshell-session --test actor_lifecycle --locked
  cargo test -p rshell-session --test ports --locked
  cargo test -p rshell-ui --test terminal_view_model --locked
  cargo check --workspace --all-targets --all-features --locked
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  ```

  Expected: all commands PASS; ETX is one byte under Kitty negotiation; recovery is mode-clean and primary-preserving; all redacted Debug tests remain green; every new match is exhaustive.

- [ ] **Step 8: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-core/src/protocol.rs', 'crates/rshell-core/src/protocol/commands.rs',
      'crates/rshell-core/src/protocol/session.rs', 'crates/rshell-core/src/render.rs',
      'crates/rshell-core/tests/protocol_secrets.rs', 'crates/rshell-session/src/alacritty_display.rs',
      'crates/rshell-session/src/actor_commands.rs', 'crates/rshell-session/src/lib.rs',
      'crates/rshell-session/src/engine.rs', 'crates/rshell-session/src/message.rs',
      'crates/rshell-session/src/ports.rs', 'crates/rshell-session/src/actor.rs',
      'crates/rshell-session/src/actor_io.rs', 'crates/rshell-session/src/alacritty_adapter.rs',
      'crates/rshell-session/src/alacritty_event.rs', 'crates/rshell-session/src/render.rs',
      'crates/rshell-session/tests/engine_contract.rs', 'crates/rshell-session/tests/actor_lifecycle.rs',
      'crates/rshell-session/tests/ports.rs', 'crates/rshell-ui/src/terminal_view_keys.rs',
      'crates/rshell-ui/tests/terminal_view_model.rs'
  )
  git add -- @taskPaths
  git commit -m "fix: add safe terminal interrupt and display reset" -m "Bypass negotiated key encoding for Ctrl+C and recover Alacritty display modes without clearing primary history."
  ```

### Task 3: Track interruption residue and publish terminal-state recovery

**Files:**
- Create: `crates/rshell-session/src/display_recovery.rs`
- Create: `crates/rshell-session/src/actor_termination.rs`
- Modify: `crates/rshell-session/src/lib.rs`
- Modify: `crates/rshell-session/src/actor.rs`
- Modify: `crates/rshell-session/src/actor_commands.rs`
- Modify: `crates/rshell-session/src/actor_io.rs`
- Modify: `crates/rshell-session/src/manager.rs`
- Modify: `crates/rshell-session/src/message.rs`
- Modify: `crates/rshell-session/src/ports.rs`
- Modify: `crates/rshell-session/src/presentation.rs`
- Modify: `crates/rshell-session/tests/actor_lifecycle.rs`
- Modify: `crates/rshell-session/tests/ports.rs`
- Modify: `crates/rshell-core/src/protocol/session.rs`
- Modify: `crates/rshell-core/src/application/model.rs`
- Modify: `crates/rshell-core/src/application/session_events.rs`
- Modify: `crates/rshell-core/tests/application.rs`
- Create: `crates/rshell-ui/src/pane_status.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/pane_view_model.rs`
- Modify: `crates/rshell-ui/src/pane_host.rs`
- Modify: `crates/rshell-ui/src/pane_host_model.rs`
- Modify: `crates/rshell-ui/src/pane_host_render.rs`
- Modify: `crates/rshell-ui/tests/workspace_view_model.rs`
- Modify: `crates/rshell-ui/tests/application_live_view.rs`
- Modify: `crates/rshell-ui/tests/native_widgets.rs`

**Interfaces:**
- Consumes: `TerminalDisplayModes`, `DisplayRecovery`, `SessionCommand::{Interrupt, ResetDisplay, Reconnect}`, `ActorControl::Reconnect`, actor presentation generation, immutable `RenderFrame`, and current terminal lifecycle events.
- Produces: `DisplayRecoveryNotice`; `SessionEvent::RecoveryChanged(Option<DisplayRecoveryNotice>)`; `SessionUiEvent::RecoveryChanged(Option<DisplayRecoveryNotice>)`; `AppViewModel::display_recovery`; `DisplayRecoveryTracker`; `PaneAction::ResetDisplay`.

- [ ] **Step 1: Write RED tracker, termination, and pane tests**

  Define the non-secret notice at the core boundary:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub struct DisplayRecoveryNotice {
      pub interrupted_generation: u64,
      pub observed_generation: u64,
      pub modes: TerminalDisplayModes,
  }
  ```

  Tests must prove these state transitions:

  1. `Interrupt` records the current generation/modes only after one successful ETX write.
  2. The same generation cannot create a notice; the next authoritative frame with residue creates one notice.
  3. A fixture that catches ETX remains alternate/enhanced and is not auto-reset.
  4. `ResetDisplay` clears the notice and publishes a newer clean generation.
  5. exit, EOF/disconnect, transport failure, shutdown failure, and actor failure recover before terminal lifecycle publication.
  6. `SessionCommand::Reconnect -> ActorControl::Reconnect` from dirty alternate/Kitty state publishes a newer mode-clean frame and `RecoveryChanged(None)`, then shuts down the old transport, then asks the existing factory for and connects the replacement transport; the actor and the same engine remain owned throughout.
  7. actor panic cannot access the dropped engine, so the supervisor clears recovery state before `Crashed`; the pane renders only the stable crash page and never the detached terminal.
  8. recovery/disconnected pages contain no old-TUI/prompt overlap and no `U+FFFD`.

- [ ] **Step 2: Run the RED tracker and UI tests**

  Run:

  ```powershell
  cargo test -p rshell-session --test actor_lifecycle --locked interruption_notice_waits_for_a_new_authoritative_frame -- --exact
  cargo test -p rshell-session --test actor_lifecycle --locked every_terminal_completion_recovers_before_event -- --exact
  cargo test -p rshell-session --test actor_lifecycle --locked dirty_reconnect_recovers_before_old_shutdown_and_new_connect -- --exact
  cargo test -p rshell-core --test application --locked recovery_notice_is_bound_to_current_session -- --exact
  cargo test -p rshell-ui --test workspace_view_model --locked recovery_notice_and_terminal_pages_are_mutually_exclusive -- --exact
  ```

  Expected: FAIL because the tracker, event, core map, pane action, and ordered reconnect recovery path do not exist.

- [ ] **Step 3: Implement actor-owned observation and authoritative-frame detection**

  Keep tracker policy isolated:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct InterruptionObservation {
      pub generation: u64,
      pub modes: TerminalDisplayModes,
  }

  #[derive(Default)]
  pub struct DisplayRecoveryTracker {
      pending: Option<InterruptionObservation>,
      published: Option<DisplayRecoveryNotice>,
  }

  impl DisplayRecoveryTracker {
      pub fn record(&mut self, generation: u64, modes: TerminalDisplayModes);
      pub fn observe(&mut self, frame: &RenderFrame) -> RecoveryTransition;
      pub fn clear(&mut self) -> bool;
  }

  pub enum RecoveryTransition {
      Unchanged,
      Changed(Option<DisplayRecoveryNotice>),
  }
  ```

  Add `PresentationState::generation(&self) -> u64`. After `transport.write(&[0x03])` succeeds, record that generation and `engine.display_modes()`. In `publish_frame`, assign the new generation first, then call `observe`; emit `RecoveryChanged` only when the option changes. A clean next frame clears the pending observation without showing a notice.

- [ ] **Step 4: Recover before terminal lifecycle publication**

  Move `shutdown`, `exit`, `fail`, and `illegal` from `actor_io.rs` into `actor_termination.rs`. Use one shared sequence:

  ```rust
  fn prepare_final_presentation(&mut self) -> Result<u64, EngineError> {
      self.engine.recover_display()?;
      self.recovery.clear();
      self.presentation.on_display_recovery(self.engine.viewport_bounds());
      self.frame_clock.mark_dirty();
      self.publish_frame()?;
      let _ = self.events.send(SessionEvent::RecoveryChanged(None));
      Ok(self.presentation.generation())
  }
  ```

  Call it after transport completion is known and before `Lifecycle::{exit, fail, illegal}` emits the terminal event. Also call this exact helper in the `ActorControl::Reconnect` branch before `transport.shutdown().await`, `clear_stopped_child_process`, `factory.create`, or any replacement `connect`; only after the clean generation/event is published may the old transport shut down and the existing actor create/connect the replacement. Reconnect does not replace the actor or engine, does not re-read/mutate credentials beyond existing factory semantics, and does not change authentication, host-key, or reconnect security policy. If recovery fails, publish `SessionFailure::Platform`, shut down the old transport, and do not create a replacement. In the manager panic supervisor, emit `RecoveryChanged(None)` before `StateChanged(Crashed)`/`Crashed`; do not manufacture a fake terminal frame from a dropped engine.

  The reconnect regression uses two ordered recording transports and a recording factory. Its exact event ledger must be `clean_frame(new_generation)`, `RecoveryChanged(None)`, `old_transport.shutdown`, `factory.create_replacement`, `new_transport.connect`; it also asserts the frame has all display modes false, no stale title/U+FFFD, and that no replacement connect occurs before old shutdown succeeds.

- [ ] **Step 5: Retain notice identity in core and reject stale session events**

  Add:

  ```rust
  pub display_recovery: BTreeMap<SessionId, DisplayRecoveryNotice>,
  ```

  `application/session_events.rs` inserts only for the currently bound session, removes on `None`, exit/failure/crash/retry/unbind, and ignores a notice from a replaced session. Preserve the current `latest_frames` generation filter and `error_panes` behavior.

- [ ] **Step 6: Render one pane layer and expose Reset display**

  Extract page/status/action policy into `pane_status.rs` to keep `pane_view_model.rs` under cap. Extend actions:

  ```rust
  pub enum PaneAction {
      ResetDisplay,
      SplitHorizontal,
      SplitVertical,
      Reconnect,
      Retry,
      EditConnection,
      CopyDiagnostics,
      Close,
  }
  ```

  `PaneAction::ResetDisplay.command(pane, Some(session))` returns `UiCommand::Session { session, command: SessionUiCommand::ResetDisplay }`. A connected pane with a notice renders terminal plus one compact `.display-recovery-notice` row containing exact text `Display mode not restored` and an accessible `Reset display` button. Exited/failed/crashed/disconnected status replaces the terminal widget entirely. Retry, Copy diagnostics, Edit connection when applicable, Reset display when residue exists, and Close are available on the terminal-state page; detached terminal controllers receive no further messages.

- [ ] **Step 7: Run GREEN lifecycle and pane checks**

  Run:

  ```powershell
  cargo test -p rshell-session --test actor_lifecycle --locked
  cargo test -p rshell-session --test ports --locked
  cargo test -p rshell-core --test application --locked
  cargo test -p rshell-ui --test workspace_view_model --locked
  cargo test -p rshell-ui --test application_live_view --locked
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test native_widgets --locked -- --nocapture; $nativeExit = $LASTEXITCODE } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  ```

  Expected: all tests PASS; surviving TUI remains active until explicit reset; every terminal completion and actor reconnect leaves the old presentation clean; reconnect ordering is clean frame/event -> old shutdown -> replacement creation/connect while retaining actor/engine ownership; stale session notices are rejected; GTK emits no warning.

- [ ] **Step 8: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-session/src/display_recovery.rs', 'crates/rshell-session/src/actor_termination.rs',
      'crates/rshell-session/src/lib.rs', 'crates/rshell-session/src/actor.rs',
      'crates/rshell-session/src/actor_commands.rs', 'crates/rshell-session/src/actor_io.rs',
      'crates/rshell-session/src/manager.rs', 'crates/rshell-session/src/message.rs',
      'crates/rshell-session/src/ports.rs', 'crates/rshell-session/src/presentation.rs',
      'crates/rshell-session/tests/actor_lifecycle.rs', 'crates/rshell-session/tests/ports.rs',
      'crates/rshell-core/src/protocol/session.rs', 'crates/rshell-core/src/application/model.rs',
      'crates/rshell-core/src/application/session_events.rs', 'crates/rshell-core/tests/application.rs',
      'crates/rshell-ui/src/pane_status.rs', 'crates/rshell-ui/src/lib.rs',
      'crates/rshell-ui/src/pane_view_model.rs', 'crates/rshell-ui/src/pane_host.rs',
      'crates/rshell-ui/src/pane_host_model.rs', 'crates/rshell-ui/src/pane_host_render.rs',
      'crates/rshell-ui/tests/workspace_view_model.rs', 'crates/rshell-ui/tests/application_live_view.rs',
      'crates/rshell-ui/tests/native_widgets.rs'
  )
  git add -- @taskPaths
  git commit -m "fix: recover terminal presentation on completion" -m "Track post-interrupt residue, offer explicit display reset, and detach cleanly on every terminal state."
  ```

### Task 4: Replace fixed cells with one measured geometry source

**Files:**
- Create: `crates/rshell-ui/src/terminal_metrics.rs`
- Create: `crates/rshell-ui/src/terminal_view_geometry.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/terminal_input.rs`
- Modify: `crates/rshell-ui/src/terminal_geometry.rs`
- Modify: `crates/rshell-ui/src/terminal_view_model.rs`
- Modify: `crates/rshell-ui/src/terminal_view_message.rs`
- Modify: `crates/rshell-ui/src/terminal_renderer.rs`
- Modify: `crates/rshell-ui/src/terminal_paint.rs`
- Modify: `crates/rshell-ui/src/pane_host_terminals.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_resize.rs`
- Modify: `crates/rshell-ui/tests/terminal_view_model.rs`
- Modify: `crates/rshell-ui/tests/terminal_draw.rs`
- Create: `crates/rshell-ui/tests/terminal_metrics.rs`

**Interfaces:**
- Consumes: GTK-main-thread `pango::Context`, `ResolvedTerminalProfile`, logical allocation, effective scale/DPI, immutable render cells, and existing `TerminalSize`.
- Produces: `FontMetricSample`; `FontMetricEnvironment`; `FontMetricKey`; `MeasuredFontMetrics`; `MetricsChange`; `FontMetricsService`; `TerminalGeometryInput`; `TerminalViewModel::apply_metrics`; duplicate-resize suppression.

- [ ] **Step 1: Write RED pure metric/geometry and real Pango tests**

  Define exact value types:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct FontMetricSample {
      pub approximate_char_width: f64,
      pub ascii_advance: f64,
      pub ascent: f64,
      pub descent: f64,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct FontMetricEnvironment {
      pub effective_scale: f64,
      pub effective_dpi: f64,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct FontMetricKey {
      pub family: String,
      pub font_size_bits: u32,
      pub effective_scale_bits: u64,
      pub effective_dpi_bits: u64,
      pub color_scheme: ColorScheme,
  }

  #[derive(Debug, Clone, PartialEq)]
  pub struct MeasuredFontMetrics {
      pub metrics: FontMetrics,
      pub key: FontMetricKey,
      pub environment: FontMetricEnvironment,
      pub fallback_used: bool,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct TerminalGeometryInput {
      pub logical_width: i32,
      pub logical_height: i32,
      pub metrics: FontMetrics,
      pub environment: FontMetricEnvironment,
  }
  ```

  Pure tests iterate effective DPI `[96.0, 120.0, 144.0, 192.0]` and font sizes `[6.0, 15.0, 72.0]`; assert positive finite ceil-rounded metrics, exact rows/columns, physical pixels derived from effective scale, and exact integer DPI. Native Pango tests measure ASCII, combining text, CJK, and emoji fallback, then assert wide cells occupy exactly two grid cells and fallback advance never changes column identity.

- [ ] **Step 2: Run RED metric tests**

  Run:

  ```powershell
  cargo test -p rshell-ui --test terminal_metrics --locked -- --nocapture
  cargo test -p rshell-ui --test terminal_view_model --locked geometry_matrix_uses_one_measured_source -- --exact
  ```

  Expected: FAIL because `terminal_font_metrics()` is still fixed at 9×18 and the measurement/environment interfaces do not exist.

- [ ] **Step 3: Implement Pango sampling and fail-closed fallback**

  `FontMetricsService` is GTK-main-thread only and caches one measurement:

  ```rust
  pub struct FontMetricsService {
      current: Option<MeasuredFontMetrics>,
  }

  impl FontMetricsService {
      pub fn measure(
          &mut self,
          context: &gtk::pango::Context,
          profile: &ResolvedTerminalProfile,
          environment: FontMetricEnvironment,
      ) -> Result<MetricsChange, TerminalViewError>;
  }

  pub enum MetricsChange {
      Unchanged(MeasuredFontMetrics),
      Changed(MeasuredFontMetrics),
  }
  ```

  Build `FontMetricKey` from exact font family, `font_size.to_bits()`, effective scale/DPI bits, and color-scheme rendering identity. Use the context's resolved monospace metrics and a Pango layout for ASCII `M`; `cell_width = ceil(max(approximate_char_width, ascii_advance))`, `cell_height = ceil(ascent + descent + 0.0)`. If Pango cannot resolve usable positive finite values, measure GTK's resolved `Monospace 10` in the same context, set `fallback_used = true`, and fail if that is also invalid. No numeric 9×18 fallback remains.

- [ ] **Step 4: Make the view model the sole metrics/size owner**

  Move cursor, resize, mouse, and selection methods to `terminal_view_geometry.rs`. Replace `resize(&self) -> UiCommand` with stateful suppression:

  ```rust
  pub fn apply_geometry(
      &mut self,
      input: TerminalGeometryInput,
  ) -> Result<Option<UiCommand>, TerminalViewError>;

  pub fn apply_metrics(
      &mut self,
      measured: MeasuredFontMetrics,
      allocation: Option<(i32, i32)>,
  ) -> Result<Option<UiCommand>, TerminalViewError>;
  ```

  Store `MeasuredFontMetrics`, last logical allocation, and last emitted `TerminalSize`. Emit `SessionUiCommand::Resize` only when the complete `TerminalSize` changes. Renderer, cursor rectangle, selection, pointer hit-test, IME rectangle, rows/columns, and physical pixel dimensions all read `model.metrics()`; no caller creates an independent `FontMetrics`.

- [ ] **Step 5: Constrain fallback glyph painting to grid rectangles**

  In `terminal_paint.rs`, create a Pango layout for the cell text but clip to `cell.width * metrics.cell_width` by `metrics.cell_height`, center narrower fallback glyphs, and center/clip wider fallback glyphs without moving the next cell origin. Combining marks stay in the base cell; cells with width `2` get exactly two rectangles. Cursor and selection use the same rectangles.

- [ ] **Step 6: Remove every product fixed-cell source**

  Delete `terminal_font_metrics()`, remove `FontMetrics::default()`'s 9×18 values, and require measured metrics in `TerminalViewInit`. Update smoke resize to consume the active terminal's measured evidence rather than reconstructing geometry. Tests may construct explicit `FontMetrics::new(...)`; product code may not contain `9.0, 18.0`, `/ 9`, or `/ 18` cell assumptions.

- [ ] **Step 7: Run GREEN metric/render checks**

  Run:

  ```powershell
  cargo test -p rshell-ui --test terminal_metrics --locked -- --nocapture
  cargo test -p rshell-ui --test terminal_view_model --locked
  cargo test -p rshell-ui --test terminal_draw --locked
  rg "terminal_font_metrics|FontMetrics::default|9\.0, 18\.0|/ 9|/ 18" crates/rshell-ui/src
  if ($LASTEXITCODE -eq 0) { throw "Fixed product terminal geometry remains." }
  cargo check --workspace --all-targets --all-features --locked
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  ```

  Expected: all tests PASS; `rg` finds no product fixed-cell source; 96/120/144/192 DPI and 6/15/72 font matrices have binary expected dimensions; fallback glyphs, cursor, selection, and hit-testing remain inside assigned cells.

- [ ] **Step 8: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-ui/src/terminal_metrics.rs', 'crates/rshell-ui/src/terminal_view_geometry.rs',
      'crates/rshell-ui/src/lib.rs', 'crates/rshell-ui/src/terminal_input.rs',
      'crates/rshell-ui/src/terminal_geometry.rs', 'crates/rshell-ui/src/terminal_view_model.rs',
      'crates/rshell-ui/src/terminal_view_message.rs', 'crates/rshell-ui/src/terminal_renderer.rs',
      'crates/rshell-ui/src/terminal_paint.rs', 'crates/rshell-ui/src/pane_host_terminals.rs',
      'crates/rshell-ui/src/main_window_smoke_resize.rs', 'crates/rshell-ui/tests/terminal_view_model.rs',
      'crates/rshell-ui/tests/terminal_draw.rs', 'crates/rshell-ui/tests/terminal_metrics.rs'
  )
  git add -- @taskPaths
  git commit -m "fix: measure terminal geometry with Pango" -m "Drive rendering, input, and PTY sizing from one invalidatable measured cell metric."
  ```

### Task 5: Invalidate render state and render scale-aware product icons

**Files:**
- Create: `crates/rshell-ui/src/terminal_view_metrics.rs`
- Create: `crates/rshell-ui/src/icon_registry_metadata.rs`
- Create: `crates/rshell-ui/src/icon_render.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/terminal_view.rs`
- Modify: `crates/rshell-ui/src/terminal_view_message.rs`
- Modify: `crates/rshell-ui/src/terminal_view_widgets.rs`
- Modify: `crates/rshell-ui/src/terminal_render_cache.rs`
- Modify: `crates/rshell-ui/src/pane_host.rs`
- Modify: `crates/rshell-ui/src/pane_host_terminals.rs`
- Modify: `crates/rshell-ui/src/icon_registry.rs`
- Modify: `crates/rshell-ui/src/icon_cache.rs`
- Modify: `crates/rshell-ui/src/icon_vector.rs`
- Modify: `crates/rshell-ui/src/icon_vector_data.rs`
- Modify: `crates/rshell-ui/src/main_window_layout.rs`
- Modify: `crates/rshell-ui/src/connection_sidebar_widgets.rs`
- Modify: `crates/rshell-ui/src/connection_editor_widgets.rs`
- Modify: `crates/rshell-ui/src/session_tab_bar.rs`
- Modify: `crates/rshell-ui/src/pane_host_render.rs`
- Modify: `crates/rshell-ui/src/import_dialog_render.rs`
- Modify: `crates/rshell-ui/src/interaction_dialog_render.rs`
- Modify: `crates/rshell-ui/src/startup_probe.rs`
- Modify: `crates/rshell-ui/src/visual_contract.rs`
- Modify: `crates/rshell-ui/tests/icon_registry.rs`
- Modify: `crates/rshell-ui/tests/terminal_metrics.rs`
- Modify: `crates/rshell-ui/tests/native_visual_contract.rs`
- Modify: `crates/rshell-ui/tests/component_dependencies.rs`

**Interfaces:**
- Consumes: `FontMetricsService`, resolved terminal profile changes, GTK scale/settings notifications, existing embedded SVG/internal-vector data, and current visual facts.
- Produces: `TerminalViewMsg::RefreshMetrics(FontMetricEnvironment)`; `IconCacheKey`; `IconRenderRequest`; physical texture evidence; deterministic cache/render invalidation without session recreation.

- [ ] **Step 1: Write RED invalidation and physical icon tests**

  Define exact icon interfaces:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  pub struct IconCacheKey {
      pub icon: ProductIcon,
      pub backend: IconBackend,
      pub physical_size: u16,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct IconRenderRequest {
      pub logical_size: u16,
      pub effective_scale: f64,
  }

  impl IconRenderRequest {
      pub fn physical_size(self) -> Result<u16, IconRenderError>;
  }
  ```

  Test scales `[1.0, 1.25, 1.5, 2.0]` produce physical sizes `[16, 20, 24, 32]`, both backends return textures with those physical dimensions, cache entries differ by backend/size, and GTK `Image::pixel_size()` remains 16. Add a terminal test proving font, scale, effective DPI, and color-scheme identity each invalidate metrics/cache once, emit at most one changed resize, and retain the same session/controller.

- [ ] **Step 2: Run RED invalidation/icon tests**

  Run:

  ```powershell
  cargo test -p rshell-ui --test icon_registry --locked physical_icon_cache_keys_scale_and_backend -- --exact --nocapture
  cargo test -p rshell-ui --test terminal_metrics --locked metric_identity_invalidates_without_recreating_session -- --exact
  ```

  Expected: FAIL because icon cache is keyed only by `ProductIcon` and no explicit metric environment refresh exists.

- [ ] **Step 3: Wire GTK invalidation to one measurement refresh path**

  In `terminal_view_metrics.rs`, connect `DrawingArea::notify::scale-factor`, realized Pango context changes, and `gtk::Settings` effective font-DPI notification to `TerminalViewMsg::RefreshMetrics`. `PaneHostMsg::SetViewModel` sends the new resolved profile to an existing terminal controller; it does not recreate that controller when session identity is unchanged. Every signal calls `FontMetricsService::measure`; `Unchanged` does nothing, `Changed` updates the renderer/cache/model and calls `apply_metrics` once.

  Query pinned GTK/Pango APIs in this isolated module and compile-prove them locally. If effective GTK font DPI is unavailable, use `96.0 * widget.scale_factor()` and record `dpi_fallback_used`; do not claim a fractional physical scale that GTK did not expose.

- [ ] **Step 4: Implement physical-size rendering and cache keys**

  Split icon metadata from rendering. Replace the fixed APIs with:

  ```rust
  impl ProductIcon {
      pub fn decode_texture(self, request: IconRenderRequest)
          -> Result<gtk::gdk::Texture, IconRenderError>;
      pub fn image(self, request: IconRenderRequest)
          -> Result<gtk::Image, IconRenderError>;
      pub fn button(self, label: Option<&str>, request: IconRenderRequest)
          -> Result<gtk::Button, IconRenderError>;
  }
  ```

  SVG loading scales into the requested physical pixbuf; internal vectors create a Cairo surface at physical size and apply the same coordinate scale. Validate native snapshot at physical dimensions, set GTK image allocation to logical size, and cache only by `IconCacheKey`. Malformed SVG remains an error; only loader unavailability selects internal vectors.

  Update every product-icon call site listed in this task to pass the current render request. On initial unrealized construction use the widget's reported scale, then replace its paintable when Task 5's scale/DPI invalidation fires; no call site is allowed to retain an implicit scale-1 wrapper.

- [ ] **Step 5: Add non-secret physical evidence**

  Extend visual facts with logical icon size, texture width/height, backend, effective scale bits, effective DPI, measured cell width/height bits, and whether DPI fallback was used. These are numeric/closed-enum fields only. A logical PNG dimension is never used as proof of icon sharpness.

- [ ] **Step 6: Run GREEN invalidation/native rendering checks**

  Run:

  ```powershell
  cargo test -p rshell-ui --test terminal_metrics --locked
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test icon_registry --locked -- --nocapture; if ($LASTEXITCODE -ne 0) { $nativeExit = $LASTEXITCODE } else { cargo test -p rshell-ui --test native_visual_contract --locked -- --nocapture; $nativeExit = $LASTEXITCODE } } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  cargo test -p rshell-ui --test component_dependencies --locked
  cargo test --locked --test production_module_limits
  ```

  Expected: all tests PASS; 16 logical pixels use 16/20/24/32 physical texture pixels at the four evidence scales; invalidation sends one changed resize only; no session/controller identity changes; GTK emits no warning.

- [ ] **Step 7: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-ui/src/terminal_view_metrics.rs', 'crates/rshell-ui/src/icon_registry_metadata.rs',
      'crates/rshell-ui/src/icon_render.rs', 'crates/rshell-ui/src/lib.rs',
      'crates/rshell-ui/src/terminal_view.rs', 'crates/rshell-ui/src/terminal_view_message.rs',
      'crates/rshell-ui/src/terminal_view_widgets.rs', 'crates/rshell-ui/src/terminal_render_cache.rs',
      'crates/rshell-ui/src/pane_host.rs', 'crates/rshell-ui/src/pane_host_terminals.rs',
      'crates/rshell-ui/src/icon_registry.rs', 'crates/rshell-ui/src/icon_cache.rs',
      'crates/rshell-ui/src/icon_vector.rs', 'crates/rshell-ui/src/icon_vector_data.rs',
      'crates/rshell-ui/src/main_window_layout.rs', 'crates/rshell-ui/src/connection_sidebar_widgets.rs',
      'crates/rshell-ui/src/connection_editor_widgets.rs', 'crates/rshell-ui/src/session_tab_bar.rs',
      'crates/rshell-ui/src/pane_host_render.rs', 'crates/rshell-ui/src/import_dialog_render.rs',
      'crates/rshell-ui/src/interaction_dialog_render.rs', 'crates/rshell-ui/src/startup_probe.rs',
      'crates/rshell-ui/src/visual_contract.rs', 'crates/rshell-ui/tests/icon_registry.rs',
      'crates/rshell-ui/tests/terminal_metrics.rs', 'crates/rshell-ui/tests/native_visual_contract.rs',
      'crates/rshell-ui/tests/component_dependencies.rs'
  )
  git add -- @taskPaths
  git commit -m "fix: invalidate HiDPI terminal rendering" -m "Refresh measured metrics and cache embedded icons by backend and physical target size."
  ```

### Task 6: Introduce pure adaptive modes and terminal-first shell hierarchy

**Files:**
- Create: `crates/rshell-ui/src/adaptive_layout.rs`
- Create: `crates/rshell-ui/src/main_window_shell.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/main_window.rs`
- Modify: `crates/rshell-ui/src/main_window_layout.rs`
- Modify: `crates/rshell-ui/src/main_window_events.rs`
- Modify: `crates/rshell-ui/src/main_window_snapshots.rs`
- Modify: `crates/rshell-ui/src/connection_sidebar.rs`
- Modify: `crates/rshell-ui/src/connection_sidebar_widgets.rs`
- Modify: `resources/style.css`
- Create: `crates/rshell-ui/tests/adaptive_layout.rs`
- Modify: `crates/rshell-ui/tests/native_visual_contract.rs`
- Modify: `crates/rshell-ui/tests/application_live_view.rs`
- Modify: `crates/rshell-ui/tests/task18_native_widgets.rs`
- Modify: `crates/rshell-ui/tests/component_dependencies.rs`

**Interfaces:**
- Consumes: main-window logical allocation, existing Sidebar/TabBar/PaneHost controllers, current `AppViewModel`, and Task 1 tokens.
- Produces: `ShellLayoutMode`; `ShellLayout`; `ShellLayout::for_width`; closed `ShellChildOwner`; `MainWindowMsg::Allocated`; `MainWindowShell::{detach_sidebar, attach_sidebar, apply}`; command bar with one identity owner and no global status strip.

- [ ] **Step 1: Write RED pure breakpoint and preservation tests**

  Define the pure model:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ShellLayoutMode { Compact, Standard, Wide }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct ShellLayout {
      pub mode: ShellLayoutMode,
      pub navigation_width: i32,
      pub sidebar_overlay: bool,
      pub text_global_actions: bool,
      pub pane_actions_compact: bool,
  }

  impl ShellLayout {
      pub const fn for_width(width: i32) -> Self;
  }
  ```

  Assert widths 1/800/899 are Compact with 48px rail, 900/1360/1439 are Standard with 240px sidebar, and 1440/1920 are Wide with 280px maximum. Native tests cross Compact -> Standard -> Wide -> Compact under `G_DEBUG=fatal-warnings` and compare active tab, active pane, terminal session IDs, search state, selection, unsaved editor draft revision/fields, and focused pane before/after. A source contract rejects `.unparent(` and `WidgetExt::unparent` in `main_window_shell.rs`, `main_window_layout.rs`, and every present shell-owner module; its fixed candidate list includes Task 8's `navigation_drawer.rs` when that file is created, while a not-yet-created candidate is skipped rather than failing Task 6 RED/GREEN.

- [ ] **Step 2: Run RED layout tests**

  Run:

  ```powershell
  cargo test -p rshell-ui --test adaptive_layout --locked
  cargo test -p rshell-ui --test application_live_view --locked breakpoint_crossing_preserves_controller_and_reducer_identity -- --exact
  cargo test -p rshell-ui --test component_dependencies --locked production_shell_forbids_generic_widget_unparent -- --exact
  ```

  Expected: FAIL because no pure mode or allocation-driven shell host exists.

- [ ] **Step 3: Implement allocation-driven composition without controller recreation**

  `MainWindowShell` owns GTK containers only:

  ```rust
  pub struct MainWindowShell {
      pub overlay: gtk::Overlay,
      pub background: gtk::Box,
      pub command_status: gtk::Label,
      pub navigation_host: gtk::Box,
      pub terminal_workspace: gtk::Box,
      pub workspace_paned: gtk::Paned,
      pub drawer_overlay: gtk::Overlay,
      sidebar_owner: ShellChildOwner,
      current: ShellLayout,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum ShellChildOwner {
      Detached,
      WorkspacePanedStart,
      DrawerOverlay,
  }

  impl MainWindowShell {
      fn detach_sidebar(&mut self, sidebar: &gtk::Widget);
      fn attach_sidebar(&mut self, owner: ShellChildOwner, sidebar: &gtk::Widget);
      pub fn apply(&mut self, layout: ShellLayout, sidebar: &gtk::Widget);
      pub fn set_status(&self, text: &str);
      pub fn layout(&self) -> ShellLayout;
  }
  ```

  Connect the root allocation to `MainWindowMsg::Allocated { width }`. On mode change, move the existing sidebar widget between `workspace_paned` (Standard/Wide) and `drawer_overlay` (Compact); never relaunch a Relm4 controller or replace `AppViewModel`. `detach_sidebar` must match the stored owner exactly: `Detached` performs no GTK call, `WorkspacePanedStart` verifies `start_child() == Some(sidebar)` then calls `workspace_paned.set_start_child(None)`, and `DrawerOverlay` verifies the widget is an overlay child then calls `drawer_overlay.remove_overlay(sidebar)`. It then stores `Detached`. `attach_sidebar` is legal only from `Detached`; it calls `workspace_paned.set_start_child(Some(sidebar))` or `drawer_overlay.add_overlay(sidebar)` and stores the new owner. A mismatched owner/child fails closed before attachment. Generic `WidgetExt::unparent` is forbidden in production shell code.

  `MainWindowShell::apply` always performs the typed removal before typed attachment, retains active widget focus where possible, and restores focused pane after GTK allocation settles. No widget may have two GTK parents at any point.

  Standard mode starts at 240px but remains user-resizable; Wide preserves the user's stored logical width while clamping it to 280px. Compact uses 48px and does not overwrite the stored standard/wide width.

- [ ] **Step 4: Correct command and status hierarchy**

  The native title bar remains the sole `rsHell` identity. Remove `.command-bar-identity` and the 20px bottom `.status-bar`. Build command bar actions in order: New session, Import, Settings, flexible spacer, concise `command_status`. Standard/wide use icon+text for New session/Import/Settings; compact uses accessible 16px icon buttons. Global actions emit existing component messages/`UiCommand::NewLocalTab` through `MainWindow::dispatch`; status remains presentation text and is never a second command path.

- [ ] **Step 5: Prove five representative recursive pane layouts remain allocatable**

  Keep product `PaneTree` recursive; do not add a second product layout enum. In tests, define fixture names `Single`, `HSplit`, `VSplit`, `TopBottom3`, and `Grid` that construct exact one-, two-, three-, and four-leaf recursive trees. Realize each fixture at 800×600, 1360×860, and 1920×1080; assert every leaf terminal allocation is positive and every split/close/reconnect control has an accessible name. This binds the five acceptance layouts without replacing arbitrary binary splits.

- [ ] **Step 6: Run GREEN adaptive/native checks**

  Run:

  ```powershell
  cargo test -p rshell-ui --test adaptive_layout --locked
  cargo test -p rshell-ui --test application_live_view --locked
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test native_visual_contract --locked breakpoint_crossing_uses_typed_detach_without_gtk_warning -- --exact --nocapture; if ($LASTEXITCODE -ne 0) { $nativeExit = $LASTEXITCODE } else { cargo test -p rshell-ui --test task18_native_widgets --locked adaptive_shell_ -- --nocapture; $nativeExit = $LASTEXITCODE } } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  cargo test -p rshell-ui --test component_dependencies --locked production_shell_forbids_generic_widget_unparent -- --exact
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  ```

  Expected: all tests PASS; exact breakpoint decisions hold; every crossing uses the closed owner ledger and typed GTK removal before attachment; generic `unparent` is absent; fatal warnings remain clean; no duplicate identity/status strip exists; all five pane fixtures have non-zero terminals in all modes; sessions and unsaved state survive crossings.

- [ ] **Step 7: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  git add crates/rshell-ui/src/adaptive_layout.rs crates/rshell-ui/src/main_window_shell.rs crates/rshell-ui/src/lib.rs crates/rshell-ui/src/main_window.rs crates/rshell-ui/src/main_window_layout.rs crates/rshell-ui/src/main_window_events.rs crates/rshell-ui/src/main_window_snapshots.rs crates/rshell-ui/src/connection_sidebar.rs crates/rshell-ui/src/connection_sidebar_widgets.rs resources/style.css crates/rshell-ui/tests/adaptive_layout.rs crates/rshell-ui/tests/native_visual_contract.rs crates/rshell-ui/tests/application_live_view.rs crates/rshell-ui/tests/task18_native_widgets.rs crates/rshell-ui/tests/component_dependencies.rs
  git commit -m "feat: add adaptive terminal-first shell" -m "Select compact, standard, and wide GTK composition without recreating session or editor state."
  ```

### Task 7: Compose all editors and dialogs through one modal host

**Files:**
- Create: `crates/rshell-ui/src/modal_host.rs`
- Create: `crates/rshell-ui/src/modal_focus.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/main_window.rs`
- Modify: `crates/rshell-ui/src/main_window_layout.rs`
- Modify: `crates/rshell-ui/src/main_window_dialogs.rs`
- Modify: `crates/rshell-ui/src/main_window_dialog_events.rs`
- Modify: `crates/rshell-ui/src/connection_editor.rs`
- Modify: `crates/rshell-ui/src/connection_editor_widgets.rs`
- Modify: `crates/rshell-ui/src/connection_editor_render.rs`
- Modify: `crates/rshell-ui/src/connection_editor_bindings.rs`
- Modify: `crates/rshell-ui/src/settings_window.rs`
- Modify: `crates/rshell-ui/src/settings_window_widgets.rs`
- Modify: `crates/rshell-ui/src/settings_window_render.rs`
- Modify: `crates/rshell-ui/src/import_dialog.rs`
- Modify: `crates/rshell-ui/src/import_dialog_widgets.rs`
- Modify: `crates/rshell-ui/src/import_dialog_render.rs`
- Modify: `crates/rshell-ui/src/interaction_dialog.rs`
- Modify: `crates/rshell-ui/src/interaction_dialog_widgets.rs`
- Modify: `crates/rshell-ui/src/interaction_dialog_render.rs`
- Modify: `resources/style.css`
- Create: `crates/rshell-ui/tests/modal_host.rs`
- Modify: `crates/rshell-ui/tests/task18_native_widgets.rs`

**Interfaces:**
- Consumes: `MainWindowShell::overlay/background`, existing editor/settings/import/interaction component messages, and focused GTK widget at open time.
- Produces: `ModalKind`; `ModalRequest`; `ModalHost`; `ModalFocusSession`; `MainWindowMsg::Modal(ModalRequest)`; exact focus/sensitivity/geometry contract shared by all four surfaces.

- [ ] **Step 1: Write RED pure/native modal behavior tests**

  Define the host boundary:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ModalKind { ConnectionEditor, Settings, Import, Interaction }

  pub enum ModalRequest {
      Open { kind: ModalKind, trigger: gtk::Widget },
      Close(ModalKind),
  }

  pub struct ModalFocusSession {
      trigger: gtk::glib::WeakRef<gtk::Widget>,
      fallback: gtk::glib::WeakRef<gtk::Widget>,
      first: gtk::glib::WeakRef<gtk::Widget>,
      last: gtk::glib::WeakRef<gtk::Widget>,
  }

  impl ModalFocusSession {
      pub fn contain_tab(&self, backwards: bool) -> gtk::glib::Propagation;
      pub fn restore(self);
  }

  pub struct ModalHost {
      overlay: gtk::Overlay,
      scrim: gtk::Box,
      background: gtk::Widget,
      open: Option<ModalKind>,
      focus: Option<ModalFocusSession>,
  }

  impl ModalHost {
      pub fn open(&mut self, kind: ModalKind, surface: &gtk::Widget, trigger: &gtk::Widget);
      pub fn resize(&self, window_width: i32);
      pub fn close(&mut self, kind: ModalKind);
      pub fn open_kind(&self) -> Option<ModalKind>;
  }
  ```

  For every kind, native tests assert opaque mapped scrim, background `is_sensitive() == false`, width `min(680, window_width - 48)` at 800/1360/1920, fixed header/footer, scrollable body, initial focus, Tab/Shift+Tab containment, Escape using the component's cancel/close reducer, trigger focus return, and no command on cancel. Editor draft/secret wiping and interaction security assertions remain unchanged.

- [ ] **Step 2: Run RED modal tests**

  Run:

  ```powershell
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test modal_host --locked -- --nocapture; $nativeExit = $LASTEXITCODE } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  ```

  Expected: FAIL because the editor is still a 560px in-flow sibling and there is no common scrim/focus coordinator.

- [ ] **Step 3: Build the shared overlay stack and focus session**

  Install children in z-order: background shell, scrim, editor, settings, import, interaction. Scrim is opaque, visible only while a modal is open, can target pointer events, and has no dismiss-on-click behavior. Opening records a `glib::WeakRef<gtk::Widget>` to the current trigger, disables the whole background, shows only the requested surface, focuses its declared first control, and installs a capture-phase key controller. Tab from the last focusable wraps to the first; Shift+Tab wraps in reverse. Closing removes the controller, hides surface/scrim, reenables background, and focuses the live trigger or terminal canvas fallback.

- [ ] **Step 4: Recompose each dialog as header/body/footer**

  Remove fixed 560/620/520 width requests. Each component exposes a root `.content-dialog`, a non-scrolling header, `gtk::ScrolledWindow` body with grouped sections, and non-scrolling action footer. Editor groups Identity, Transport, Authentication, and Terminal overrides. Settings groups Application and Active terminal profile. Import groups Source, Preview, and Result. Interaction groups Trust/auth message, required inputs, and actions. Keep native fields, reducers, pending/disabled states, changed-host-key rejection-only semantics, and immediate secret clearing.

- [ ] **Step 5: Route open/close without adding command egress**

  Sidebar editor output and command-bar settings/import messages first capture the trigger and call `ModalHost::open`, then send existing component open messages. Interaction events open the interaction modal. Every component `Closed` output calls `ModalHost::close`; accepted commands still route through `MainWindow::dispatch`. Escape invokes `Cancel`/`Close` on the active component, never directly mutates its draft.

- [ ] **Step 6: Run GREEN modal/security/accessibility checks**

  Run:

  ```powershell
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test modal_host --locked -- --nocapture; if ($LASTEXITCODE -ne 0) { $nativeExit = $LASTEXITCODE } else { cargo test -p rshell-ui --test task18_native_widgets --locked -- --nocapture; $nativeExit = $LASTEXITCODE } } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  cargo test -p rshell-ui --test component_dependencies --locked
  cargo test --locked --test production_module_limits
  ```

  Expected: all tests PASS; all four surfaces share the exact modal contract; background input cannot dispatch; focus is contained/restored; secure-field wiping and changed-host-key behavior remain green; GTK emits no warning.

- [ ] **Step 7: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-ui/src/modal_host.rs', 'crates/rshell-ui/src/modal_focus.rs',
      'crates/rshell-ui/src/lib.rs', 'crates/rshell-ui/src/main_window.rs',
      'crates/rshell-ui/src/main_window_layout.rs', 'crates/rshell-ui/src/main_window_dialogs.rs',
      'crates/rshell-ui/src/main_window_dialog_events.rs', 'crates/rshell-ui/src/connection_editor.rs',
      'crates/rshell-ui/src/connection_editor_widgets.rs', 'crates/rshell-ui/src/connection_editor_render.rs',
      'crates/rshell-ui/src/connection_editor_bindings.rs', 'crates/rshell-ui/src/settings_window.rs',
      'crates/rshell-ui/src/settings_window_widgets.rs', 'crates/rshell-ui/src/settings_window_render.rs',
      'crates/rshell-ui/src/import_dialog.rs', 'crates/rshell-ui/src/import_dialog_widgets.rs',
      'crates/rshell-ui/src/import_dialog_render.rs', 'crates/rshell-ui/src/interaction_dialog.rs',
      'crates/rshell-ui/src/interaction_dialog_widgets.rs', 'crates/rshell-ui/src/interaction_dialog_render.rs',
      'resources/style.css', 'crates/rshell-ui/tests/modal_host.rs',
      'crates/rshell-ui/tests/task18_native_widgets.rs'
  )
  git add -- @taskPaths
  git commit -m "feat: unify modal dialog composition" -m "Add an opaque scrim, bounded scrolling forms, focus containment, Escape cancellation, and trigger restoration."
  ```

### Task 8: Add tab overflow, pane priorities, and compact navigation drawer

**Files:**
- Create: `crates/rshell-ui/src/tab_overflow.rs`
- Create: `crates/rshell-ui/src/pane_action_layout.rs`
- Create: `crates/rshell-ui/src/navigation_drawer.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/session_tab_bar.rs`
- Modify: `crates/rshell-ui/src/pane_status.rs`
- Modify: `crates/rshell-ui/src/pane_host_render.rs`
- Modify: `crates/rshell-ui/src/connection_sidebar.rs`
- Modify: `crates/rshell-ui/src/connection_sidebar_widgets.rs`
- Modify: `crates/rshell-ui/src/main_window_shell.rs`
- Modify: `crates/rshell-ui/src/icon_registry_metadata.rs`
- Modify: `crates/rshell-ui/src/icon_vector_data.rs`
- Create: `resources/icons/more.svg`
- Create: `resources/icons/navigation.svg`
- Modify: `resources/style.css`
- Create: `crates/rshell-ui/tests/overflow_models.rs`
- Modify: `crates/rshell-ui/tests/native_widgets.rs`
- Modify: `crates/rshell-ui/tests/task18_native_widgets.rs`
- Modify: `crates/rshell-ui/tests/icon_registry.rs`

**Interfaces:**
- Consumes: `WorkspaceState`, active `TabId`, `SessionPaneViewModel::actions`, `ShellLayoutMode`, and existing sidebar controller/widget.
- Produces: `TabOverflowModel`; `SessionTabBarMsg::{Cycle, RevealActive, ActivateFromOverflow}`; `PaneActionLayout`; `NavigationDrawerState`; `ProductIcon::{More, Navigation}`.

- [ ] **Step 1: Write RED pure overflow and 20-tab tests**

  Define pure policies:

  ```rust
  pub struct TabOverflowModel {
      pub active_index: Option<usize>,
      pub overflow_indices: Vec<usize>,
  }

  impl TabOverflowModel {
      pub fn new(tab_count: usize, active_index: Option<usize>, visible_indices: &[usize]) -> Self;
      pub fn cycle(&self, delta: i32) -> Option<usize>;
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct PaneActionLayout {
      pub visible: Vec<PaneAction>,
      pub overflow: Vec<PaneAction>,
  }

  impl PaneActionLayout {
      pub fn for_width(actions: &[PaneAction], width: i32) -> Self;
  }
  ```

  Pure tests cover 20 tabs, wraparound keyboard cycling, active-tab presence in the overflow list, and action widths 180/320/600. Reset display, retry/reconnect, close, and split retain higher priority; Edit connection and Copy diagnostics move to overflow first. Native tests require every tab reachable through Ctrl+Tab/Shift+Ctrl+Tab and overflow list, with active-tab auto-reveal.

- [ ] **Step 2: Run RED overflow tests**

  Run:

  ```powershell
  cargo test -p rshell-ui --test overflow_models --locked
  cargo test -p rshell-ui --test native_widgets --locked twenty_tabs_are_keyboard_and_overflow_reachable -- --exact --nocapture
  ```

  Expected: FAIL because tabs are an unscrolled box and pane/navigation overflow policies do not exist.

- [ ] **Step 3: Build horizontal tab scrolling and closed overflow list**

  Place the existing tab box in a horizontal `gtk::ScrolledWindow` with vertical policy Never. Keep new-tab and overflow buttons outside the scroller. Build overflow rows only from authoritative `workspace.tabs`; selecting a row sends `ActivateFromOverflow(TabId)`. On active tab/workspace change, use the horizontal adjustment to reveal the active button after allocation. A capture-phase Ctrl+Tab/Shift+Ctrl+Tab sends `Cycle(1/-1)` and wraps across all 20 tabs. No optimistic tabs are inserted.

- [ ] **Step 4: Apply deterministic pane-action priorities**

  Render `PaneActionLayout::visible` in the command row and `overflow` in one accessible menu button. At narrow widths, diagnostics/edit move first; Split, Reset display, retry/reconnect, and Close remain visible in priority order while space permits. If only one action fits, a recovery notice keeps Reset display and terminal-state pages keep Retry/Close reachable via the overflow. Every menu row emits the existing `PaneHostMsg::Action`.

- [ ] **Step 5: Implement compact rail and drawer with one sidebar controller**

  `NavigationDrawerState { open: bool }` is owned by the shell, not core. Compact mode shows a 48px rail with accessible Navigation, New connection, and New group icons; Navigation toggles the existing sidebar widget as an overlay drawer. Opening moves focus into connection search, Escape closes and returns focus to Navigation, selecting/activating a connection closes the drawer after the existing output is delivered. Standard/wide reuse that same widget and show text+icon primary actions; no duplicate sidebar controller is launched.

- [ ] **Step 6: Add and scale the two source-controlled icons**

  Extend the closed registry to 18 icons. Both SVGs use the existing safe 16×16/currentColor contract; add matching internal vector paths. `More` is three centered dots; `Navigation` is three horizontal lines. They flow through Task 5's physical-size cache and receive non-empty labels/tooltips.

- [ ] **Step 7: Run GREEN tab/pane/navigation checks**

  Run:

  ```powershell
  cargo test -p rshell-ui --test overflow_models --locked
  cargo test -p rshell-ui --test icon_registry --locked
  $priorDebug = $env:G_DEBUG
  $nativeExit = 0
  $env:G_DEBUG = "fatal-warnings"
  try { cargo test -p rshell-ui --test native_widgets --locked -- --nocapture; if ($LASTEXITCODE -ne 0) { $nativeExit = $LASTEXITCODE } else { cargo test -p rshell-ui --test task18_native_widgets --locked compact_navigation_ -- --nocapture; $nativeExit = $LASTEXITCODE } } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($nativeExit -ne 0) { exit $nativeExit }
  cargo test -p rshell-ui --test component_dependencies --locked
  cargo test --locked --test production_module_limits
  ```

  Expected: all tests PASS; twenty tabs are keyboard/list reachable and active-revealed; pane actions follow exact priorities; compact drawer preserves sidebar state/focus; all 18 icons render at physical target size; GTK emits no warning.

- [ ] **Step 8: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  git add crates/rshell-ui/src/tab_overflow.rs crates/rshell-ui/src/pane_action_layout.rs crates/rshell-ui/src/navigation_drawer.rs crates/rshell-ui/src/session_tab_bar.rs crates/rshell-ui/src/pane_status.rs crates/rshell-ui/src/pane_host_render.rs crates/rshell-ui/src/connection_sidebar.rs crates/rshell-ui/src/connection_sidebar_widgets.rs crates/rshell-ui/src/main_window_shell.rs crates/rshell-ui/src/icon_registry_metadata.rs crates/rshell-ui/src/icon_vector_data.rs crates/rshell-ui/src/lib.rs resources/icons/more.svg resources/icons/navigation.svg resources/style.css crates/rshell-ui/tests/overflow_models.rs crates/rshell-ui/tests/native_widgets.rs crates/rshell-ui/tests/task18_native_widgets.rs crates/rshell-ui/tests/icon_registry.rs
  git commit -m "feat: add adaptive navigation overflow" -m "Keep tabs, pane actions, and connections reachable at compact and high-count layouts."
  ```

### Task 9: Expand typed P0 smoke with recovery, DPI, layout, modal, and accessibility evidence

**Files:**
- Create: `crates/rshell-ui/src/smoke_driver_recovery.rs`
- Create: `crates/rshell-ui/src/smoke_driver_visual_matrix.rs`
- Create: `crates/rshell-ui/src/main_window_smoke_matrix.rs`
- Modify: `crates/rshell-ui/src/lib.rs`
- Modify: `crates/rshell-ui/src/smoke_driver.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_action_kind.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_actions.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_evidence.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_report.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_routing.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_completion.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_state.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_routes.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_input.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_resize.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_visual.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_capture.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_evidence.rs`
- Modify: `crates/rshell-ui/src/visual_contract.rs`
- Modify: `crates/rshell-ui/src/visual_png.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_auth_tests.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_completion_tests.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_progress_tests.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_state_tests.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_terminal_tests.rs`
- Modify: `crates/rshell-ui/src/smoke_driver_visual_tests.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_binding_tests.rs`
- Modify: `crates/rshell-ui/src/main_window_smoke_tests.rs`
- Modify: `src/main.rs`
- Modify: `src/p0_smoke_actions.rs`
- Modify: `src/p0_smoke_scenario.rs`
- Modify: `src/p0_smoke_report.rs`
- Modify: `src/p0_smoke_report_steps.rs`
- Modify: `src/p0_smoke_report_terminal.rs`
- Modify: `src/p0_smoke_report_visual.rs`
- Modify: `src/p0_smoke_contract.rs`
- Modify: `src/p0_smoke_contract_binding.rs`
- Modify: `tests/fixtures/smoke/p0-scenario.json`
- Modify: `tests/p0_acceptance.rs`
- Create: `crates/rshell-session/tests/interrupt_conpty.rs`
- Modify: `scripts/qa/p0-smoke.ps1`
- Modify: `scripts/qa/assert-no-secrets.ps1`

**Interfaces:**
- Consumes: typed interrupt/reset commands, measured metric/icon facts, adaptive/modal/overflow models, real GTK capture, permanent interrupt fixture, existing run-nonce/component/session binding, and all eleven existing P0 surfaces.
- Produces: additive smoke actions `InterruptTerminal`, `ResetDisplay`, `ResizeWindow`; parameterized `VisualCheckpoint`; `SmokeInterruptEvidence`; `SmokeDpiEvidence`; `SmokeAccessibilityEvidence`; `SmokeVisualCheckpointEvidence`; multi-PNG leaf names; isolated `-ArtifactRoot` harness parameter.

- [ ] **Step 1: Write RED additive action/schema tests**

  Preserve the existing first 25 `#[repr(u8)]` discriminants and append three actions at indices 25-27:

  ```rust
  pub enum SmokeActionKind {
      // Existing variants 0 through 24 remain in their current order.
      InterruptTerminal = 25,
      ResetDisplay = 26,
      ResizeWindow = 27,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum SmokeVisualState {
      Empty, Connected, TwentyTabs, Single, HSplit, VSplit,
      TopBottom3, Grid, Editor, Settings, Import, HostKey,
      Authentication, Failure, Recovery,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct SmokeVisualCheckpoint {
      pub id: String,
      pub state: SmokeVisualState,
      pub width: i32,
      pub height: i32,
      pub expected_mode: ShellLayoutMode,
  }
  ```

  Replace the fieldless `SmokeAction::VisualCheckpoint` payload with `VisualCheckpoint(SmokeVisualCheckpoint)` while keeping its discriminant/name. Parser tests reject duplicate checkpoint IDs, unsupported dimensions, mismatched width/mode, absolute output paths, unknown fields, reordered old action names, and any terminal text/path/endpoint field in evidence.

  Set `SMOKE_SCENARIO_VERSION` and the emitted report version to `2`. Version 1 scenarios fail with the exact unsupported-version error; there is no dual parser path. The static fixture and dynamic harness both emit version 2, while all pre-existing action names and evidence requirements remain unchanged.

- [ ] **Step 2: Define exact typed recovery/DPI/accessibility evidence**

  Add closed, non-secret structs:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct SmokeInterruptEvidence {
      pub sequence: u64,
      pub command_count: u64,
      pub wire_byte: u8,
      pub exact_etx: bool,
      pub enhanced_encoder_bypassed: bool,
      pub surviving_tui: bool,
      pub notice_visible: bool,
      pub reset_generation: Option<u64>,
      pub modes_clean: bool,
      pub replacement_character_count: usize,
      pub old_tui_overlap: bool,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub struct SmokeDpiEvidence {
      pub logical_width: i32,
      pub logical_height: i32,
      pub effective_scale: f64,
      pub effective_dpi: f64,
      pub cell_width: f64,
      pub cell_height: f64,
      pub icon_logical_size: u16,
      pub icon_texture_width: i32,
      pub icon_texture_height: i32,
      pub dpi_fallback_used: bool,
  }

  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
  pub struct SmokeAccessibilityEvidence {
      pub unnamed_icon_controls: usize,
      pub hidden_primary_actions: usize,
      pub zero_size_panes: usize,
      pub horizontal_clipping: bool,
      pub background_insensitive: bool,
      pub focus_contained: bool,
      pub focus_restored: bool,
      pub escape_cancelled: bool,
  }
  ```

  `SmokeVisualCheckpointEvidence` contains checkpoint ID/state/layout, `SmokeVisualFacts`, PNG facts, DPI facts, and accessibility facts. `SmokeCounters.visual` becomes a keyed collection by checkpoint ID; `SmokeReport` carries a vector of requested/captured PNG paths internally, while root serialization emits validated leaf names only.

- [ ] **Step 3: Run RED parser/report/contract tests**

  Run:

  ```powershell
  cargo test --locked --test p0_acceptance p0_action_schema_preserves_old_discriminants_and_adds_recovery_matrix -- --exact
  cargo test --locked --test p0_acceptance p0_report_rejects_incomplete_visual_and_dpi_matrix -- --exact
  cargo test -p rshell-ui --lib smoke_driver_recovery --locked
  ```

  Expected: FAIL because the actions, evidence structs, and multiple checkpoint contract do not exist.

- [ ] **Step 4: Route real keyboard interrupt and explicit recovery actions**

  `InterruptTerminal` locates the active, verified `TerminalView` binding and submits `TerminalViewMsg::Key { key: gdk::Key::c, state: CONTROL_MASK }`; it may not dispatch a protocol command directly. Capture the resulting command sequence and fixture frame marker to prove one `03`. `ResetDisplay` clicks/invokes the visible pane action, not the engine directly, then waits for a newer frame with clean modes and no overlap/replacement character. Completion predicates fail if a surviving fixture was auto-reset before the action.

  Add ignored Windows tests in `interrupt_conpty.rs` that compile the Task 1 fixture and launch it both directly and as a child of `pwsh -NoProfile -Command`. Each real ConPTY case reads the marker, sends one Ctrl+C through the actor, and requires `interrupt=03` rather than visible CSI-u bytes.

- [ ] **Step 5: Capture an exact real GTK matrix**

  `ResizeWindow` calls `ApplicationWindow::set_default_size`, waits for the requested mode and positive allocations, and records actual realized size. Build the following fail-closed checkpoint matrix in the dynamically generated P0 scenario:

  | Logical size | Required real states |
  |---|---|
  | 800×600 | Empty, TwentyTabs, Grid, Editor, Settings, Import, Recovery |
  | 1360×860 | Connected, Single, HSplit, VSplit, TopBottom3, Grid, Editor, Settings, Import, HostKey, Authentication, Failure, Recovery |
  | 1920×1080 | Connected, TwentyTabs, Grid, Editor, Settings, Import |

  Each checkpoint captures a fresh real `WidgetPaintable` PNG under a unique leaf name, records measured cell/icon physical metadata, and checks no clipping, hidden primary action, zero-size pane, missing icon, duplicate identity, or unbounded form. Settings/Import checkpoints use normal modal open/close paths; Editor uses its normal draft; HostKey/Authentication are captured while the real interaction is pending; Failure/Recovery use authoritative session states. No mock widget tree or synthesized PNG satisfies this matrix.

  The keyboard pass drives, rather than merely inspects, local-tab creation, connection selection/edit/save/cancel, tab switching, pane switching/splitting/closing, each dialog action, interrupt, Reset display, and reconnect. It asserts every icon control has a non-empty accessible name and tooltip, modal background disabled state is exposed, primary text contrast is at least 4.5:1, focus contrast is at least 3:1, and pending/success/error states have text or native semantics.

- [ ] **Step 6: Preserve all existing P0 surfaces and identity binding**

  Use the version 2 contract established in Step 1 and reject every other version. Every current local/native password/native key/native keyboard-interactive/System agent/host-key/vault/import/tabs-splits/cleanup requirement remains. Continue matching run nonce, fixture, connection label, connection/profile/endpoint, pane/session IDs, and external observations. Additive visual/recovery evidence may not substitute for any old SSH/vault/import/cleanup proof.

- [ ] **Step 7: Make artifact output isolated and cleanup fail closed**

  Add a harness parameter:

  ```powershell
  param(
      [ValidateSet("Unit", "Ssh", "Gtk", "Vault", "All")][string]$Mode = "All",
      [string]$ArtifactRoot = (Join-Path $PSScriptRoot "..\..\artifacts\p0-smoke"),
      [AllowEmptyString()][string]$RegressionParserProbe = "",
      [AllowEmptyString()][string]$RegressionCaseProbe = ""
  )
  ```

  Resolve and require `ArtifactRoot` outside the repository's pre-existing `artifacts/` when explicitly supplied. Run GTK with `G_DEBUG=fatal-warnings`. Finalize JSON/JUnit only after fixture, SSH agent, vault, actor, child-process, temporary file, and secret scans complete. Scan every PNG/JSON/JUnit and captured log; preserve leaf-only paths and zero skipped/non-passed actions.

- [ ] **Step 8: Run GREEN typed/native smoke checks without touching pre-existing artifacts**

  Run:

  ```powershell
  cargo test --locked --test p0_acceptance
  cargo test -p rshell-ui --lib smoke_driver --locked
  cargo test -p rshell-session --test interrupt_conpty --locked --no-run
  $qaRoot = Join-Path $env:TEMP "rshell-p0-plan-$([Guid]::NewGuid().ToString('N'))"
  [void](New-Item -ItemType Directory -Path $qaRoot)
  try {
      $priorDebug = $env:G_DEBUG
      $env:G_DEBUG = "fatal-warnings"
      pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode Gtk -ArtifactRoot $qaRoot
      $smokeExit = $LASTEXITCODE
      if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug }
      if ($smokeExit -ne 0) { exit $smokeExit }
      pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot $qaRoot
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
      if (Test-Path -LiteralPath $qaRoot -PathType Container) { Remove-Item -LiteralPath $qaRoot -Recurse -Force }
  }
  cargo test --locked --test production_module_limits
  cargo test -p rshell-ui --test component_dependencies --locked
  ```

  Expected: all commands PASS; the exact screenshot matrix is real and complete; one ETX and explicit reset evidence are present; all old P0 contracts remain; cleanup reports zero actors/direct children and no secret matches; pre-existing repository `artifacts/` is unchanged.

- [ ] **Step 9: Conditional suggested commit boundary**

  Only with existing explicit user Git authorization:

  ```powershell
  $taskPaths = @(
      'crates/rshell-ui/src/smoke_driver_recovery.rs', 'crates/rshell-ui/src/smoke_driver_visual_matrix.rs',
      'crates/rshell-ui/src/main_window_smoke_matrix.rs', 'crates/rshell-ui/src/lib.rs',
      'crates/rshell-ui/src/smoke_driver.rs',
      'crates/rshell-ui/src/smoke_driver_action_kind.rs', 'crates/rshell-ui/src/smoke_driver_actions.rs',
      'crates/rshell-ui/src/smoke_driver_evidence.rs', 'crates/rshell-ui/src/smoke_driver_report.rs',
      'crates/rshell-ui/src/smoke_driver_routing.rs', 'crates/rshell-ui/src/smoke_driver_completion.rs',
      'crates/rshell-ui/src/smoke_driver_state.rs', 'crates/rshell-ui/src/main_window_smoke.rs',
      'crates/rshell-ui/src/main_window_smoke_routes.rs', 'crates/rshell-ui/src/main_window_smoke_input.rs',
      'crates/rshell-ui/src/main_window_smoke_resize.rs', 'crates/rshell-ui/src/main_window_smoke_visual.rs',
      'crates/rshell-ui/src/main_window_smoke_capture.rs', 'crates/rshell-ui/src/main_window_smoke_evidence.rs',
      'crates/rshell-ui/src/visual_contract.rs', 'crates/rshell-ui/src/visual_png.rs',
      'crates/rshell-ui/src/smoke_driver_auth_tests.rs', 'crates/rshell-ui/src/smoke_driver_completion_tests.rs',
      'crates/rshell-ui/src/smoke_driver_progress_tests.rs', 'crates/rshell-ui/src/smoke_driver_state_tests.rs',
      'crates/rshell-ui/src/smoke_driver_terminal_tests.rs', 'crates/rshell-ui/src/smoke_driver_visual_tests.rs',
      'crates/rshell-ui/src/main_window_smoke_binding_tests.rs', 'crates/rshell-ui/src/main_window_smoke_tests.rs',
      'src/main.rs', 'src/p0_smoke_actions.rs', 'src/p0_smoke_scenario.rs',
      'src/p0_smoke_report.rs', 'src/p0_smoke_report_steps.rs', 'src/p0_smoke_report_terminal.rs',
      'src/p0_smoke_report_visual.rs', 'src/p0_smoke_contract.rs', 'src/p0_smoke_contract_binding.rs',
      'tests/fixtures/smoke/p0-scenario.json', 'tests/p0_acceptance.rs',
      'crates/rshell-session/tests/interrupt_conpty.rs', 'scripts/qa/p0-smoke.ps1',
      'scripts/qa/assert-no-secrets.ps1'
  )
  git add -- @taskPaths
  git commit -m "test: expand recovery and HiDPI P0 evidence" -m "Bind interrupt, physical rendering, adaptive layouts, dialogs, focus, and accessibility to real GTK smoke captures."
  ```

### Task 10: Run full native, package, hosted, and same-identity acceptance

**Files:**
- Modify: `crates/rshell-ui/src/startup_probe.rs`
- Modify: `crates/rshell-ui/tests/startup.rs`
- Modify: `src/main.rs`
- Modify: `scripts/qa/workflow-contract.ps1`
- Modify: `scripts/qa/assert-package.ps1`
- Modify: `tests/p0_acceptance.rs`
- Verify, modify only on proven contract failure: `.github/workflows/ci.yml`
- Verify, modify only on proven contract failure: `.github/workflows/release.yml`
- Generate outside repository, under one `$reviewRoot` below `$env:TEMP`: exact-HEAD native logs/P0/PNGs, local P0 outputs, local gate logs, package downloads, hosted run metadata, scale availability, native and overall hash manifests, and review identity JSON.

**Interfaces:**
- Consumes: all Tasks 1-9, base SHA `c7be3bf2ccfbd635075d0ef5b1e89c94271fdd14`, conditional user-authorized commits/push, GitHub Actions API token, Oracle-high, and Reviewer-high.
- Produces: post-commit, exact-HEAD local/native command receipts under one `$reviewRoot`; release startup/package proof; successful exact-SHA Linux/macOS/Windows CI and release jobs; honest physical-scale ledger; native and overall SHA-256 evidence manifests; two acceptance receipts over the identical identity.

- [ ] **Step 1: Lock package/workflow contracts before the full run**

  Extend startup evidence with closed fields `measured_terminal_geometry_ready`, `scale_aware_icons_ready`, `icon_backend`, `icon_count = 18`, and `adaptive_layout_modes = 3`; emit no font family/path, terminal text, endpoint, or scale claim. `assert-package.ps1` requires these fields from the packaged binary and still rejects loose `resources/`, icons, SVGs, old SSH/backend markers, and missing GTK runtime files.

  Extend `workflow-contract.ps1`/`p0_acceptance.rs` to require: full locked workspace fmt/check/test/Clippy, module-cap test, terminal engine gate, P0 All on all three CI platforms, `G_DEBUG=fatal-warnings` inside the smoke harness, fail-closed cleanup/no-secret ordering, and package assertion for all three release targets. Preserve the deliberate rule that the release workflow does not rerun the terminal-engine benchmark.

- [ ] **Step 2: Run RED then GREEN workflow/package contract tests**

  Before implementation, the new named tests must fail on missing startup/matrix markers. After the exact script/startup changes, run:

  ```powershell
  cargo test -p rshell-ui --test startup --locked
  cargo test --locked --test p0_acceptance workflow_contract_requires_recovery_hidpi_and_native_matrix -- --exact
  cargo test --locked --test p0_acceptance package_contract_requires_measured_geometry_and_physical_icons -- --exact
  pwsh -NoProfile -File scripts/qa/workflow-contract.ps1
  ```

  Expected: all commands PASS; mutation probes fail for a skipped/conditional/continued P0 gate, absent fatal-warning setting, missing package startup fields, missing platform matrix member, or weakened cleanup/no-secret ordering.

- [ ] **Step 3: Conditional final implementation commit boundary**

  Only when explicit user Git authorization already exists, commit Task 10 contract changes after Step 2 is green and before any acceptance evidence is captured. Include the approved spec and this plan only if that same authorization covers planning artifacts:

  ```powershell
  git add crates/rshell-ui/src/startup_probe.rs crates/rshell-ui/tests/startup.rs src/main.rs scripts/qa/workflow-contract.ps1 scripts/qa/assert-package.ps1 tests/p0_acceptance.rs
  if (Test-Path -LiteralPath docs/superpowers/specs/2026-08-28-terminal-hidpi-ui-recovery-design.md) { git add docs/superpowers/specs/2026-08-28-terminal-hidpi-ui-recovery-design.md }
  if (Test-Path -LiteralPath docs/superpowers/plans/2026-08-28-terminal-hidpi-ui-recovery.md) { git add docs/superpowers/plans/2026-08-28-terminal-hidpi-ui-recovery.md }
  git commit -m "ci: bind terminal recovery acceptance" -m "Require native recovery, HiDPI, adaptive UI, package, and hosted evidence before final review."
  ```

  If `.github/workflows/ci.yml` or `.github/workflows/release.yml` changed because a failing contract proved a missing step, stage only the exact verified workflow path in this boundary. Without authorization, do not commit or push; report hosted/final identity steps as blocked.

- [ ] **Step 4: Freeze the committed candidate before native or local acceptance**

  All authorized implementation commits must already exist. Run Steps 4-11 in one guarded PowerShell session so `$baseSha`, `$reviewHead`, `$reviewRoot`, `$repository`, and `$headers` remain bound to one candidate. Earlier development native runs are diagnostic only and cannot satisfy acceptance. Create the single evidence root before rerunning native QA; do not inspect or mutate the pre-existing repository `artifacts/`:

  ```powershell
  $baseSha = "c7be3bf2ccfbd635075d0ef5b1e89c94271fdd14"
  $reviewHead = (git rev-parse HEAD).Trim()
  if ($LASTEXITCODE -ne 0 -or $reviewHead -notmatch '^[0-9a-f]{40}$') { throw "Review HEAD is invalid." }
  git diff --quiet
  if ($LASTEXITCODE -ne 0) { throw "Tracked working-tree changes would make native evidence differ from HEAD." }
  git diff --cached --quiet
  if ($LASTEXITCODE -ne 0) { throw "Staged changes would make native evidence differ from HEAD." }
  $preFreezeStatus = @(git status --porcelain=v1 --untracked-files=normal | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $preFreezeUnexpected = @($preFreezeStatus | Where-Object { $_ -ne '?? artifacts/' })
  if ($preFreezeUnexpected.Count -ne 0) { throw "Unexpected pre-freeze state invalidates native QA: $($preFreezeUnexpected -join ', ')" }
  $reviewRoot = Join-Path $env:TEMP "rshell-review-$reviewHead"
  if (Test-Path -LiteralPath $reviewRoot) { throw "Review evidence root already exists; choose a fresh candidate identity." }
  [void](New-Item -ItemType Directory -Path $reviewRoot)
  $nativeRoot = Join-Path $reviewRoot "native"
  $localRoot = Join-Path $reviewRoot "local"
  [void](New-Item -ItemType Directory -Path $nativeRoot)
  [void](New-Item -ItemType Directory -Path $localRoot)
  $candidate = [ordered]@{
      base = $baseSha
      head = $reviewHead
      started_utc = [DateTimeOffset]::UtcNow.ToString('O')
  }
  [System.IO.File]::WriteAllText((Join-Path $reviewRoot "candidate-identity.json"), ($candidate | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))

  function Assert-ReviewHead {
      $current = (git rev-parse HEAD).Trim()
      if ($LASTEXITCODE -ne 0 -or $current -ne $reviewHead) { throw "HEAD changed from frozen review identity." }
  }
  Assert-ReviewHead
  ```

  Expected: the committed candidate HEAD is frozen before native evidence; `$reviewRoot/native` and `$reviewRoot/local` are the only acceptance roots.

- [ ] **Step 5: Run exact-HEAD Windows ConPTY and GTK QA under `$reviewRoot/native`**

  Assert HEAD immediately before and after all native work. Capture every command's output beneath the frozen root:

  ```powershell
  Assert-ReviewHead
  & cargo test -p rshell-session --test interrupt_conpty --locked direct_fixture_receives_exact_etx_on_windows_conpty -- --ignored --exact --nocapture *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "conpty-direct.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & cargo test -p rshell-session --test interrupt_conpty --locked powershell_child_receives_exact_etx_on_windows_conpty -- --ignored --exact --nocapture *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "conpty-powershell.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $nativeP0Root = Join-Path $nativeRoot "gtk-current"
  [void](New-Item -ItemType Directory -Path $nativeP0Root)
  $priorDebug = $env:G_DEBUG
  $env:G_DEBUG = "fatal-warnings"
  try {
      & cargo test -p rshell-ui --test terminal_metrics --locked -- --nocapture *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "terminal-metrics.log")
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      & cargo test -p rshell-ui --test native_visual_contract --locked -- --nocapture *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "native-visual.log")
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      & cargo test -p rshell-ui --test modal_host --locked -- --nocapture *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "modal-host.log")
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      & pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode Gtk -ArtifactRoot $nativeP0Root *>&1 | Tee-Object -FilePath (Join-Path $nativeRoot "p0-gtk.log")
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      $pendingNativeSecretLog = Join-Path $reviewRoot "native-no-secrets.pending.log"
      & pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot $nativeRoot *>&1 | Tee-Object -FilePath $pendingNativeSecretLog
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
      Move-Item -LiteralPath $pendingNativeSecretLog -Destination (Join-Path $nativeRoot "native-no-secrets.log")
  } finally {
      if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug }
  }
  Assert-ReviewHead
  ```

  Expected: both ConPTY launches observe exactly `03`; all GTK tests/captures pass with fatal warnings; 800/1360/1920, twenty tabs, five pane fixtures, all dialogs, keyboard/focus/accessibility, recovery cleanliness, no `U+FFFD`, and no old-TUI overlap have exact-HEAD files under `$reviewRoot/native`.

- [ ] **Step 6: Record scale truth and freeze a native-file manifest**

  Parse only reports beneath `$reviewRoot/native`. Mark 100/125/150/200% as captured only when a real screenshot reports that scale; otherwise record `unavailable_on_current_windows_session`. If another physical scale is available, rerun only the Step 5 GTK/P0 portion into the next unused fixed ordinal directory (`gtk-additional-1`, then `gtk-additional-2`), assert HEAD before/after, then regenerate this ledger. Never relabel pure/synthetic evidence as physical:

  ```powershell
  Assert-ReviewHead
  $reports = @(Get-ChildItem -LiteralPath $nativeRoot -Filter '*.json' -File -Recurse |
      Where-Object { $_.Name -ne 'physical-scale-availability.json' -and $_.Name -ne 'native-hashes.json' })
  if ($reports.Count -eq 0) { throw "Native GTK report is unavailable." }
  $captured = foreach ($reportFile in $reports) {
      $report = Get-Content -LiteralPath $reportFile.FullName -Raw | ConvertFrom-Json
      foreach ($checkpoint in @($report.visual_checkpoints)) {
          [pscustomobject]@{
              scale = [double]$checkpoint.dpi.effective_scale
              png = [string]$checkpoint.png
              report = [System.IO.Path]::GetRelativePath($nativeRoot, $reportFile.FullName).Replace('\', '/')
          }
      }
  }
  if (@($captured).Count -eq 0) { throw "Native GTK reports have no visual checkpoints." }
  $scaleLedger = foreach ($percent in @(100, 125, 150, 200)) {
      $scale = [double]$percent / 100.0
      $matches = @($captured | Where-Object { [Math]::Abs($_.scale - $scale) -lt 0.000001 })
      [ordered]@{
          percent = $percent
          status = if ($matches.Count -gt 0) { 'captured' } else { 'unavailable_on_current_windows_session' }
          checkpoints = @($matches | ForEach-Object { "$($_.report)::$($_.png)" } | Sort-Object -Unique)
      }
  }
  $scalePath = Join-Path $nativeRoot "physical-scale-availability.json"
  [System.IO.File]::WriteAllText($scalePath, ($scaleLedger | ConvertTo-Json -Depth 4), [System.Text.UTF8Encoding]::new($false))

  $nativeManifestPath = Join-Path $nativeRoot "native-hashes.json"
  $nativeHashes = @(Get-ChildItem -LiteralPath $nativeRoot -Recurse -File |
      Where-Object { $_.FullName -ne [System.IO.Path]::GetFullPath($nativeManifestPath) } |
      Sort-Object FullName |
      ForEach-Object {
          $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
          [pscustomobject]@{
              path = [System.IO.Path]::GetRelativePath($nativeRoot, $_.FullName).Replace('\', '/')
              sha256 = $hash.Hash.ToLowerInvariant()
          }
      })
  foreach ($required in @('conpty-direct.log', 'conpty-powershell.log', 'terminal-metrics.log', 'native-visual.log', 'modal-host.log', 'p0-gtk.log', 'native-no-secrets.log', 'physical-scale-availability.json')) {
      if ($nativeHashes.path -notcontains $required) { throw "Required native evidence is missing: $required" }
  }
  if (@($nativeHashes | Where-Object { $_.path -match '\.(json|png|junit\.xml)$' }).Count -lt 3) { throw "Native GTK JSON/PNG/JUnit evidence is incomplete." }
  [System.IO.File]::WriteAllText($nativeManifestPath, ($nativeHashes | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))
  $nativeManifestSha256 = (Get-FileHash -LiteralPath $nativeManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  Assert-ReviewHead
  ```

  Expected: the scale ledger is honest, and `native-hashes.json` binds every exact post-freeze ConPTY/GTK/log/report/PNG/JUnit file to `$reviewHead`.

- [ ] **Step 7: Run every remaining local gate under the same review root**

  Run fail-fast with HEAD assertions before and after; P0 All output belongs under `$reviewRoot/local`:

  ```powershell
  Assert-ReviewHead
  $localP0Root = Join-Path $localRoot "p0-smoke"
  [void](New-Item -ItemType Directory -Path $localP0Root)

  cargo fmt --all -- --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo check --workspace --all-targets --all-features --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo test --workspace --all-features --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo test --locked --test production_module_limits
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo test -p rshell-ui --test component_dependencies --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & pwsh -NoProfile -File scripts/qa/terminal-engine-gate.ps1 *>&1 | Tee-Object -FilePath (Join-Path $localRoot "terminal-engine.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & pwsh -NoProfile -File scripts/qa/workflow-contract.ps1 *>&1 | Tee-Object -FilePath (Join-Path $localRoot "workflow-contract.log")
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $priorDebug = $env:G_DEBUG
  $env:G_DEBUG = "fatal-warnings"
  try { pwsh -NoProfile -File scripts/qa/p0-smoke.ps1 -Mode All -ArtifactRoot $localP0Root; $p0Exit = $LASTEXITCODE } finally { if ($null -eq $priorDebug) { Remove-Item Env:G_DEBUG -ErrorAction SilentlyContinue } else { $env:G_DEBUG = $priorDebug } }
  if ($p0Exit -ne 0) { exit $p0Exit }
  pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot $localP0Root
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  cargo build --release --workspace --locked
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  Assert-ReviewHead
  ```

  Expected: every command exits 0; fmt/check/test/Clippy/module cap/single egress/engine/workflow pass; P0 All has no skipped or non-passed step across all eleven old surfaces and new matrix evidence; cleanup reports zero actors/direct children and completed secret scan; release workspace builds.

- [ ] **Step 8: Verify tracked identity and pre-existing untracked boundary**

  Run read-only Git checks; `?? artifacts/` is the only accepted visible untracked baseline and remains outside the candidate identity:

  ```powershell
  $candidate = Get-Content -LiteralPath (Join-Path $reviewRoot "candidate-identity.json") -Raw | ConvertFrom-Json
  $currentHead = (git rev-parse HEAD).Trim()
  if ($currentHead -ne $candidate.head -or $candidate.base -ne $baseSha) { throw "Candidate identity changed after local gates." }
  git diff --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  git diff --cached --check
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  $tracked = @((git diff --name-only) + (git diff --cached --name-only) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
  if ($tracked.Count -ne 0) { throw "Tracked working-tree changes invalidate the review identity." }
  $status = @(git status --porcelain=v1 --untracked-files=normal | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
  $unexpected = @($status | Where-Object { $_ -ne '?? artifacts/' })
  if ($unexpected.Count -ne 0) { throw "Unexpected working-tree state invalidates review: $($unexpected -join ', ')" }
  $rangeNames = @(git diff --name-only "$baseSha..$reviewHead")
  if ($LASTEXITCODE -ne 0 -or $rangeNames.Count -eq 0) { throw "Candidate range is empty or unreadable." }
  ```

  Any tracked edit, commit, changed gate input, or rewritten temporary evidence after this point invalidates all hosted and reviewer receipts.

- [ ] **Step 9: Obtain exact-SHA hosted CI and release-package evidence**

  A push is permitted only with explicit user authorization. After that authorized push, query the exact candidate SHA with a 30-minute deadline; queued, missing, cancelled, skipped, neutral, or stale-SHA runs are not acceptance:

  ```powershell
  if ([string]::IsNullOrWhiteSpace($env:GITHUB_TOKEN)) { throw "GITHUB_TOKEN is required for hosted evidence." }
  $origin = (git remote get-url origin).Trim()
  if ($origin -notmatch 'github\.com[:/](?<repo>[^/]+/[^/.]+)(?:\.git)?$') { throw "GitHub origin is unavailable." }
  $repository = $matches.repo
  $headers = @{
      Authorization = "Bearer $env:GITHUB_TOKEN"
      Accept = "application/vnd.github+json"
      'X-GitHub-Api-Version' = "2022-11-28"
  }

  function Wait-ExactWorkflow {
      param([string]$Workflow, [string]$Label)
      $uri = "https://api.github.com/repos/$repository/actions/workflows/$Workflow/runs?head_sha=$reviewHead&event=push&per_page=100"
      $deadline = [DateTimeOffset]::UtcNow.AddMinutes(30)
      do {
          $response = Invoke-RestMethod -Uri $uri -Headers $headers
          $runs = @($response.workflow_runs | Where-Object { $_.head_sha -eq $reviewHead } | Sort-Object created_at -Descending)
          if ($runs.Count -gt 0 -and $runs[0].status -eq 'completed') {
              if ($runs[0].conclusion -ne 'success') { throw "$Label failed for $reviewHead." }
              return $runs[0]
          }
          Start-Sleep -Seconds 15
      } while ([DateTimeOffset]::UtcNow -lt $deadline)
      throw "$Label did not complete for $reviewHead before the deadline."
  }

  function Assert-ExactJobs {
      param($Run, [hashtable]$Required)
      $jobs = @((Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/actions/runs/$($Run.id)/jobs?per_page=100" -Headers $headers).jobs)
      foreach ($jobName in $Required.Keys) {
          $job = @($jobs | Where-Object { $_.name -eq $jobName })
          if ($job.Count -ne 1 -or $job[0].conclusion -ne 'success') { throw "Hosted job $jobName is missing or unsuccessful." }
          foreach ($stepName in $Required[$jobName]) {
              $step = @($job[0].steps | Where-Object { $_.name -eq $stepName })
              if ($step.Count -ne 1 -or $step[0].conclusion -ne 'success') { throw "Hosted step $jobName / $stepName is missing or unsuccessful." }
          }
      }
  }

  $ciRun = Wait-ExactWorkflow -Workflow "ci.yml" -Label "CI"
  $releaseRun = Wait-ExactWorkflow -Workflow "release.yml" -Label "Release"
  Assert-ExactJobs -Run $ciRun -Required @{
      'Linux x86_64' = @('Run required workspace gates', 'Run terminal engine gate', 'Run Secret Service vault probe and P0 All smoke (Linux)')
      'macOS arm64' = @('Run required workspace gates', 'Run terminal engine gate', 'Run temporary keychain vault probe and P0 All smoke (macOS)')
      'Windows x86_64' = @('Run required workspace gates', 'Run terminal engine gate', 'Run Credential Manager vault probe and P0 All smoke (Windows)')
  }
  Assert-ExactJobs -Run $releaseRun -Required @{
      'Build linux-x86_64' = @('Build release', 'Package (Linux/macOS)', 'Upload artifact (Unix)')
      'Build macos-arm64' = @('Build release', 'Package (Linux/macOS)', 'Upload artifact (Unix)')
      'Build windows-x86_64' = @('Build release', 'Package (Windows)', 'Upload artifact (Windows)')
      'Release' = @('Download all artifacts', 'Update Nightly')
  }
  $hosted = [ordered]@{
      head = $reviewHead
      ci_run_id = $ciRun.id
      ci_head_sha = $ciRun.head_sha
      release_run_id = $releaseRun.id
      release_head_sha = $releaseRun.head_sha
  }
  [System.IO.File]::WriteAllText((Join-Path $reviewRoot "hosted-runs.json"), ($hosted | ConvertTo-Json), [System.Text.UTF8Encoding]::new($false))
  ```

  Expected: CI proves Linux/macOS/Windows full workspace, terminal gate, real platform vault, P0 All and cleanup; Release proves all three locked builds and package assertions; both workflow `head_sha` values equal `$reviewHead`.

- [ ] **Step 10: Download, recheck, and hash exact release artifacts**

  Download the three non-expired artifacts from the exact release run into `$reviewRoot\hosted`; require one package per target, locally recheck the Windows package, then scan the entire temporary evidence root:

  ```powershell
  $hostedRoot = Join-Path $reviewRoot "hosted"
  [void](New-Item -ItemType Directory -Path $hostedRoot)
  $artifactResponse = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/actions/runs/$($releaseRun.id)/artifacts?per_page=100" -Headers $headers
  $expectedArtifacts = @(
      'rshell-x86_64-unknown-linux-gnu',
      'rshell-aarch64-apple-darwin',
      'rshell-x86_64-pc-windows-msvc'
  )
  foreach ($name in $expectedArtifacts) {
      $match = @($artifactResponse.artifacts | Where-Object { $_.name -eq $name -and -not $_.expired })
      if ($match.Count -ne 1) { throw "Hosted artifact $name is missing or ambiguous." }
      $wrapper = Join-Path $hostedRoot "$name.github.zip"
      Invoke-WebRequest -Uri $match[0].archive_download_url -Headers $headers -OutFile $wrapper
      $destination = Join-Path $hostedRoot $name
      Expand-Archive -LiteralPath $wrapper -DestinationPath $destination
  }
  $windowsPackage = Join-Path $hostedRoot "rshell-x86_64-pc-windows-msvc\rshell-x86_64-pc-windows-msvc.zip"
  $linuxPackage = Join-Path $hostedRoot "rshell-x86_64-unknown-linux-gnu\rshell-x86_64-unknown-linux-gnu.tar.gz"
  $macPackage = Join-Path $hostedRoot "rshell-aarch64-apple-darwin\rshell-aarch64-apple-darwin.tar.gz"
  foreach ($package in @($windowsPackage, $linuxPackage, $macPackage)) {
      if (-not (Test-Path -LiteralPath $package -PathType Leaf)) { throw "Hosted release package is missing: $package" }
  }
  pwsh -NoProfile -File scripts/qa/assert-package.ps1 -Target x86_64-pc-windows-msvc -Package $windowsPackage
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  pwsh -NoProfile -File scripts/qa/assert-no-secrets.ps1 -ArtifactRoot $reviewRoot
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

  $nativeManifestPath = Join-Path $nativeRoot "native-hashes.json"
  $currentNativeManifestSha256 = (Get-FileHash -LiteralPath $nativeManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($currentNativeManifestSha256 -ne $nativeManifestSha256) { throw "Native evidence changed after it was frozen." }
  $manifestPath = Join-Path $reviewRoot "artifact-hashes.json"
  $hashes = @(Get-ChildItem -LiteralPath $reviewRoot -Recurse -File |
      Where-Object { $_.FullName -ne [System.IO.Path]::GetFullPath($manifestPath) } |
      Sort-Object FullName |
      ForEach-Object {
          $hash = Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256
          [pscustomobject]@{
              path = [System.IO.Path]::GetRelativePath($reviewRoot, $_.FullName).Replace('\', '/')
              sha256 = $hash.Hash.ToLowerInvariant()
          }
      })
  if ($hashes.Count -lt 25) { throw "Identity-bound evidence set is incomplete." }
  [System.IO.File]::WriteAllText($manifestPath, ($hashes | ConvertTo-Json -Depth 3), [System.Text.UTF8Encoding]::new($false))
  $manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  ```

  Expected: all three packages exist; hosted package jobs already ran target-native assertions; downloaded Windows package passes again locally; no secret scan match exists; the overall manifest contains the unchanged native manifest plus every exact native file, local logs/P0 JSON/JUnit/PNGs, scale ledger, hosted metadata, and all packages.

- [ ] **Step 11: Obtain Oracle-high and Reviewer-high on one immutable identity**

  The orchestrator, not a planner or implementation worker, constructs one exact identity block and includes it unchanged in both review requests:

  ```powershell
  $hosted = Get-Content -LiteralPath (Join-Path $reviewRoot "hosted-runs.json") -Raw | ConvertFrom-Json
  $nativeManifestPath = Join-Path $nativeRoot "native-hashes.json"
  $nativeManifestSha256 = (Get-FileHash -LiteralPath $nativeManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $manifestPath = Join-Path $reviewRoot "artifact-hashes.json"
  $manifestSha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $reviewIdentity = [ordered]@{
      base = $baseSha
      head = $reviewHead
      committed_range = "$baseSha..$reviewHead"
      native_manifest = $nativeManifestPath
      native_manifest_sha256 = $nativeManifestSha256
      artifact_manifest = $manifestPath
      artifact_manifest_sha256 = $manifestSha256
      hosted_ci_head_sha = $hosted.ci_head_sha
      hosted_release_head_sha = $hosted.release_head_sha
  }
  $reviewIdentityJson = $reviewIdentity | ConvertTo-Json -Compress
  ```

  Oracle-high and Reviewer-high each inspect the full committed range, all global constraints, the exact post-freeze native manifest/files, local evidence, package/workflow evidence, hosted jobs, and the same overall manifest. Each receipt must repeat the exact serialized identity and explicitly pass. A timeout, partial response, different base/head/native-manifest/overall-manifest/hosted SHA, or verdict on an older revision is not acceptance.

- [ ] **Step 12: Enforce receipt invalidation**

  If either review requests any product/test/doc correction, if HEAD changes, if native QA is rerun and changes `native-hashes.json`, if another gate changes bound evidence, if the hosted run targets another SHA, or if either manifest changes, discard both receipts. With separate Git authorization, correct the issue, rerun Steps 4-11, and obtain both receipts again. Completion requires two explicit passes naming the identical base, head, native-manifest SHA-256, overall-manifest SHA-256, hosted CI SHA, and hosted release SHA.

---

## Dependency order and review boundaries

1. Task 1 makes visual authority and runtime discoveries permanent without changing runtime behavior.
2. Task 2 establishes the typed protocol, exact ETX path, mode inspection, and deterministic engine reset.
3. Task 3 consumes Task 2 to distinguish surviving applications from terminal completion, runs the same mode-clean sequence before actor reconnect replaces its transport, and adds the pane recovery action without changing credential/security semantics.
4. Task 4 replaces fixed product geometry with measured metrics and one geometry owner.
5. Task 5 wires environment invalidation and physical icon rendering to Task 4's measurement identity.
6. Task 6 introduces controller-preserving adaptive shell composition with a closed `ShellChildOwner` ledger and typed GTK detach/attach operations; generic `WidgetExt::unparent` is forbidden.
7. Task 7 uses Task 6's overlay/background to unify all modal surfaces.
8. Task 8 adds high-count/narrow-width reachability after the shell and modal boundaries are stable.
9. Task 9 binds Tasks 2-8 into typed P0 actions, real native screenshots, DPI metadata, and fail-closed cleanup.
10. Task 10 locks package/workflow contracts, conditionally commits first, freezes one identity/root, reruns native evidence under that root, freezes its hash manifest, then runs local/hosted/package gates and obtains both reviews over the exact native and overall manifests.

Each task is an independent RED→GREEN review boundary. Conditional semantic commits are suggested at those boundaries, but no Git write is part of planning and no implementation worker may commit without existing explicit user authorization.

## Requirement coverage matrix

| Spec requirement | Task(s) | Binary proof |
|---|---:|---|
| Research conclusions and selected approach | 1 | permanent whole/every-split parser tests, real interrupt fixture, updated design authority |
| Ctrl+C always exactly ETX; Ctrl+Shift+C copy; negotiated keys unchanged otherwise | 2, 9, 10 | model test, actor write capture, direct/PowerShell real ConPTY, P0 interrupt evidence |
| Surviving TUI is not auto-reset | 2-3, 9 | actor tracker and fixture remain alternate/enhanced until explicit Reset display |
| Exit/failure/crash/disconnect mode-clean final presentation | 3, 9 | lifecycle matrix, panic detach, stable status page, no old-TUI overlap/U+FFFD |
| Actor reconnect cleans old presentation before transport replacement | 3 | dirty alternate/Kitty ordered ledger: clean newer frame/event, old shutdown, replacement creation/connect with same actor/engine |
| Explicit display recovery preserves primary text/scrollback and clears modes/title | 2-3 | engine contract plus newer clean generation and cleared notice |
| Mutually exclusive terminal/recovery/disconnected pane layers | 3 | workspace model/native pane tests and detached-controller guard |
| Pango-measured metrics replace 9×18 everywhere | 4 | source scan, metric/geometry matrix, render/cursor/selection/hit-test tests |
| DPI 96/120/144/192 and font 6/15/72 | 4-5, 9-10 | pure metric/icon tests, real metadata, honest physical-scale ledger |
| Font/scale/DPI/color invalidation without session recreation or duplicate resize | 4-5 | identity/invalidation tests and exact `TerminalSize` emission count |
| Physical icon rendering/cache by icon/backend/physical size | 5, 8-10 | 16/20/24/32 texture tests, 18-icon native render, package startup evidence |
| Compact/Standard/Wide breakpoints and state preservation | 6, 9 | pure 800/1360/1920 decisions, closed owner/typed detach tests under fatal warnings, real screenshots, and source rejection of generic `unparent` |
| Product identity/status hierarchy | 6, 9 | widget-tree facts reject duplicate identity and bottom status strip |
| Tabs, five pane fixtures, pane priorities, compact drawer | 6, 8-10 | 20-tab keyboard/list tests, all five layouts in three widths, overflow/drawer tests |
| Modal scrim, geometry, scrolling, sensitivity, focus/Escape/return | 7, 9-10 | all-dialog real GTK tests and screenshot/accessibility matrix |
| Visual tokens, state coverage, motion, contrast, accessible icon controls | 1, 6-9 | theme contract, native widget states, names/tooltips, focus/contrast facts |
| Real screenshot state matrix and no clipping/hidden action/zero pane/missing icon | 9-10 | checkpoint table plus post-commit/exact-HEAD PNG facts and fatal-warning GTK files under `$reviewRoot/native` |
| Existing P0 SSH/vault/import/cleanup evidence is not weakened | 9-10 | unchanged eleven surfaces, identity-binding negative tests, P0 All |
| Workspace fmt/check/test/Clippy/module cap/single egress/engine/workflow | all, 10 | exact local commands and mutation-tested scripts |
| Release package and hosted Linux/macOS/Windows | 10 | exact-SHA CI/release jobs and three package artifacts |
| Oracle-high and Reviewer-high same identity | 10 | two receipts repeating the same base/head/native-manifest/overall-manifest/hosted tuple |
| Out-of-scope exclusions | all | unchanged manifests/security/storage/import/SSH contracts and dependency scans |

## Global-constraint coverage

| Global constraint | Enforced by |
|---|---|
| GTK4/Relm4/core boundary/single adapter | Tasks 2-10 plus `component_dependencies` |
| SSH/security/storage/import/reconnect semantics unchanged | Tasks 2-3, 9-10 regression suites; reconnect retains actor/engine and existing factory credential/host-key policy |
| Exact safety ETX | Tasks 2, 9, 10 |
| No automatic surviving-TUI reset | Tasks 3, 9 |
| Mode-clean terminal completion | Tasks 2-3, 9-10 |
| No fixed 9×18 | Task 4 source scan/tests |
| One measured geometry source | Tasks 4-5 |
| Embedded SVG/internal vectors only | Tasks 5, 8, 10 package checks |
| Physical icon cache key | Task 5 |
| Modal sensitivity/focus contract | Task 7 |
| Every production module <=250 pure LOC | every task plus recursive gate |
| No sensitive diagnostics/evidence | Tasks 2-3 redacted Debug and Tasks 9-10 no-secret/schema checks |

## Plan self-review result

- **Spec coverage:** PASS. Every section 0-9 and every global constraint maps to an implementation task and an executable proof in the two matrices.
- **Interface consistency:** PASS. `TerminalDisplayModes` originates in core and is consumed by engine, tracker, frame, report, and pane policy; `prepare_final_presentation` is the one mode-clean primitive used by terminal completion and the explicit reconnect branch before transport replacement; `ShellChildOwner` is the single sidebar-parent ledger used by typed detach/attach; `MeasuredFontMetrics` is owned by `TerminalViewModel`; native and overall manifest names/hashes are consistent from freeze through both review receipts.
- **Safety semantics:** PASS. Interrupt always writes one ETX and never calls negotiated encoding or recovery. Display recovery runs only for explicit Reset display, terminal completion, or an explicit reconnect command; reconnect publishes clean state before old shutdown/replacement creation while retaining actor/engine and security semantics; a surviving dirty frame otherwise shows Reset display.
- **Module boundaries:** PASS. Near-cap protocol, actor I/O, terminal model, pane policy, icon registry, main-window layout, smoke, and report responsibilities are split before expansion; the recursive 250 pure-LOC and one-egress tests run at every boundary.
- **Failure semantics:** PASS. Invalid Pango metrics, unsupported Alacritty recovery APIs, transport writes, reconnect recovery/old-shutdown failure, stale session identity, mismatched `ShellChildOwner`, duplicate resize, invalid scale/icon size, modal focus loss, screenshot/schema gaps, process/secret cleanup, native-manifest drift, package drift, hosted SHA drift, and review identity drift all fail closed.
- **Scope:** PASS. No dependency, framework/backend, shell integration, heuristic prompt parsing, P1 SSH, security/persistence, or unrelated refactor is introduced.
- **Marker/shape scan:** PASS. The plan has exactly 10 tasks and 81 coherently numbered checkbox steps, with no placeholder or deferred-decision markers, placeholder paths, trailing whitespace, or incomplete code/test/command markers.
- **Agent-executable QA:** PASS. RED/GREEN commands, fatal-warning breakpoint crossings, ordered dirty reconnect, post-commit exact-HEAD Windows/GTK actions under `$reviewRoot/native`, scale/native manifests, local/hosted queries, overall artifact hashes, and receipt invalidation are explicit and PowerShell-compatible. Unavailable physical scales are reported rather than presented as real captures.

**Plan review receipt status:** `waiting for receipt`. This planner session does not dispatch plan-critic, Oracle, Reviewer, or implementation workers; formal review and execution are orchestrator-owned.
