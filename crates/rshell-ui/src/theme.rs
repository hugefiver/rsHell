const APPLICATION_CSS: &str = include_str!("../../../resources/style.css");

/// Applies the stylesheet bundled with the application binary.
pub fn apply_global_css() {
    relm4::set_global_css(APPLICATION_CSS);
}

/// Returns the bundled stylesheet for focused startup checks.
pub fn embedded_theme_css() -> &'static str {
    APPLICATION_CSS
}
