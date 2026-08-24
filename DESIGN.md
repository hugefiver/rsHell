# rsHell Design System

## 0. Task21 Fluent shell authority

This document records the current implemented visual system. For the Task21 change, `docs/superpowers/specs/2026-08-24-fluent-task21-design.md` is the source of truth; this file mirrors that approved design and no longer authorizes adaptive GNOME HeaderBar chrome.

The design was derived from the existing GTK4/Relm4 application, `resources/style.css`, and the realized Windows Task20 screenshot. The framework remains GTK4/Relm4 so the product stays native and cross-platform; the product-owned shell is now a unified Windows Terminal Dark / Fluent 2 presentation rather than a GTK-theme-dependent light shell around a dark terminal.

## 1. Identity and tokens

rsHell is a dense operations console. Connection identity, session state, and terminal content remain dominant. Depth comes from opaque tonal layers and one-pixel boundaries, not cards or decorative effects.

| Token | Value |
|---|---|
| font-ui | `"Segoe UI", system-ui, sans-serif` |
| font-terminal | `"Cascadia Mono", "JetBrains Mono", "Consolas", monospace` |
| surface-shell | opaque Fluent dark neutral |
| surface-command | `#202020` |
| surface-sidebar | `#202020` |
| surface-control | `#2b2b2b` |
| surface-control-hover | `#333333` |
| surface-overlay | `#2b2b2b` |
| surface-tab | `#1e1e1e` |
| surface-terminal | `#1a1a1a` |
| content-primary | `#f5f5f5` |
| content-secondary | `#cccccc` |
| content-tertiary | `#9d9d9d` |
| border-default | `rgba(255, 255, 255, 0.10)` |
| border-strong | `rgba(255, 255, 255, 0.16)` |
| accent | `#60cdff` |
| accent-hover | `#78d6ff` |
| accent-active | `#4ab5e6` |
| accent-content | `#002b40` |
| control-radius | 4px |
| overlay-radius | 8px |
| focus-width | 2px |
| motion-fast | 80ms |
| motion-standard | 100ms |
| motion-focus | 120ms |

Semantic danger, warning, and success colors must always be paired with visible text or an accessible name. Accent is reserved for focus, selection, current state, and primary action.

## 2. Typography

- Application chrome prefers Segoe UI and falls back to the active system sans-serif face.
- Terminal content prefers Cascadia Mono, then JetBrains Mono, Consolas, and the platform monospace face.
- Primary chrome and row text is 13px; input text is 14px; metadata and overlines are 11px; status text is 12px.
- Connection names use weight 600 rather than increased size. Required instructions and errors never rely on 11px metadata text.

## 3. Geometry and layout

- Default requested window size is **1360×860**. Smoke evidence reports the actual realized size instead of assuming the request was honored.
- The sidebar starts at **232px**, remains resizable, and indents nested groups by 12px per level.
- Spacing follows a dense 2px base grid: 2, 4, 6, 8, 10, 12, 14, and 20px roles.
- Standard controls use a 4px radius. ContentDialog and popover surfaces use an 8px radius.
- Fields and primary dialog actions have a 30px minimum height. Compact icon controls use 22–28px geometry.
- Focus is a visible 2px cyan outline or underline with sufficient contrast.
- Native window decorations remain enabled. The product shell begins with a custom in-content command bar; it does not replace the operating system title bar.

## 4. Shell architecture

### Command bar

The top in-content command bar carries product identity, global import and settings actions, and concise application status. It uses product-owned accessible icons and never duplicates command dispatch. All commands continue through `command_port::dispatch` and `UiCommandPort::try_send`.

### Navigation sidebar

The left pane follows a compact NavigationView pattern: section title, search, a small action row, grouped connection rows, and explicit confirmation/error states. Rows are transparent at rest, gain a tonal hover state, and use an accent edge plus tonal fill when selected. Connection data is not presented as cards.

### Session tabs and pane command row

The tab strip follows Windows Terminal geometry: dark continuous rail, compact tabs, clear selected accent, product-owned close action, and a dedicated new-tab action. Tabs remain core-confirmed; no optimistic tab insertion is allowed.

