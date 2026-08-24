# rsHell Fluent Redesign and Task21 Completion Design

## 1. Purpose and authority

This specification directs the combined Fluent shell redesign and Task21 local CI and package-contract completion. It is the authoritative target design for this work. `DESIGN.md` records the current extracted system and remains useful as the baseline, but where it conflicts with this document during this change, this document wins.

The work changes presentation, embedded visual resources, native visual evidence, and the existing local workflow and package contracts. It does not change connection, session, terminal, credential, storage, SSH, or cross-platform product semantics.

## 2. Exact global constraints

The implementation must satisfy each of these constraints exactly:

- Retain GTK4/Relm4 and cross-platform architecture.
- Use one unified Windows Terminal Dark / Fluent 2 shell.
- Use a custom command bar, not a GNOME HeaderBar visual.
- Use a NavigationView-like dense sidebar, Windows Terminal tabs, a compact pane command row, and ContentDialog/settings-style overlays.
- Bundle accessible SVG icons compiled and embedded into the binary. Do not ship an external icon payload.
- Use Segoe UI as the system-first chrome face with sensible cross-platform system fallback, and use Cascadia Mono, JetBrains Mono, Consolas, then monospace for terminal content.
- Use 4px controls, 8px overlays, a 2px focus indicator, and 80 to 120ms state motion.
- Do not add fake Mica, blur, gradients, shadows, cards, or emoji.
- Preserve terminal canvas behavior and all core, session, and storage semantics.
- The only command egress remains `command_port::dispatch -> UiCommandPort::try_send`.
- Keep production modules at or below 250 pure production lines and add no UI infrastructure dependencies.
- Do not perform Git writes or install software.
- Complete UI and resources first, update native smoke and screenshot evidence second, then finalize Task21 local workflow and package contracts using embedded assets.
- Treat hosted Linux, macOS, and Windows runs as later external evidence. This work must not claim that hosted CI evidence exists.

## 3. Architecture and ownership

The crate boundaries remain unchanged. `rshell-ui` owns GTK widget composition, CSS-class assignment, embedded icon lookup, accessibility metadata, and projection of existing view models. `rshell-core`, `rshell-session`, `rshell-storage`, and `rshell-platform` retain their public data, lifecycle, persistence, SSH, vault, and file-selection responsibilities.

`MainWindow` continues to compose the sidebar, editor, tab bar, pane host, settings, import, and interaction controllers. Replace the visual use of `gtk::HeaderBar` in `main_window_layout.rs` with a purpose-built horizontal command bar, but retain the existing window, controller, overlay, and message topology. The new command bar presents product identity at the leading edge and Import and Settings at the trailing edge. Window decorations remain native. It must not imitate a GNOME header bar or create a Windows-only title-bar integration.

The following focused modules own the visible change. A helper may be introduced only when it has one clear role and preserves the 250 pure-line limit.

| Area | Current owner | Design responsibility |
|---|---|---|
| Main shell | `main_window_layout.rs`, `main_window_init.rs` | Custom command bar, shell regions, native-window composition, overlay host |
| Connections | `connection_sidebar.rs`, `connection_sidebar_widgets.rs` | Dense NavigationView-like hierarchy, selection indicator, search, action strip, contextual error and delete confirmation |
| Tabs | `session_tab_bar.rs` | Windows Terminal-style fixed-dark tab strip, new-tab affordance, selected state, close affordance, rejection text |
| Panes | `pane_host_render.rs` | Compact command row above each pane, fixed-dark status and error surfaces, embedded icons |
| Editor and dialogs | `connection_editor_*`, `settings_window_*`, `import_dialog_*`, `interaction_dialog_*` | ContentDialog-like chrome, explicit errors, native focus and secret behavior |
| Theme and icons | `theme.rs`, `resources/style.css`, new bundled SVG source directory | Tokenized Fluent CSS and binary-embedded icon registry |
| Smoke evidence | `main_window_smoke_capture.rs`, smoke report types, `scripts/qa/p0-smoke.ps1` | Semantic visual-contract report plus screenshot capture and validation |

