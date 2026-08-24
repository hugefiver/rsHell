use gtk::prelude::*;
use relm4::gtk;
use rshell_ui::{IconBackend, ProductIcon, embedded_icons_ready};

#[test]
fn registry_is_closed_complete_and_binary_embedded() {
    assert_eq!(ProductIcon::ALL.len(), 16);
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

#[test]
fn every_icon_decodes_and_snapshots_on_native_gtk() {
    gtk::init().expect("Task21 native icon test requires GTK");
    for icon in ProductIcon::ALL {
        let texture = icon.decode_texture().expect("embedded SVG must decode");
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
    assert!(embedded_icons_ready());
    let button = ProductIcon::Delete
        .button(Some("Delete selected connection"))
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
    assert!(matches!(
        ProductIcon::backend(),
        IconBackend::GtkSvg | IconBackend::InternalVector
    ));
}
