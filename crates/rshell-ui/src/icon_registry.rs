use std::fmt;

use gtk::prelude::*;
use relm4::gtk;

use crate::{icon_cache, icon_vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProductIcon {
    Import,
    Settings,
    AddConnection,
    AddGroup,
    Edit,
    Duplicate,
    Delete,
    CloseTab,
    NewTab,
    SplitHorizontal,
    SplitVertical,
    Retry,
    CopyDiagnostics,
    Warning,
    SecretPresent,
    HostTrust,
}

impl ProductIcon {
    pub const ALL: [Self; 16] = [
        Self::Import,
        Self::Settings,
        Self::AddConnection,
        Self::AddGroup,
        Self::Edit,
        Self::Duplicate,
        Self::Delete,
        Self::CloseTab,
        Self::NewTab,
        Self::SplitHorizontal,
        Self::SplitVertical,
        Self::Retry,
        Self::CopyDiagnostics,
        Self::Warning,
        Self::SecretPresent,
        Self::HostTrust,
    ];

    pub fn metadata(self) -> IconMetadata {
        let (accessible_label, tooltip, svg) = match self {
            Self::Import => (
                "Import",
                "Import connections",
                include_bytes!("../../../resources/icons/import.svg").as_slice(),
            ),
            Self::Settings => (
                "Settings",
                "Terminal settings",
                include_bytes!("../../../resources/icons/settings.svg").as_slice(),
            ),
            Self::AddConnection => (
                "Add connection",
                "Add connection",
                include_bytes!("../../../resources/icons/add-connection.svg").as_slice(),
            ),
            Self::AddGroup => (
                "Add group",
                "Add group",
                include_bytes!("../../../resources/icons/add-group.svg").as_slice(),
            ),
            Self::Edit => (
                "Edit",
                "Edit selected connection",
                include_bytes!("../../../resources/icons/edit.svg").as_slice(),
            ),
            Self::Duplicate => (
                "Duplicate",
                "Duplicate selected connection",
                include_bytes!("../../../resources/icons/duplicate.svg").as_slice(),
            ),
            Self::Delete => (
                "Delete",
                "Delete selected connection",
                include_bytes!("../../../resources/icons/delete.svg").as_slice(),
            ),
            Self::CloseTab => (
                "Close",
                "Close tab or pane",
                include_bytes!("../../../resources/icons/close-tab.svg").as_slice(),
            ),
            Self::NewTab => (
                "New tab",
                "New local terminal tab",
                include_bytes!("../../../resources/icons/new-tab.svg").as_slice(),
            ),
            Self::SplitHorizontal => (
                "Split horizontally",
                "Split pane horizontally",
                include_bytes!("../../../resources/icons/split-horizontal.svg").as_slice(),
            ),
            Self::SplitVertical => (
                "Split vertically",
                "Split pane vertically",
                include_bytes!("../../../resources/icons/split-vertical.svg").as_slice(),
            ),
            Self::Retry => (
                "Reconnect",
                "Reconnect session",
                include_bytes!("../../../resources/icons/retry.svg").as_slice(),
            ),
            Self::CopyDiagnostics => (
                "Copy diagnostics",
                "Copy diagnostics",
                include_bytes!("../../../resources/icons/copy-diagnostics.svg").as_slice(),
            ),
            Self::Warning => (
                "Warning",
                "Warning",
                include_bytes!("../../../resources/icons/warning.svg").as_slice(),
            ),
            Self::SecretPresent => (
                "Secret present",
                "Secret present",
                include_bytes!("../../../resources/icons/secret-present.svg").as_slice(),
            ),
            Self::HostTrust => (
                "Host trust",
                "Host trust",
                include_bytes!("../../../resources/icons/host-trust.svg").as_slice(),
            ),
        };
        IconMetadata {
            accessible_label,
            tooltip,
            svg,
        }
    }

    pub fn backend() -> IconBackend {
        if gtk::gdk_pixbuf::PixbufLoader::with_type("svg").is_ok() {
            IconBackend::GtkSvg
        } else {
            IconBackend::InternalVector
        }
    }

    pub fn decode_texture(self) -> Result<gtk::gdk::Texture, IconRenderError> {
        if let Some(texture) = icon_cache::get(self) {
            return Ok(texture);
        }
        let rendered = render_with(self, decode_svg, icon_vector::render)?;
        debug_assert_eq!(rendered.backend, Self::backend());
        icon_cache::insert(self, rendered.texture.clone());
        Ok(rendered.texture)
    }

    pub fn image(self) -> Result<gtk::Image, IconRenderError> {
        let texture = self.decode_texture()?;
        let image = gtk::Image::from_paintable(Some(&texture));
        image.set_pixel_size(16);
        image.add_css_class("product-icon");
        Ok(image)
    }

    pub fn button(self, tooltip: Option<&str>) -> Result<gtk::Button, IconRenderError> {
        let metadata = self.metadata();
        let label = tooltip.unwrap_or(metadata.tooltip);
        let button = gtk::Button::builder().tooltip_text(label).build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.set_child(Some(&self.image()?));
        Ok(button)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IconMetadata {
    pub accessible_label: &'static str,
    pub tooltip: &'static str,
    pub svg: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconBackend {
    GtkSvg,
    InternalVector,
}

#[derive(Debug)]
pub enum SvgDecodeOutcome {
    Texture(gtk::gdk::Texture),
    LoaderUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconRenderError {
    InvalidSvg(String),
    EmptyFrame,
    WrongSize(i32, i32),
    Snapshot(String),
    InternalVector,
}

impl fmt::Display for IconRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("embedded product icon could not be rendered")
    }
}

impl std::error::Error for IconRenderError {}

struct RenderedIcon {
    backend: IconBackend,
    texture: gtk::gdk::Texture,
}

fn decode_svg(bytes: &'static [u8]) -> Result<SvgDecodeOutcome, IconRenderError> {
    let loader = match gtk::gdk_pixbuf::PixbufLoader::with_type("svg") {
        Ok(loader) => loader,
        Err(_) => return Ok(SvgDecodeOutcome::LoaderUnavailable),
    };
    loader
        .write(bytes)
        .map_err(|error| IconRenderError::InvalidSvg(error.to_string()))?;
    loader
        .close()
        .map_err(|error| IconRenderError::InvalidSvg(error.to_string()))?;
    let pixbuf = loader.pixbuf().ok_or(IconRenderError::EmptyFrame)?;
    if (pixbuf.width(), pixbuf.height()) != (16, 16) {
        return Err(IconRenderError::WrongSize(pixbuf.width(), pixbuf.height()));
    }
    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
    validate_native_snapshot(&texture)?;
    Ok(SvgDecodeOutcome::Texture(texture))
}

fn render_with<D, V>(
    icon: ProductIcon,
    decode_svg: D,
    render_vector: V,
) -> Result<RenderedIcon, IconRenderError>
where
    D: FnOnce(&'static [u8]) -> Result<SvgDecodeOutcome, IconRenderError>,
    V: FnOnce(ProductIcon) -> Result<gtk::gdk::Texture, IconRenderError>,
{
    match decode_svg(icon.metadata().svg)? {
        SvgDecodeOutcome::Texture(texture) => Ok(RenderedIcon {
            backend: IconBackend::GtkSvg,
            texture,
        }),
        SvgDecodeOutcome::LoaderUnavailable => Ok(RenderedIcon {
            backend: IconBackend::InternalVector,
            texture: render_vector(icon)?,
        }),
    }
}

fn validate_native_snapshot(texture: &gtk::gdk::Texture) -> Result<(), IconRenderError> {
    let snapshot = gtk::Snapshot::new();
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, 16.0, 16.0);
    snapshot.append_texture(texture, &bounds);
    snapshot
        .to_node()
        .ok_or_else(|| IconRenderError::Snapshot("empty".to_owned()))?;
    Ok(())
}

#[cfg(test)]
#[path = "icon_registry_tests.rs"]
mod tests;