Each terminal leaf uses a compact pane command row for state, split, reconnect, copy diagnostics, edit, and close. `PaneTree` projection, actor ownership, shutdown-before-collapse, and retry semantics are unchanged.

### Terminal

The terminal canvas remains `#1a1a1a`, renders only immutable `Arc<RenderFrame>` values, and preserves search, selection, cursor, mouse, resize, IME, and clipboard behavior. Chrome must not access engine, storage, or session internals from the draw path.

### ContentDialog overlays

Connection editor, settings, import, and secure interaction surfaces use opaque Fluent ContentDialog-style tonal surfaces inside the existing GTK overlay architecture. They use an 8px boundary, grouped fields, persistent labels, adjacent errors, and a clear primary/secondary action row. They remain real GTK controls rather than custom-drawn form widgets.

## 5. Component state requirements

| Surface | Required states |
|---|---|
| Command bar | default, hover, pressed, keyboard focus, disabled, command rejection |
| Sidebar search/actions | empty, populated, no results, hover, focus, disabled, send error |
| Group and connection rows | default, hover, selected, focused, keyboard activation, metadata, empty group |
| Session tabs | inactive, hover, active, focus, close hover/focus, rejected command |
| Pane surfaces | pending, connecting, host-key wait, auth wait, connected, reconnecting, closing, exited, failed, crashed, unavailable |
| Fields | default, focus, populated, disabled, invalid, inherited, explicit override |
| Import | source selection, preview, disabled wildcard, warning, secret-present marker, pending, result, retry, cancel |
| Secure interaction | unknown key, changed key, password, passphrase, keyboard-interactive, pending, rejected, retry |

Every icon-only action has an accessible name and tooltip. Every state has text or native semantics; color and motion are never the only signal.

## 6. Motion and interaction

- `motion-fast` 80ms: row hover, close/destructive hover, small state acknowledgement.
- `motion-standard` 100ms: command, sidebar, tab, and pane-control background/content changes.
- `motion-focus` 120ms: focus border/outline and field boundary changes.
- Motion honors `gtk-enable-animations`, never delays dispatch, and never conveys required information by itself.
- Keyboard activation remains native. Ctrl+Enter saves the connection editor; Escape closes or cancels the active overlay; Enter submits only when the corresponding reducer permits it.
- Secret input is cleared from GTK and reducer buffers immediately after submit/cancel and is never restored from public state.

## 7. Depth and exclusions

Depth is borders-first and tonal. The following are explicitly absent:

- Mica, acrylic, backdrop blur, or fake translucency;
- gradients or decorative filters;
- drop shadows and floating card stacks;
- emoji used as icons;
- platform-specific title-bar APIs;
- browser or web-UI infrastructure.

Product icons are source-controlled monochrome SVGs compiled into the binary. They use `currentColor`, contain no external references, scripts, fonts, animation, raster payloads, gradients, or filters. Missing SVG-loader support selects the compiled internal-vector backend; malformed assets remain hard errors rather than silently falling back.

## 8. Accessibility and evidence

- Follow WCAG 2.2 AA principles where GTK exposes equivalent native semantics.
- Native controls retain roles, keyboard navigation, disabled state, labels, and focus behavior.
- Focus is visible at 2px. Error, warning, success, selection, and host-trust states include text or an accessible name.
- Existing secrets never enter widgets, screenshots, public snapshots, diagnostics, JSON, JUnit, or logs.
- Changed host keys have no acceptance path. Unknown host keys expose Reject and Accept-and-store only.
- The visual checkpoint must verify realized command bar, sidebar, tabs, pane command row, ContentDialog state, and resolved product icons through component-owned facts.
- Native PNG analysis converts premultiplied Cairo ARGB32 to canonical RGBA before checking dark-shell, accent, danger, focus, and missing-image ranges.
- Task20 smoke remains the authoritative local real-surface flow. Task21 package checks verify the embedded resources through startup; no external icon payload is required.
- Hosted Linux, macOS, and Windows jobs remain external evidence and are not claimed until a GitHub run exists.

No accessibility debt is waived. Windows local smoke supplies the current native evidence; screen-reader announcements, high-contrast behavior, and hosted Linux/macOS rendering remain unverified until their explicit acceptance runs.
