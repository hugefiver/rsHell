use std::{cell::RefCell, collections::BTreeMap};

use relm4::gtk;

use crate::{IconBackend, ProductIcon};

thread_local! {
    static TEXTURES: RefCell<BTreeMap<ProductIcon, gtk::gdk::Texture>> =
        const { RefCell::new(BTreeMap::new()) };
}

pub(crate) fn get(icon: ProductIcon) -> Option<gtk::gdk::Texture> {
    TEXTURES.with(|cache| cache.borrow().get(&icon).cloned())
}

pub(crate) fn insert(icon: ProductIcon, texture: gtk::gdk::Texture) {
    TEXTURES.with(|cache| {
        cache.borrow_mut().insert(icon, texture);
    });
}

pub fn embedded_icons_ready() -> bool {
    ProductIcon::ALL
        .into_iter()
        .all(|icon| icon.decode_texture().is_ok())
}

impl IconBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GtkSvg => "gtk_svg",
            Self::InternalVector => "internal_vector",
        }
    }
}
