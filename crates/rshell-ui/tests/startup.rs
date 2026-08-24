#![cfg(not(target_os = "macos"))]

use std::sync::Arc;

use rshell_core::{
    CellAttributes, Color, RenderCell, RenderFrame, RenderRow, SessionState, TerminalSize,
};
use rshell_ui::{StartupProbe, embedded_theme_css};

#[test]
fn smoke_report_requires_realized_window_local_session_frame_and_clean_shutdown() {
    if gtk::init().is_err() {
        return;
    }
    let probe = StartupProbe::new();

    probe.observe_window_realized();
    probe.observe_local_session_state(SessionState::Connected);
    probe.observe_render_frame(&frame());

    let report = probe.report(true);
    assert!(report.window_realized);
    assert!(report.local_session_connected);
    assert!(report.non_empty_render_frame);
    assert!(report.shutdown_clean);
    assert!(report.embedded_css_loaded);
    assert!(report.embedded_icons_renderable);
    assert!(matches!(
        report.embedded_icon_backend,
        "gtk_svg" | "internal_vector"
    ));
    assert!(report.is_complete());
}

fn frame() -> RenderFrame {
    RenderFrame {
        generation: 1,
        size: TerminalSize {
            cols: 1,
            rows: 1,
            pixel_width: 9,
            pixel_height: 18,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from([RenderCell {
                text: "x".into(),
                width: 1,
                foreground: Color::Default,
                background: Color::Default,
                attributes: CellAttributes::default(),
                selected: false,
            }]),
        }]),
        cursor: None,
        title: "probe".into(),
        alternate_screen: false,
        mouse_reporting: false,
    }
}

#[test]
fn embedded_theme_contains_terminal_and_sidebar_selectors() {
    let css = embedded_theme_css();
    assert!(!css.trim().is_empty());
    assert!(css.contains(".terminal-view"));
    assert!(css.contains(".sidebar"));
}
