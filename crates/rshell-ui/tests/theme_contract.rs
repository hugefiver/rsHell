use rshell_ui::embedded_theme_css;

const DESIGN: &str = include_str!("../../../DESIGN.md");
const SIDEBAR_WIDGETS: &str = include_str!("../src/connection_sidebar_widgets.rs");
const TERMINAL_WIDGETS: &str = include_str!("../src/terminal_view_widgets.rs");

#[test]
fn design_and_embedded_css_share_the_task21_fluent_contract() {
    for marker in [
        "Task21 Fluent shell authority",
        "surface-shell",
        "surface-terminal",
        "control-radius | 4px",
        "overlay-radius | 8px",
        "focus-width | 2px",
        "motion-fast | 80ms",
        "motion-standard | 100ms",
        "motion-focus | 120ms",
        "1360×860",
        "232px",
    ] {
        assert!(DESIGN.contains(marker), "DESIGN.md is missing {marker}");
    }

    let css = embedded_theme_css();
    for marker in [
        ".fluent-shell",
        ".command-bar",
        ".content-dialog",
        ".tab-bar",
        ".pane-command-row",
        "font-family: \"Segoe UI\", system-ui, sans-serif",
        "font-family: \"Cascadia Mono\", \"JetBrains Mono\", \"Consolas\", monospace",
        "border-radius: 4px",
        "border-radius: 8px",
        "outline: 2px",
        "80ms",
        "100ms",
        "120ms",
    ] {
        assert!(css.contains(marker), "embedded CSS is missing {marker}");
    }
}

#[test]
fn fluent_css_contains_no_prohibited_effect_or_out_of_range_motion() {
    let css = embedded_theme_css();
    for forbidden in [
        "gradient(",
        "backdrop-filter",
        "filter: blur",
        "box-shadow:",
        "150ms",
        "animation:",
    ] {
        assert!(
            !css.contains(forbidden),
            "prohibited CSS fragment: {forbidden}"
        );
    }
}

#[test]
fn native_search_entries_have_explicit_programmatic_names() {
    for (source, label) in [
        (SIDEBAR_WIDGETS, "Search connections"),
        (TERMINAL_WIDGETS, "Search terminal output"),
    ] {
        let property = format!("gtk::accessible::Property::Label(\"{label}\")");
        assert!(
            source.contains(&property),
            "native SearchEntry is missing accessible label {label}"
        );
    }
}

#[test]
fn sidebar_identity_text_has_explicit_high_contrast_foregrounds() {
    let css = embedded_theme_css();
    for (selector, color) in [
        (".connection-name {", "color: #f5f5f5;"),
        (".connection-meta {", "color: #cccccc;"),
        (".folder-header {", "color: #cccccc;"),
    ] {
        let body = css
            .split_once(selector)
            .unwrap_or_else(|| panic!("missing sidebar selector {selector}"))
            .1
            .split_once('}')
            .expect("complete sidebar rule")
            .0;
        assert!(
            body.contains(color),
            "{selector} must use explicit Fluent foreground {color}"
        );
        assert!(
            !body.contains("opacity:"),
            "{selector} must not reduce text contrast with opacity"
        );
    }
}