Component outputs continue to flow to `MainWindow`, which remains the single command gateway. No widget may call a core actor, repository, session handle, filesystem API, or `UiCommandPort` directly. Component-local messages may update local visual state, but a product command follows the existing route through `MainWindow::dispatch`, then `command_port::dispatch`, then `UiCommandPort::try_send`.

## 4. Fluent visual system

The current style sheet already establishes a dense, borders-first dark terminal workspace. The redesign turns that direction into a single deliberate shell rather than mixing adaptive GTK chrome with a separate terminal theme. Application chrome uses a restrained Fluent dark neutral scale. The terminal canvas, tabs, pane command rows, status, and terminal context menu stay dark, with no translucent effects.

### Tokens

| Token | Value or rule | Use |
|---|---|---|
| `font-ui` | `"Segoe UI", system-ui, sans-serif` | Command bar, sidebar, dialogs, labels. GTK resolves the platform fallback when Segoe UI is unavailable. |
| `font-terminal` | `"Cascadia Mono", "JetBrains Mono", Consolas, monospace` | Terminal canvas only. |
| `surface-shell` | Fluent dark neutral, opaque | Command bar, sidebar, dialogs, non-terminal workspace chrome. |
| `surface-terminal` | Existing `#1a1a1a` | Terminal canvas and pane body. |
| `surface-tab` | Existing `#1e1e1e` | Tabs and pane command rows. |
| `surface-overlay` | Opaque Fluent dark neutral | Dialog and popover surfaces. |
| `text-primary` | High-contrast neutral text | Labels and active controls. |
| `text-secondary` | Muted neutral text | Metadata, inactive tabs, supporting labels. |
| `accent` | Existing cyan `#60cdff` family | Primary action, selection rule, focus companion, and status only. |
| `danger` | Existing error family | Destructive affordances and changed-host warning, always paired with text. |
| `border` | One-pixel opaque or low-alpha neutral boundary | Region separation and input outlines. |
| `control-radius` | 4px | Buttons, fields, rows, tabs, and command controls. |
| `overlay-radius` | 8px | Dialogs, menus, and popovers. |
| `focus-width` | 2px | Visible keyboard focus, including icon-only controls and terminal canvas. |
| `motion-fast` | 80ms | Row and destructive hover feedback. |
| `motion-standard` | 100ms | Command bar, tab, and button state feedback. |
| `motion-focus` | 120ms | Field, primary-action, scrollbar, and focus-color feedback. |

Use a compact 4px spacing rhythm, with 8px as the standard group and overlay inset. One-pixel dividers separate regions. The shell should look dense and operational, not card-based. Keep the existing 1360 by 860 default window, 232px sidebar starting width, resizable `GtkPaned` boundary, and terminal font behavior unless normal GTK allocation requires a smaller arrangement.

All motion is state acknowledgement only. It must honor `gtk-enable-animations`, never block a command, and never be the only indicator of status, selection, error, or focus. There are no gradients, blur, fake Mica, drop shadows, emoji, decorative entrance animation, or platform-specific visual APIs.

## 5. Embedded resource and icon strategy

Create a small source-controlled SVG icon set under `resources/icons/` for every product-owned action now represented by GTK symbolic names: import, settings, add connection, add group, edit, duplicate, delete, close tab, new tab, split horizontal, split vertical, reconnect or retry, copy diagnostics, warning, secret present, and host-trust state. Icons are monochrome Fluent-style SVGs sized for the existing compact controls. They use `currentColor`, have no embedded raster, external reference, script, font, animation, gradient, or filter.

