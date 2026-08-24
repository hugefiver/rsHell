use super::*;

#[test]
fn selector_uses_svg_without_vector_when_available() {
    let texture = icon_vector::render(ProductIcon::Import).unwrap();
    let rendered = render_with(
        ProductIcon::Import,
        |_| Ok(SvgDecodeOutcome::Texture(texture)),
        |_| panic!("vector called"),
    )
    .unwrap();
    assert_eq!(rendered.backend, IconBackend::GtkSvg);
    assert_eq!(
        (rendered.texture.width(), rendered.texture.height()),
        (16, 16)
    );
}

#[test]
fn selector_uses_always_compiled_vector_only_when_loader_is_unavailable() {
    for icon in ProductIcon::ALL {
        let rendered = render_with(
            icon,
            |_| Ok(SvgDecodeOutcome::LoaderUnavailable),
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
            |_| Err(error),
            |_| {
                vector_called.set(true);
                icon_vector::render(ProductIcon::Import)
            },
        );
        assert!(result.is_err());
        assert!(!vector_called.get());
    }
}
