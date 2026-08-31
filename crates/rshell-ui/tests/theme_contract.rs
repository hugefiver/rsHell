use rshell_ui::embedded_theme_css;

const DESIGN: &str = include_str!("../../../DESIGN.md");
const SIDEBAR_WIDGETS: &str = include_str!("../src/connection_sidebar_widgets.rs");
const TERMINAL_WIDGETS: &str = include_str!("../src/terminal_view_widgets.rs");

fn rule_body<'a>(css: &'a str, selector: &str) -> &'a str {
    css.split_once(selector)
        .unwrap_or_else(|| panic!("missing CSS selector {selector}"))
        .1
        .split_once('}')
        .unwrap_or_else(|| panic!("incomplete CSS selector {selector}"))
        .0
}

fn contrast_ratio(foreground: &str, background: &str) -> f64 {
    fn luminance(color: &str) -> f64 {
        assert_eq!(color.len(), 7, "expected #rrggbb color");
        let channel = |start| {
            let value = u8::from_str_radix(&color[start..start + 2], 16).expect("hex channel");
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
    }

    let lighter = luminance(foreground).max(luminance(background));
    let darker = luminance(foreground).min(luminance(background));
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn design_and_embedded_css_share_the_fluent_contract() {
    for marker in [
        "Terminal recovery authority",
        "surface-shell",
        "surface-terminal",
        "type-root | 15 logical px",
        "type-secondary | 14 logical px",
        "type-dialog-title | 18 logical px",
        "spacing-unit | 4 logical px",
        "control-radius | 4px",
        "overlay-radius | 8px",
        "focus-width | 2px",
        "motion-fast | 80ms",
        "motion-standard | 100ms",
        "motion-focus | 120ms",
        "navigation-compact | 48 logical px",
        "navigation-standard | 260 logical px",
        "navigation-wide-max | 280 logical px",
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
        "font-family: \"Segoe UI Variable Text\", \"Segoe UI Variable\", \"Segoe UI\", system-ui, sans-serif",
        "font-family: \"Cascadia Mono\", \"Microsoft YaHei UI\", \"Segoe UI Emoji\", \"Consolas\", monospace",
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
        "opacity: 0.4",
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
fn design_records_recovery_hidpi_and_adaptive_authority() {
    for marker in [
        "Terminal recovery authority",
        "terminal-line-spacing | 2 logical px",
        "Compact | `< 900`",
        "Standard | `900–1439`",
        "Wide | `>= 1440`",
        "max width of `min(680px, window width - 48px)`",
        "Display mode not restored",
    ] {
        assert!(DESIGN.contains(marker), "missing design marker: {marker}");
    }

    let css = embedded_theme_css();
    for marker in [
        ".modal-scrim",
        ".display-recovery-notice",
        ".compact-nav-rail",
        ".tab-overflow",
        ".pane-action-overflow",
        "font-size: 15px",
    ] {
        assert!(css.contains(marker), "missing CSS contract: {marker}");
    }
}

#[test]
fn readable_typography_and_dialog_rhythm_are_token_bound() {
    let css = embedded_theme_css();
    for marker in [
        "font-size: 15px",
        "font-size: 14px",
        ".dialog-header",
        "font-size: 18px",
        ".dialog-section",
        "padding: 20px",
    ] {
        assert!(
            css.contains(marker),
            "missing readable typography marker {marker}"
        );
    }
    for selector in [
        ".connection-meta {",
        ".folder-header {",
        ".pane-state-label {",
    ] {
        assert!(
            rule_body(css, selector).contains("font-size: 14px"),
            "{selector} must meet the secondary-text floor"
        );
    }
}

#[test]
fn recovery_and_adaptive_hooks_cover_their_applicable_states() {
    let css = embedded_theme_css();
    for selector in [
        ".modal-scrim.modal-open",
        ".display-recovery-notice.pending",
        ".display-recovery-notice.success",
        ".display-recovery-notice.error",
        ".display-recovery-notice button:hover",
        ".display-recovery-notice button:focus",
        ".display-recovery-notice button:active",
        ".display-recovery-notice button:disabled",
        ".compact-nav-rail button:hover",
        ".compact-nav-rail button:focus",
        ".compact-nav-rail button:active",
        ".compact-nav-rail button:disabled",
        ".tab-overflow button:hover",
        ".tab-overflow button:focus",
        ".tab-overflow button:active",
        ".tab-overflow button:disabled",
        ".pane-action-overflow button:hover",
        ".pane-action-overflow button:focus",
        ".pane-action-overflow button:active",
        ".pane-action-overflow button:disabled",
    ] {
        assert!(css.contains(selector), "missing CSS state hook {selector}");
    }
}

#[test]
fn css_motion_uses_only_the_approved_durations() {
    let css = embedded_theme_css();
    let mut durations = Vec::new();
    for (end, _) in css.match_indices("ms") {
        let digits = css[..end]
            .chars()
            .rev()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        if !digits.is_empty() {
            durations.push(digits.parse::<u16>().expect("CSS duration before ms"));
        }
    }
    assert!(!durations.is_empty(), "theme must retain state motion");
    assert!(
        durations
            .iter()
            .all(|duration| matches!(duration, 80 | 100 | 120)),
        "unsupported CSS motion durations: {durations:?}"
    );
}

#[test]
fn operational_text_and_focus_tokens_meet_contrast_thresholds() {
    let css = embedded_theme_css();
    for color in ["#202020", "#f5f5f5", "#60cdff"] {
        assert!(css.contains(color), "embedded CSS is missing token {color}");
    }
    assert!(contrast_ratio("#f5f5f5", "#202020") >= 4.5);
    assert!(contrast_ratio("#60cdff", "#202020") >= 3.0);
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
        let body = rule_body(css, selector);
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

#[test]
fn operational_labels_do_not_reduce_contrast_with_opacity() {
    let css = embedded_theme_css();
    for selector in [
        ".sidebar-header {",
        ".editor-group label {",
        ".dim-label {",
        ".editor-dialog grid > label {",
        ".settings-window grid > label {",
    ] {
        let body = rule_body(css, selector);
        assert!(
            body.contains("color:"),
            "{selector} needs an explicit color"
        );
        assert!(
            !body.contains("opacity:"),
            "{selector} must not reduce operational text contrast"
        );
    }
}
