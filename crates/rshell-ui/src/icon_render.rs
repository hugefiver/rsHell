use std::fmt;

use gtk::prelude::*;
use relm4::gtk;

use crate::{IconBackend, IconCacheKey, ProductIcon, icon_cache, icon_vector};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconRenderRequest {
    pub logical_size: u16,
    pub effective_scale: f64,
}

impl IconRenderRequest {
    pub fn for_widget(logical_size: u16, widget: &impl IsA<gtk::Widget>) -> Self {
        Self {
            logical_size,
            effective_scale: f64::from(widget.scale_factor()),
        }
    }

    pub fn physical_size(self) -> Result<u16, IconRenderError> {
        if self.logical_size == 0 {
            return Err(IconRenderError::InvalidLogicalSize);
        }
        if !self.effective_scale.is_finite() || self.effective_scale <= 0.0 {
            return Err(IconRenderError::InvalidScale);
        }
        let physical = (f64::from(self.logical_size) * self.effective_scale).ceil();
        if physical > f64::from(u16::MAX) {
            return Err(IconRenderError::PhysicalSizeOverflow);
        }
        Ok(physical as u16)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconRenderError {
    InvalidLogicalSize,
    InvalidScale,
    PhysicalSizeOverflow,
    LoaderUnavailable,
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

#[derive(Debug)]
pub enum SvgDecodeOutcome {
    Texture(gtk::gdk::Texture),
    LoaderUnavailable,
}

#[cfg(test)]
pub(crate) struct RenderedIcon {
    pub(crate) backend: IconBackend,
    pub(crate) texture: gtk::gdk::Texture,
}

pub(crate) fn decode_texture(
    icon: ProductIcon,
    backend: IconBackend,
    request: IconRenderRequest,
) -> Result<gtk::gdk::Texture, IconRenderError> {
    let physical_size = request.physical_size()?;
    let key = IconCacheKey {
        icon,
        backend,
        physical_size,
    };
    if let Some(texture) = icon_cache::get(key) {
        return Ok(texture);
    }
    let texture = match backend {
        IconBackend::GtkSvg => match decode_svg(icon.metadata().svg, physical_size)? {
            SvgDecodeOutcome::Texture(texture) => texture,
            SvgDecodeOutcome::LoaderUnavailable => return Err(IconRenderError::LoaderUnavailable),
        },
        IconBackend::InternalVector => icon_vector::render(icon, physical_size)?,
    };
    icon_cache::insert(key, texture.clone());
    Ok(texture)
}

pub(crate) fn image(
    icon: ProductIcon,
    request: IconRenderRequest,
) -> Result<gtk::Image, IconRenderError> {
    let texture = icon.decode_texture(request)?;
    let image = gtk::Image::from_paintable(Some(&texture));
    image.set_pixel_size(i32::from(request.logical_size));
    image.add_css_class("product-icon");
    bind_image_scale(&image, icon, request.logical_size);
    Ok(image)
}

fn bind_image_scale(image: &gtk::Image, icon: ProductIcon, logical_size: u16) {
    image.connect_notify_local(Some("scale-factor"), move |image, _| {
        let request = IconRenderRequest::for_widget(logical_size, image);
        match icon.decode_texture(request) {
            Ok(texture) => image.set_paintable(Some(&texture)),
            Err(_) => image.set_paintable(gtk::gdk::Paintable::NONE),
        }
    });
}

fn decode_svg(
    bytes: &'static [u8],
    physical_size: u16,
) -> Result<SvgDecodeOutcome, IconRenderError> {
    let loader = match gtk::gdk_pixbuf::PixbufLoader::with_type("svg") {
        Ok(loader) => loader,
        Err(_) => return Ok(SvgDecodeOutcome::LoaderUnavailable),
    };
    let target = i32::from(physical_size);
    loader.set_size(target, target);
    loader
        .write(bytes)
        .map_err(|error| IconRenderError::InvalidSvg(error.to_string()))?;
    loader
        .close()
        .map_err(|error| IconRenderError::InvalidSvg(error.to_string()))?;
    let pixbuf = loader.pixbuf().ok_or(IconRenderError::EmptyFrame)?;
    if (pixbuf.width(), pixbuf.height()) != (target, target) {
        return Err(IconRenderError::WrongSize(pixbuf.width(), pixbuf.height()));
    }
    let texture = gtk::gdk::Texture::for_pixbuf(&pixbuf);
    validate_native_snapshot(&texture, physical_size)?;
    Ok(SvgDecodeOutcome::Texture(texture))
}

#[cfg(test)]
pub(crate) fn render_with<D, V>(
    icon: ProductIcon,
    physical_size: u16,
    decode_svg: D,
    render_vector: V,
) -> Result<RenderedIcon, IconRenderError>
where
    D: FnOnce(&'static [u8], u16) -> Result<SvgDecodeOutcome, IconRenderError>,
    V: FnOnce(ProductIcon, u16) -> Result<gtk::gdk::Texture, IconRenderError>,
{
    match decode_svg(icon.metadata().svg, physical_size)? {
        SvgDecodeOutcome::Texture(texture) => Ok(RenderedIcon {
            backend: IconBackend::GtkSvg,
            texture,
        }),
        SvgDecodeOutcome::LoaderUnavailable => Ok(RenderedIcon {
            backend: IconBackend::InternalVector,
            texture: render_vector(icon, physical_size)?,
        }),
    }
}

fn validate_native_snapshot(
    texture: &gtk::gdk::Texture,
    physical_size: u16,
) -> Result<(), IconRenderError> {
    let snapshot = gtk::Snapshot::new();
    let side = f32::from(physical_size);
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, side, side);
    snapshot.append_texture(texture, &bounds);
    snapshot
        .to_node()
        .ok_or_else(|| IconRenderError::Snapshot("empty".to_owned()))?;
    Ok(())
}