`rshell-ui` exposes a narrow icon registry keyed by a closed Rust enum rather than accepting arbitrary paths or icon-theme names. The registry embeds each SVG at compile time with `include_bytes!` and produces the GTK image or texture used by buttons and status content. It also centralizes accessible labels. A visible text label remains on commands that have room; icon-only controls must receive a descriptive accessible label and tooltip from the same action metadata. No icon is semantic information by itself.

The compiled executable is the only delivery vehicle for product SVGs. The Windows release package continues to contain GTK runtime files only where GTK itself requires them. It gains no icon directory, image loader requirement, copied SVG, or package validation exception. `theme.rs` keeps embedding `resources/style.css`; the icon registry follows the same binary-embedding model without restoring the removed GResource manifest.

## 6. Component behavior

### Command bar and sidebar

The command bar replaces the HeaderBar visual with a flat opaque shell row. It contains the rsHell identity, a concise optional status context, then Import and Settings commands. Commands use the embedded icons, native GTK buttons, tooltips, labels, and existing `MainWindowMsg` routing. They remain keyboard focusable and preserve native window decorations.

The sidebar is a dense NavigationView-like navigation surface, not a set of cards. It retains search, group hierarchy, selection, activate-to-connect behavior, create, edit, duplicate, delete, confirmation, and command-rejection state. Group depth remains 12px per level. Selection receives a clear accent-adjacent left rule plus a non-color surface change. Rows preserve name, metadata, and tags, but trim or ellipsize before changing behavior. Empty search and no-result states are explicit text surfaces. Disabled toolbar actions retain their reason in adjacent text or a tooltip and use GTK sensitivity.

### Tabs and panes

`SessionTabBar` remains driven only by the authoritative `WorkspaceState`. It continues to avoid optimistic insertion and uses local visual activation only until the next authoritative workspace update. The redesign changes tab geometry and embedded close/new icons, not tab lifecycle. Active tabs use a flat dark surface and cyan rule; inactive tabs remain legible. Command rejection stays visible text below or adjacent to the tab command area and is not represented only by color.

`pane_host_render.rs` retains the existing `PaneProjection`, split ratio projection, focus activation, and page mapping. Each leaf gets a compact pane command row with status text at the leading edge and embedded action icons at the trailing edge. The action set remains exactly projection-driven. Pending, unavailable, exited, failed, and crashed panes keep their existing text and action order: Retry, optional Edit Connection, Copy Diagnostics, Close. The terminal canvas itself, its renderer, input, selection, search, clipboard, geometry, and frame handling do not change.

### Editor, settings, import, and interaction overlays

Connection editing, settings, import preview, host trust, authentication, and destructive confirmation become consistent ContentDialog-style opaque overlays. They use an 8px overlay radius, compact native controls, persistent labels, visible focus, explicit textual validation, and a clear footer action hierarchy. The editor retains its current fields and inherit-versus-explicit override model. Settings retain dirty-state and authoritative acceptance behavior. Import retains stable candidate IDs, disabled wildcard rows, fresh-preview retry, and secret-present warning semantics. Host-key and authentication overlays retain fail-closed choices and one-time secret transfer and clearing.

Close, cancel, Escape, Enter, Ctrl+Enter, validation, disabled states, error routing, and focus return retain their current semantics. Do not convert a dialog into a browser-style modal, alter credential visibility, or add an acceptance route for changed host keys.

## 7. State, errors, and accessibility

Every interactive element must be a native GTK focusable control where applicable. The tab bar, pane row, sidebar toolbar, dialog footer, search, editor fields, and icon-only buttons expose a programmatic name. Tooltips add convenience but are never the only name. Persistent labels remain associated with form fields. Errors remain adjacent textual content and do not disclose secret values, paths, tokens, or credentials.

Use the 2px focus indicator on keyboard focus in both shell and fixed-dark terminal regions. Selected rows and tabs must have a text, geometry, or surface distinction in addition to accent color. Pending and error pages retain their text labels. Dynamic status should use the existing GTK-accessible mechanisms where available, but no claim of screen-reader announcement quality is made until native assistive-technology testing occurs.

