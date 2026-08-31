# Terminal Recovery, HiDPI, and Adaptive Fluent UI Design

**Date:** 2026-08-28

**Status:** Approved by unambiguous self-review after runtime discovery

**Scope:** Repair terminal interruption recovery, replace fixed terminal geometry with measured HiDPI geometry, and redesign the existing GTK4/Relm4 shell into an adaptive terminal-first Fluent interface. Core SSH, storage, credential, host-key, and import semantics remain unchanged.

## 0. Research log

The design is grounded in current-HEAD runtime evidence rather than the previous green smoke result.

- Real ConPTY Ctrl+C sent ETX and interrupted the foreground fixture, but the shell returned while `RenderFrame.alternate_screen` remained true. The new prompt rendered over retained TUI content.
- The same OSC title, 1049 alternate-screen, and UTF-8 sequence produced identical frames at every two-chunk split. Split parsing is not the root cause for this reproduction.
- Kitty keyboard negotiation changed Ctrl+C from `03` to `ESC [ 99 ; 5 u`. A stale enhanced-keyboard mode can therefore become visible escape text in an ordinary shell after interruption.
- The captured frames contained no replacement character. The observed corruption is terminal-mode residue, not demonstrated UTF-8 corruption.
- Pango at the configured 15pt measured an ASCII cell near 12×24 logical pixels and a CJK glyph near 20×21. Product geometry is fixed at 9×18.
- At GDK scale 2, PTY pixel dimensions and DPI doubled correctly, but the embedded icon source remained 16×16 and the saved PNG remained logical resolution. Existing screenshots cannot prove HiDPI sharpness.
- Current screenshots show duplicate product identity, weak visual hierarchy, an oversized settings form, icon-only navigation, no modal scrim, and rigid component widths.

References loaded: project `DESIGN.md`; frontend design router, interaction mechanics, and perfection rules; Rust debugging setup, investigation, QA, and cleanup rules. Context7 documentation lookup was attempted but unavailable due quota, so GTK/Pango APIs must be compile-proven against the pinned local crates during implementation.

## 1. Product intent

rsHell is a terminal-first SSH workspace. Terminal output, connection identity, and session state must dominate the interface. Interruption must be predictable and recoverable. Font size, monitor DPI, and window width must not change cell addressing, blur icons, hide controls, or destroy workspace state.

The target remains native GTK4/Relm4 with Windows Terminal Dark / Fluent 2 influence. This is not a framework rewrite, a terminal-engine replacement, or a P1 feature expansion.

## 2. Global constraints

1. GTK4/Relm4, the core-owned application boundary, and the single UI command adapter remain.
2. SSH authentication, host-key verification, credential storage, journaling, imports, and reconnect security semantics do not change.
3. Ctrl+C without Shift/Alt/Super is a safety interrupt and emits exactly one ETX byte, independent of Kitty/CSI-u state.
4. A single Ctrl+C must not automatically destroy a TUI that catches the interrupt and continues.
5. Session exit, failure, crash, and disconnect must publish a mode-clean final presentation.
6. Product terminal geometry must not contain a fixed 9×18 assumption.
7. The same measured metrics drive rendering, cursor, selection, hit-testing, rows/columns, and PTY pixel dimensions.
8. Icons remain source-controlled embedded SVG/internal vectors; no external icon payload is introduced.
9. Product icons are rendered for the effective physical target and cached by icon/backend/physical size.
10. Background widgets are insensitive while a modal overlay is open; focus is contained and restored.
11. Production modules remain at or below the repository's 250 pure-LOC limit.
12. Secrets, paths, endpoints, and terminal text do not enter public diagnostics, reports, screenshots, or Debug output beyond existing redacted contracts.

## 3. Approaches considered

### A. Safe recovery plus adaptive shell — selected

Add an explicit interrupt command, preserve active applications, expose deterministic display recovery, measure fonts at runtime, render scale-aware icons, and adapt the existing component composition at allocation breakpoints.

Benefits: addresses observed causes, keeps architecture, permits exact TDD, and limits regression scope. Cost: coordinated session/UI work and a broader native screenshot matrix.

### B. Immediate terminal reset on every Ctrl+C — rejected

This would hide alternate-screen residue quickly, but it would break applications that catch Ctrl+C and remain active. It conflates interrupt intent with confirmed foreground-process exit.

### C. Replace GTK or the terminal engine — rejected

The evidence does not implicate GTK4/Relm4 or Alacritty parsing as the primary interruption defect. A rewrite would discard working SSH/storage/session boundaries and multiply risk without resolving the lifecycle contract.

