#![cfg(not(target_os = "macos"))]

use gtk::prelude::*;
use relm4::gtk;
use rshell_ui::{IconBackend, IconCacheKey, IconRenderRequest, ProductIcon, embedded_icons_ready};

const LOGICAL_ICON_SIZE: u16 = 16;

fn request(effective_scale: f64) -> IconRenderRequest {
    IconRenderRequest {
        logical_size: LOGICAL_ICON_SIZE,
        effective_scale,
    }
}

#[test]
fn physical_icon_cache_keys_scale_and_backend() {
    gtk::init().expect("Task 5 native icon test requires GTK");
    for (scale, physical_size) in [(1.0, 16), (1.25, 20), (1.5, 24), (2.0, 32)] {
        let request = request(scale);
        assert_eq!(request.physical_size().unwrap(), physical_size);
        let backends = match ProductIcon::backend() {
            IconBackend::GtkSvg => vec![IconBackend::GtkSvg, IconBackend::InternalVector],
            IconBackend::InternalVector => vec![IconBackend::InternalVector],
        };
        for backend in backends {
            let key = IconCacheKey {
                icon: ProductIcon::Settings,
                backend,
                physical_size,
            };
            let texture = ProductIcon::Settings
                .decode_texture_for_backend(backend, request)
                .expect("both embedded backends render at the requested physical size");
            assert_eq!(
                (texture.width(), texture.height()),
                (physical_size.into(), physical_size.into())
            );
            let cached = ProductIcon::Settings
                .decode_texture_for_backend(backend, request)
                .expect("same cache key remains renderable");
            assert_eq!(texture, cached, "same {key:?} must reuse one texture");

            let image = ProductIcon::Settings.image(request).unwrap();
            assert_eq!(image.pixel_size(), i32::from(LOGICAL_ICON_SIZE));
        }
    }

    assert_ne!(
        IconCacheKey {
            icon: ProductIcon::Settings,
            backend: IconBackend::GtkSvg,
            physical_size: 16,
        },
        IconCacheKey {
            icon: ProductIcon::Settings,
            backend: IconBackend::InternalVector,
            physical_size: 16,
        }
    );
    assert_ne!(
        IconCacheKey {
            icon: ProductIcon::Settings,
            backend: IconBackend::GtkSvg,
            physical_size: 16,
        },
        IconCacheKey {
            icon: ProductIcon::Settings,
            backend: IconBackend::GtkSvg,
            physical_size: 32,
        }
    );

    for invalid in [
        IconRenderRequest {
            logical_size: 0,
            effective_scale: 1.0,
        },
        request(0.0),
        request(f64::NAN),
        request(f64::INFINITY),
        IconRenderRequest {
            logical_size: u16::MAX,
            effective_scale: 2.0,
        },
    ] {
        assert!(
            invalid.physical_size().is_err(),
            "invalid request {invalid:?}"
        );
    }

    for icon in ProductIcon::ALL {
        let texture = icon
            .decode_texture(request(1.0))
            .expect("embedded icon must decode");
        assert_eq!((texture.width(), texture.height()), (16, 16));
        let snapshot = gtk::Snapshot::new();
        let bounds = gtk::graphene::Rect::new(0.0, 0.0, 16.0, 16.0);
        snapshot.append_texture(&texture, &bounds);
        let node = snapshot.to_node().expect("icon render node");
        let renderer = gtk::gsk::CairoRenderer::new();
        renderer.realize(None).expect("Cairo renderer");
        let rendered = renderer.render_texture(&node, Some(&bounds));
        renderer.unrealize();
        assert!(!rendered.save_to_png_bytes().is_empty());
    }
    assert!(embedded_icons_ready(request(1.0)));
    let button = ProductIcon::Delete
        .button(Some("Delete selected connection"), request(1.0))
        .expect("embedded delete icon");
    assert_eq!(
        button.tooltip_text().as_deref(),
        Some("Delete selected connection")
    );
    assert_eq!(button.accessible_role(), gtk::AccessibleRole::Button);
    let image = button
        .first_child()
        .and_then(|child| child.downcast::<gtk::Image>().ok())
        .expect("button must contain an embedded image");
    assert!(image.icon_name().is_none());
    assert!(image.has_css_class("product-icon"));
}

#[test]
fn registry_is_closed_complete_and_binary_embedded() {
    assert_eq!(ProductIcon::ALL.len(), 18);
    assert!(ProductIcon::ALL.contains(&ProductIcon::More));
    assert!(ProductIcon::ALL.contains(&ProductIcon::Navigation));
    let labels = ProductIcon::ALL
        .into_iter()
        .map(|icon| icon.metadata().accessible_label)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(labels.len(), ProductIcon::ALL.len());

    for icon in ProductIcon::ALL {
        let metadata = icon.metadata();
        let svg = std::str::from_utf8(metadata.svg).expect("embedded SVG must be UTF-8");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("width=\"16\""));
        assert!(svg.contains("height=\"16\""));
        assert!(svg.contains("viewBox=\"0 0 16 16\""));
        assert!(svg.contains("currentColor"));
        for forbidden in [
            "<script",
            "<image",
            "<foreignObject",
            "href=",
            "url(",
            "<animate",
            "<font",
            "gradient",
            "filter=",
            "data:",
        ] {
            assert!(!svg.contains(forbidden), "unsafe SVG fragment {forbidden}");
        }
    }
}
