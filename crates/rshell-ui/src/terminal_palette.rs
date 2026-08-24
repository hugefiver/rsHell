use rshell_core::{Color, ColorScheme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rgb(pub f64, pub f64, pub f64);

impl Rgb {
    pub(crate) const fn from_u8(red: u8, green: u8, blue: u8) -> Self {
        Self(
            red as f64 / 255.0,
            green as f64 / 255.0,
            blue as f64 / 255.0,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TerminalPalette {
    pub(crate) foreground: Rgb,
    pub(crate) background: Rgb,
    pub(crate) selection: Rgb,
    pub(crate) search: Rgb,
    pub(crate) current_search: Rgb,
    pub(crate) cursor: Rgb,
    ansi: [Rgb; 16],
}

impl TerminalPalette {
    pub(crate) fn for_scheme(scheme: ColorScheme) -> Self {
        let (foreground, background) = scheme_defaults(scheme);
        Self {
            foreground,
            background,
            // DESIGN.md fixed-dark-zone action/selection roles.
            selection: Rgb::from_u8(0x00, 0x78, 0xd4),
            search: Rgb::from_u8(0xc1, 0x9c, 0x00),
            current_search: Rgb::from_u8(0xff, 0xd7, 0x00),
            cursor: Rgb::from_u8(0xd0, 0xdf, 0xf0),
            ansi: ANSI,
        }
    }

    pub(crate) fn resolve(&self, color: Color, default: Rgb) -> Rgb {
        match color {
            Color::Default => default,
            Color::Ansi(index @ 0..=15) => self.ansi[usize::from(index)],
            Color::Ansi(index @ 16..=231) => color_cube(index),
            Color::Ansi(index) => {
                let value = 8u8.saturating_add(index.saturating_sub(232).saturating_mul(10));
                Rgb::from_u8(value, value, value)
            }
            Color::Rgb(red, green, blue) => Rgb::from_u8(red, green, blue),
        }
    }
}

const ANSI: [Rgb; 16] = [
    Rgb::from_u8(0x00, 0x00, 0x00),
    Rgb::from_u8(0xcd, 0x31, 0x31),
    Rgb::from_u8(0x0d, 0xbc, 0x79),
    Rgb::from_u8(0xe5, 0xe5, 0x10),
    Rgb::from_u8(0x24, 0x72, 0xc8),
    Rgb::from_u8(0xbc, 0x3f, 0xbc),
    Rgb::from_u8(0x11, 0xa8, 0xcd),
    Rgb::from_u8(0xe5, 0xe5, 0xe5),
    Rgb::from_u8(0x66, 0x66, 0x66),
    Rgb::from_u8(0xf1, 0x4c, 0x4c),
    Rgb::from_u8(0x23, 0xd1, 0x8b),
    Rgb::from_u8(0xf5, 0xf5, 0x43),
    Rgb::from_u8(0x3b, 0x8e, 0xea),
    Rgb::from_u8(0xd6, 0x70, 0xd6),
    Rgb::from_u8(0x29, 0xb8, 0xdb),
    Rgb::from_u8(0xff, 0xff, 0xff),
];

fn scheme_defaults(scheme: ColorScheme) -> (Rgb, Rgb) {
    match scheme {
        ColorScheme::Default | ColorScheme::CampbellPowershell => (
            Rgb::from_u8(0xcc, 0xcc, 0xcc),
            Rgb::from_u8(0x1a, 0x1a, 0x1a),
        ),
        ColorScheme::OneDark => (
            Rgb::from_u8(0xab, 0xb2, 0xbf),
            Rgb::from_u8(0x28, 0x2c, 0x34),
        ),
        ColorScheme::SolarizedDark | ColorScheme::SolarizedLight => (
            Rgb::from_u8(0x83, 0x94, 0x96),
            Rgb::from_u8(0x00, 0x2b, 0x36),
        ),
        ColorScheme::Dracula => (
            Rgb::from_u8(0xf8, 0xf8, 0xf2),
            Rgb::from_u8(0x28, 0x2a, 0x36),
        ),
        ColorScheme::Monokai => (
            Rgb::from_u8(0xf8, 0xf8, 0xf2),
            Rgb::from_u8(0x27, 0x28, 0x22),
        ),
        ColorScheme::Nord => (
            Rgb::from_u8(0xd8, 0xde, 0xe9),
            Rgb::from_u8(0x2e, 0x34, 0x40),
        ),
        ColorScheme::GruvboxDark => (
            Rgb::from_u8(0xeb, 0xdb, 0xb2),
            Rgb::from_u8(0x28, 0x28, 0x28),
        ),
        ColorScheme::TokyoNight => (
            Rgb::from_u8(0xc0, 0xca, 0xf5),
            Rgb::from_u8(0x1a, 0x1b, 0x26),
        ),
    }
}

fn color_cube(index: u8) -> Rgb {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let value = index - 16;
    Rgb::from_u8(
        LEVELS[usize::from(value / 36)],
        LEVELS[usize::from((value / 6) % 6)],
        LEVELS[usize::from(value % 6)],
    )
}
