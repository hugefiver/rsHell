use gtk::pango;

pub(crate) fn for_text(base: &pango::FontDescription, text: &str) -> pango::FontDescription {
    let mut font = base.clone();
    if text.chars().any(is_emoji) {
        font.set_family(emoji_family());
    } else if text.chars().any(is_cjk) {
        font.set_family(cjk_family());
    }
    font
}

fn is_cjk(value: char) -> bool {
    matches!(
        value,
        '\u{2e80}'..='\u{2fff}'
            | '\u{3000}'..='\u{303f}'
            | '\u{31c0}'..='\u{31ef}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{f900}'..='\u{faff}'
    )
}

fn is_emoji(value: char) -> bool {
    matches!(value, '\u{2600}'..='\u{27bf}' | '\u{1f000}'..='\u{1faff}')
}

#[cfg(target_os = "windows")]
const fn cjk_family() -> &'static str {
    "Microsoft YaHei UI"
}

#[cfg(target_os = "macos")]
const fn cjk_family() -> &'static str {
    "PingFang SC"
}

#[cfg(all(unix, not(target_os = "macos")))]
const fn cjk_family() -> &'static str {
    "Monospace"
}

#[cfg(target_os = "windows")]
const fn emoji_family() -> &'static str {
    "Segoe UI Emoji"
}

#[cfg(target_os = "macos")]
const fn emoji_family() -> &'static str {
    "Apple Color Emoji"
}

#[cfg(all(unix, not(target_os = "macos")))]
const fn emoji_family() -> &'static str {
    "Monospace"
}

#[cfg(test)]
mod tests {
    use gtk::pango;

    #[test]
    fn windows_terminal_fallbacks_cover_cjk_and_emoji_without_changing_ascii() {
        let mut base = pango::FontDescription::new();
        base.set_family("Cascadia Mono");
        base.set_absolute_size(15.0 * f64::from(pango::SCALE));

        assert_eq!(
            super::for_text(&base, "ASCII").family().as_deref(),
            Some("Cascadia Mono")
        );
        #[cfg(target_os = "windows")]
        {
            let map = pangocairo::FontMap::new();
            let context = pango::Context::new();
            context.set_font_map(Some(&map));
            for (text, family) in [("界", "Microsoft YaHei UI"), ("🙂", "Segoe UI Emoji")] {
                let font = super::for_text(&base, text);
                assert_eq!(font.family().as_deref(), Some(family));
                let layout = pango::Layout::new(&context);
                layout.set_font_description(Some(&font));
                layout.set_text(text);
                assert_eq!(layout.unknown_glyphs_count(), 0, "missing glyph for {text}");
            }
        }
    }
}
