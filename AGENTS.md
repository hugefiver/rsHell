# AGENTS.md — rsHell Application

Cross-platform SSH terminal manager built with Rust 2024, GTK4 and Relm4.

## Repository boundaries

- `src/`: application composition, startup, cleanup and P0 smoke entry points.
- `crates/rshell-ui/`: GTK4/Relm4 widgets, terminal rendering and UI tests.
- `resources/`: compiled-in CSS and product SVG icons; `DESIGN.md` defines the UI design.
- `tests/`: application integration and native GUI tests.
- `scripts/qa/`: application smoke, package, workflow and secret checks.
- `corelibs/`: Git submodule for `https://github.com/MoyuTeams/rsHell-corelibs`, an independent Cargo workspace containing `rshell-core`, `rshell-platform`, `rshell-session`, `rshell-storage` and the vendored PTY fork.

The application workspace contains the root `rshell` package and `rshell-ui`. Cargo path dependencies reference `corelibs/crates/`. Reusable domain, terminal, SSH, persistence and platform changes belong in the core repository; GTK and application orchestration stay here.

## Submodule maintenance

Clone with `--recurse-submodules`. After pulling or switching branches, run:

```text
git submodule update --init --recursive
```

The parent gitlink pins the core revision. Do not use `submodule update --remote` in builds. For an authorized core change, work on a branch inside `corelibs`, commit and push that repository first, then commit the updated gitlink in this repository. Commit and push permissions remain operation-specific.

Both workspaces have their own lockfile and target directory. Root Cargo patches do not propagate from a dependency workspace: preserve the required `crypto-bigint` patch in both manifests. Preserve the core repository's pinned Alacritty, russh and PTY sources.

## Build and verification

GTK4 and `pkg-config` must be available. On Windows, use the existing gvsbuild installation and its `bin`, `lib`, and `lib/pkgconfig` paths for `PATH`, `LIB`, and `PKG_CONFIG_PATH`. Never install tools without approval.

Application gates:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Core gates use `--manifest-path corelibs/Cargo.toml` with the same commands. Application workspace tests do not run dependency crates' tests. The terminal-engine gate is `corelibs/scripts/qa/terminal-engine-gate.ps1`.

`scripts/qa/p0-smoke.ps1 -Mode All` exercises real GTK, PTY, SSH and credential-store surfaces. Core helpers and core tests run in the core workspace; application binaries and UI tests run in the application workspace. Preserve cleanup, secret redaction, exact-test discovery and nonzero exit propagation.

CI and release check out submodules recursively. Release packages contain the application and required GTK runtime/resources, not QA helper binaries. Master pushes trigger the existing nightly release workflow.

## Conventions and safety

- Use default rustfmt and clippy; CI treats clippy warnings as errors.
- Preserve public core APIs and SQLite migrations when changing repository layout.
- Credentials belong in the OS vault; never write secrets to SQLite, logs or reports.
- Native SSH host-key changes fail closed; unknown keys require confirmation.
- `RSHELL_SHELL` overrides the local shell executable.
- Native GUI tests require an actual display. A skipped or unavailable surface is not a passing runtime check.
- Preserve unrelated local changes and do not stage them with task changes.