## 4. Terminal interruption and disconnect recovery

### 4.1 Explicit interrupt path

`TerminalView` maps unmodified Ctrl+C to a typed `SessionUiCommand::Interrupt`. Ctrl+Shift+C remains copy. Other configured and negotiated keys continue through the terminal encoder.

The actor handles `Interrupt` by writing exactly `[0x03]` to the active transport. It records an actor-owned interruption observation containing the presentation generation and whether alternate screen or enhanced keyboard/mouse modes were active. It does not immediately reset the backend.

This guarantees that stale Kitty state cannot convert the safety interrupt into visible CSI-u bytes. Raw-mode applications still receive ETX as data; canonical PTYs can convert it into SIGINT.

### 4.2 Safe display recovery

The engine exposes a focused `recover_display` operation that:

- returns to the primary screen;
- clears enhanced keyboard, mouse-reporting, application-cursor, hidden-cursor, and stale title state;
- preserves primary-screen text and scrollback;
- creates a fresh presentation generation.

Recovery is automatic only when the session itself exits, fails, crashes, or disconnects. For a surviving shell after Ctrl+C, the pane detects that the next authoritative frame is still alternate/enhanced and presents a compact `Display mode not restored` status with an accessible `Reset display` action. The user chooses recovery, avoiding damage to a TUI that caught Ctrl+C.

If a future shell-integration prompt marker proves foreground return, it may invoke the same command, but this design does not add shell-profile injection or heuristic prompt matching.

### 4.3 Visible terminal states

Terminal, recovery notice, and disconnected state are mutually exclusive presentation layers in one pane. A terminal controller is not fed after it is detached. Disconnect produces one stable status page with Retry, Copy diagnostics, Edit connection when applicable, Reset display when residue exists, and Close.

## 5. Measured terminal geometry and HiDPI

### 5.1 Font metrics

Replace `terminal_font_metrics() -> 9×18` with a GTK-main-thread measurement service using the resolved terminal font and Pango context.

- `cell_width`: ceiling of the monospace approximate character width and measured ASCII advance.
- `cell_height`: ceiling of ascent + descent plus the configured line spacing token.
- Wide cells always occupy two grid cells. Fallback glyphs are clipped/centered inside one or two cell rectangles; their raw glyph advance never changes terminal column identity.
- Metrics are positive, finite, and fail closed to a documented measured fallback only when Pango cannot resolve the configured font.

`TerminalViewModel` owns current metrics. Rendering, cursor rectangles, pointer-to-cell mapping, selection, resize rows/columns, and PTY pixel dimensions consume the same value.

### 5.2 Runtime invalidation

Metrics and render caches are invalidated when any of these change:

- terminal font family or size;
- widget scale factor;
- effective GTK font DPI/settings;
- color/profile rendering identity.

The terminal emits one resize only when computed `TerminalSize` changes. Changing monitor, scale, or font does not recreate the session.

### 5.3 Scale-aware icons and captures

`ProductIcon` renders SVG/internal vector output at `logical_size × effective_scale` physical pixels. The cache key is `(icon, backend, physical_size)`. GTK still allocates a 16 logical-pixel icon.

Visual evidence records logical widget size, downloaded texture size, effective scale/DPI, measured cell metrics, and icon physical source size. A logical-size PNG alone is not accepted as HiDPI evidence.

## 6. Adaptive terminal-first Fluent shell

### 6.1 Layout modes

Layout is selected from the main window's logical allocation without recreating controllers or reducer state.

| Mode | Width | Behavior |
|---|---:|---|
| Compact | `< 900` | 48px navigation rail; connection list opens as a drawer; global actions are icon buttons; pane actions use overflow. |
| Standard | `900–1439` | 240px resizable sidebar; compact text/icon command actions; terminal owns remaining space. |
| Wide | `>= 1440` | Sidebar may grow to 280px; forms remain capped; terminal expands instead of forms stretching. |

Crossing a breakpoint preserves active tab, pane focus, sessions, search, selection, and unsaved editor draft.

### 6.2 Command and status hierarchy

The native title bar owns the product name. The in-content command bar no longer repeats `rsHell`; it presents New session, Import, Settings, and concise active-workspace status. The 20px global status strip is removed; session state belongs to tabs and pane command rows.

### 6.3 Navigation, tabs, and pane actions

