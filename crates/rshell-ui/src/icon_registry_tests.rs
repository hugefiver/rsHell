use super::*;
use crate::{icon_render::render_with, icon_vector};

#[test]
fn selector_uses_svg_without_vector_when_available() {
    let texture = icon_vector::render(ProductIcon::Import, 16).unwrap();
    let rendered = render_with(
        ProductIcon::Import,
        16,
        |_, _| Ok(SvgDecodeOutcome::Texture(texture)),
        |_, _| panic!("vector called"),
    )
    .unwrap();
    assert_eq!(rendered.backend, IconBackend::GtkSvg);
    assert_eq!(
        (rendered.texture.width(), rendered.texture.height()),
        (16, 16)
    );
}

#[test]
fn both_backend_paths_preserve_requested_physical_dimensions() {
    for physical_size in [16, 20, 24, 32] {
        let svg_texture = icon_vector::render(ProductIcon::Import, physical_size).unwrap();
        let svg = render_with(
            ProductIcon::Import,
            physical_size,
            |_, _| Ok(SvgDecodeOutcome::Texture(svg_texture)),
            |_, _| panic!("vector called for available SVG loader"),
        )
        .unwrap();
        assert_eq!(svg.backend, IconBackend::GtkSvg);
        assert_eq!(
            (svg.texture.width(), svg.texture.height()),
            (i32::from(physical_size), i32::from(physical_size))
        );

        let vector = render_with(
            ProductIcon::Import,
            physical_size,
            |_, _| Ok(SvgDecodeOutcome::LoaderUnavailable),
            icon_vector::render,
        )
        .unwrap();
        assert_eq!(vector.backend, IconBackend::InternalVector);
        assert_eq!(
            (vector.texture.width(), vector.texture.height()),
            (i32::from(physical_size), i32::from(physical_size))
        );
    }
}

#[test]
fn selector_uses_always_compiled_vector_only_when_loader_is_unavailable() {
    for icon in ProductIcon::ALL {
        let rendered = render_with(
            icon,
            16,
            |_, _| Ok(SvgDecodeOutcome::LoaderUnavailable),
            icon_vector::render,
        )
        .unwrap();
        assert_eq!(rendered.backend, IconBackend::InternalVector);
        assert_eq!(
            (rendered.texture.width(), rendered.texture.height()),
            (16, 16)
        );
    }
}

#[test]
fn malformed_or_invalid_svg_never_falls_back() {
    for error in [
        IconRenderError::InvalidSvg("write".into()),
        IconRenderError::InvalidSvg("close".into()),
        IconRenderError::EmptyFrame,
        IconRenderError::WrongSize(15, 16),
        IconRenderError::Snapshot("snapshot".into()),
    ] {
        let vector_called = std::cell::Cell::new(false);
        let result = render_with(
            ProductIcon::Import,
            16,
            |_, _| Err(error),
            |_, size| {
                vector_called.set(true);
                icon_vector::render(ProductIcon::Import, size)
            },
        );
        assert!(result.is_err());
        assert!(!vector_called.get());
    }
}
