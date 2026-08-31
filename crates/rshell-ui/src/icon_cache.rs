use std::{cell::RefCell, collections::BTreeMap};

use relm4::gtk;

use crate::{IconBackend, IconRenderRequest, ProductIcon};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IconCacheKey {
    pub icon: ProductIcon,
    pub backend: IconBackend,
    pub physical_size: u16,
}

thread_local! {
    static TEXTURES: RefCell<BTreeMap<IconCacheKey, gtk::gdk::Texture>> =
        const { RefCell::new(BTreeMap::new()) };
}

pub(crate) fn get(key: IconCacheKey) -> Option<gtk::gdk::Texture> {
    TEXTURES.with(|cache| cache.borrow().get(&key).cloned())
}

pub(crate) fn insert(key: IconCacheKey, texture: gtk::gdk::Texture) {
    TEXTURES.with(|cache| {
        cache.borrow_mut().insert(key, texture);
    });
}

pub fn embedded_icons_ready(request: IconRenderRequest) -> bool {
    ProductIcon::ALL
        .into_iter()
        .all(|icon| icon.decode_texture(request).is_ok())
}

impl IconBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GtkSvg => "gtk_svg",
            Self::InternalVector => "internal_vector",
        }
    }
}