- Standard/wide navigation uses readable text plus icons for primary actions; compact mode uses accessible icons and a drawer.
- Tabs live in a horizontal scroller with active-tab auto-reveal and an overflow list. Twenty tabs remain reachable by keyboard.
- Pane actions have priority groups. Split, reconnect/retry, and close remain visible where possible; diagnostics/edit move into an overflow menu at narrow pane widths.
- All five pane layouts keep non-zero terminal allocations and accessible controls in all three window modes.

### 6.4 Modal surfaces

Connection editor joins Settings, Import, and Interaction in the main `GtkOverlay`. Every modal has:

- an opaque scrim;
- a max width of `min(680px, window width - 48px)`;
- a fixed header/footer and scrollable body;
- background sensitivity disabled;
- initial focus, contained Tab order, Escape cancel, and focus return to its trigger.

Settings and editor fields are grouped into clear sections instead of one undifferentiated grid.

### 6.5 Visual tokens

The palette remains opaque Fluent dark, but hierarchy is strengthened:

- primary text uses full `content-primary`; no 40–50% opacity for operational labels;
- one spacing rhythm based on 4px with compact 2px exceptions only for borders/separators;
- UI type is 13/14px with explicit root font configuration;
- accent is limited to focus, selected tab/pane/row, and primary action;
- no gradients, fake Mica, card stacks, or decorative shadows.

`DESIGN.md` must be updated before these tokens/components are implemented.

## 7. Interaction and accessibility

Every changed primitive covers default, hover, focus, press, disabled, loading/pending, success, and error states where applicable. Motion remains 80/100/120ms, never delays input, and honors GTK animation settings.

Keyboard-only operation must support local tab creation, connection selection/edit/save/cancel, tab switching, pane switching/splitting/closing, dialog actions, interrupt, display reset, and reconnect. Icon controls require non-empty accessible names and tooltips. Focus indicators maintain at least 3:1 contrast; primary text maintains at least 4.5:1.

## 8. Verification contract

### 8.1 Interruption and disconnect

1. A fixture enables alternate screen, OSC title, Kitty keyboard, mouse reporting, and UTF-8. Ctrl+C sends exactly `03`, never `ESC[99;5u`.
2. A fixture that catches Ctrl+C stays in alternate screen and is not auto-reset.
3. A fixture that exits without restoring modes exposes the recovery notice. `Reset display` yields `alternate_screen=false`, primary shell prompt, cleared enhanced modes, no old-TUI/prompt overlap, and no U+FFFD.
4. Exit/failure/crash/disconnect from primary and alternate states automatically publish a mode-clean final status.
5. Whole and split OSC/1049/UTF-8 feeds remain identical.

### 8.2 DPI and fonts

1. Pure metric/render tests cover effective DPI 96/120/144/192 and font sizes 6/15/72.
2. ASCII, combining text, CJK, emoji fallback, cursor, selection, and mouse hit-testing share measured geometry and do not overlap or clip outside their assigned cells.
3. Changing font or scale recomputes metrics, invalidates cache, and emits the exact PTY resize without restarting the session.
4. Icon physical source is at least logical size × effective scale at 100/125/150/200% evidence points.
5. Real Windows screenshots are captured at every available physical scale; unavailable system scales are reported rather than synthesized as real evidence.

### 8.3 Layout and visual QA

1. Realized screenshots at approximately 800×600, 1360×860, and 1920×1080 cover empty, connected, 20-tab, nested-split, editor, settings, import, host-key, auth, failure, and recovery states.
2. No horizontal clipping, hidden primary action, zero-size pane, missing icon, duplicate identity, or unbounded form width is accepted.
3. Modal scrim/background insensitivity/focus containment/Escape/focus-return are asserted on real GTK widgets.
4. Keyboard-only navigation and accessibility names/roles/disabled states are inspected.

### 8.4 Regression gates

- Targeted RED→GREEN tests precede implementation.
- Full workspace tests/check/Clippy/fmt/module-cap/workflow contracts pass.
- Real local terminal interruption and DPI/window matrix pass under `G_DEBUG=fatal-warnings`.
- P0 smoke is expanded without weakening existing SSH/vault/import/cleanup evidence.
- Hosted Linux/macOS/Windows CI and release package jobs pass before final acceptance.
- Oracle-high and Reviewer-high approve one common current identity.

## 9. Out of scope

- Replacing GTK4/Relm4 or Alacritty.
- New SSH forwarding, SFTP, proxy, jump-host, logging, broadcast, or workspace features.
- Shell-profile injection or heuristic prompt parsing.
- Theme marketplace, Mica/acrylic effects, or platform-specific WinUI code.
- Changing credential, host-key, import, or persistence semantics.