Command dispatch failures remain centralized: `MainWindow::dispatch` updates status, forwards `CommandRejected` to the originating component, and fails an active smoke run as it does today. The visual refresh must not swallow, retry, or reroute an error. Button transitions may acknowledge the failure, but no action can dispatch twice because of animation.

## 8. Task20 smoke and screenshot evidence

The existing P0 harness produces a PNG but validates only its signature and nonzero dimensions. The redesign raises this to a local native visual contract without replacing the current smoke scenario or weakening existing Task20 evidence.

The production smoke driver adds a bounded visual evidence record captured after the main window is realized and the deterministic shell state is present. It records the realized command bar, dense sidebar, tab strip, pane command row, terminal canvas, overlay style state, and embedded-icon resolution as semantic facts. The capture remains `capture_widget_png` from the realized GTK widget. The report identifies the screenshot path and a fixed, documented viewport. It must not include secrets, user file paths, hostnames from private configuration, or arbitrary widget text.

The P0 scenario gains a visual checkpoint that opens a representative non-secret overlay before capture, then closes it through the existing action path. The harness validates: PNG structure and dimensions; a nonempty image; expected dark-shell regions at documented sample areas; a visible 2px focus or selection treatment in the checkpoint state; and the presence of the structured visual facts. Checks use stable color families and geometry ranges, not a fragile full-image pixel baseline. Existing terminal, SSH, vault, import, split, cleanup, JSON, JUnit, artifact finalization, and secret-scan rules remain mandatory.

Native local Windows evidence is the first required execution target after the UI work. Linux and macOS screenshot observations remain external hosted evidence until a later authorized run. A passing local screenshot proves only the specified local native contract, not cross-platform rendering parity or hosted CI success.

## 9. Task21 workflow and package contracts

