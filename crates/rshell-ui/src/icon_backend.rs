use relm4::gtk;

use crate::IconBackend;

pub(crate) fn detect() -> IconBackend {
    let svg_available = gtk::gdk_pixbuf::Pixbuf::formats()
        .iter()
        .any(|format| !format.is_disabled() && format.name().is_some_and(|name| name == "svg"));
    from_svg_available(svg_available)
}

const fn from_svg_available(svg_available: bool) -> IconBackend {
    if svg_available {
        IconBackend::GtkSvg
    } else {
        IconBackend::InternalVector
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_registered_svg_availability_without_opening_a_loader() {
        assert_eq!(from_svg_available(true), IconBackend::GtkSvg);
        assert_eq!(from_svg_available(false), IconBackend::InternalVector);
    }
}
