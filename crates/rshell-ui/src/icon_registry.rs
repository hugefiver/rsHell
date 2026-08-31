use gtk::prelude::*;
use relm4::gtk;

use crate::{icon_backend, icon_render};

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
    More,
    Navigation,
}

impl ProductIcon {
    pub const ALL: [Self; 18] = [
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
        Self::More,
        Self::Navigation,
    ];

    pub fn backend() -> IconBackend {
        icon_backend::detect()
    }

    pub fn decode_texture(
        self,
        request: IconRenderRequest,
    ) -> Result<gtk::gdk::Texture, IconRenderError> {
        self.decode_texture_for_backend(Self::backend(), request)
    }

    pub fn decode_texture_for_backend(
        self,
        backend: IconBackend,
        request: IconRenderRequest,
    ) -> Result<gtk::gdk::Texture, IconRenderError> {
        icon_render::decode_texture(self, backend, request)
    }

    pub fn image(self, request: IconRenderRequest) -> Result<gtk::Image, IconRenderError> {
        icon_render::image(self, request)
    }

    pub fn button(
        self,
        tooltip: Option<&str>,
        request: IconRenderRequest,
    ) -> Result<gtk::Button, IconRenderError> {
        let metadata = self.metadata();
        let label = tooltip.unwrap_or(metadata.tooltip);
        let button = gtk::Button::builder().tooltip_text(label).build();
        button.update_property(&[gtk::accessible::Property::Label(label)]);
        button.set_child(Some(&self.image(request)?));
        Ok(button)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IconBackend {
    GtkSvg,
    InternalVector,
}

pub use crate::icon_render::{IconRenderError, IconRenderRequest, SvgDecodeOutcome};

#[cfg(test)]
#[path = "icon_registry_tests.rs"]
mod tests;