Task21 keeps the current product scope of `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `scripts/qa/p0-smoke.ps1`, `scripts/qa/workflow-contract.ps1`, `scripts/qa/assert-package.ps1`, and `tests/p0_acceptance.rs`. This design does not add release targets, dependencies, product features, or a second CI system.

The local contract changes only as needed to prove the embedded-resource design:

1. Add workflow-contract assertions that the release package is validated after a binary produced with embedded CSS and icons, and that no product icon payload is copied into Unix or Windows archives.
2. Add package assertions that the archive has the existing target-specific root layout, executable architecture, GTK runtime requirements, and no external rsHell icon/resource payload. The package startup smoke must still pass with the binary's embedded CSS and SVG assets.
3. Add acceptance assertions that the icon registry is closed, every required action has an embedded asset and accessible label, and Task20 finalization preserves the visual evidence contract without secret leakage.
4. Preserve the existing PowerShell default-shell, locked Cargo, matrix, least-privilege, real-service setup, fail-closed gate, release artifact, and removed-dependency checks.

The Windows packaging logic must continue to stage GTK DLLs, GLib schemas, GDK Pixbuf runtime data, and fontconfig data only as GTK runtime dependencies. The embedded SVG strategy must not require image-loader cache changes beyond the existing relocatable cache validation. Unix archives retain `rshell`, `LICENSE`, and `README.md`; Windows retains `rshell.exe` and required GTK runtime files. Neither archive gains `resources/`, `icons/`, or loose product SVG files.

The release workflows are intentionally not run, committed, pushed, tagged, or altered beyond the existing Task21 contract scope in this documentation task. Hosted results may be reported only after a separately authorized Git write and hosted execution.

## 10. Cleanup, rollback, and residual risk

The P0 harness retains its current parent-owned temporary roots, child-process tracking, agent identity ledger, vault ledger, final secret scans, and fail-closed finalization ordering. UI capture writes only to the already-owned smoke artifact path, and that artifact remains subject to the same secret scan. It must be removed on failed finalization just as the current PNG is removed.

The local Windows `ssh-agent` residual is known: the service may remain `Running` with `Manual` startup because restoring a prior `Stopped` and `Manual` state lacked service permission. Task-owned scripts and hosted workflow cleanup must continue to record the baseline, restore exact status and startup type, verify both, and fail if restoration fails. This specification does not authorize an administrator action, manual service mutation, privilege escalation, or any mutation outside the task-owned cleanup path.

Rollback is file-scoped. Revert the new command-bar, icon-registry, CSS, component, smoke, and local-contract changes together if the native smoke fails, an embedded icon cannot render on a supported GTK runtime, package startup cannot find its GTK runtime, or the redesign introduces a semantic regression. Do not roll back core, session, storage, SSH, vault, or terminal behavior because this design does not modify them. Preserve failed redacted smoke reports, logs, JUnit, and permitted screenshots for diagnosis, then remove only the harness-owned temporary roots using existing safety checks.

Primary risks are GTK renderer variation, platform font fallback, SVG decoder availability, screenshot assertion fragility, package runtime omission, and accidental command-path bypass. The mitigations are simple SVGs without unsupported features, system-first font fallback, binary embedding, semantic visual facts paired with range-based image checks, package startup smoke, existing package validation, and a code search or test assertion that all egress still passes through `command_port::dispatch`.

## 11. Test and evidence matrix

| Area | Local evidence required before completion | External evidence still required later |
|---|---|---|
| Tokens and CSS | CSS-focused unit or source assertions for opaque surfaces, 4px controls, 8px overlays, 2px focus, 80 to 120ms motion, and prohibited visual effects | Rendered contrast and high-contrast inspection on each hosted platform |
| Embedded icons | Registry coverage test, SVG safety checks, accessible-label coverage, native GTK construction with every required icon | Native rendering observation on hosted Linux, macOS, and Windows |
| Command bar, sidebar, tabs, panes | GTK widget construction and state tests for focus, selection, disabled state, rejection text, and projection-driven actions | Manual keyboard and screen-reader observation on each platform |
| Dialogs and secure interaction | Existing Task18 widget and view-model tests, plus new visual class and focus checks | Assistive-technology announcement and contrast validation |
| Terminal | Existing terminal render, input, search, selection, clipboard, resize, split, reconnect, and latest-frame tests unchanged | Native visual inspection of font fallback and fixed-dark contrast |
| Task20 P0 | `p0_acceptance`, `p0-smoke.ps1 -Mode All`, JSON/JUnit, secret scan, visual facts, PNG geometry and range checks, local Windows smoke capture | Hosted matrix P0 artifacts and screenshots |
| Task21 workflow | `workflow-contract.ps1`, acceptance tests for contract markers, locked workspace checks where the local GTK environment supports them | GitHub Actions execution for Linux, macOS, and Windows |
| Package | `assert-package.ps1` against locally built package where toolchain and GTK runtime are available, including startup smoke and no-external-icon-payload assertion | Hosted release archives and target-native startup smoke |

The completion report must distinguish executed local evidence from unexecuted hosted evidence. It must never report a hosted pass from static workflow inspection.

## 12. Explicitly out of scope

- Replacing GTK4, Relm4, the current cross-platform architecture, or native window decorations.
- Windows-only title-bar APIs, Mica, acrylic, blur, gradients, shadows, cards, emoji, web UI infrastructure, or new UI dependencies.
- Changes to terminal rendering, PTY behavior, SSH command construction, session actors, connection data, storage schema, credential handling, vault semantics, import semantics, or command routing.
- New release targets, release channels, package formats, external icon payloads, icon themes, or downloaded runtime assets.
- Changing the existing Task21 workflow and package product scope beyond contracts needed for the embedded-resource and visual-evidence design.
- Claiming hosted CI, hosted package, or cross-platform visual evidence before it has been run through an authorized workflow.
- Git commits, pushes, tags, software installation, or administrative `ssh-agent` service repair.
