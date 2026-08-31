use gtk::pango;
use gtk::pango::prelude::*;

pub(crate) fn requested_family_is_exact_monospace(
    context: &pango::Context,
    requested_family: &str,
) -> bool {
    let requested_family = requested_family.trim();
    !requested_family.is_empty()
        && context.font_map().is_some_and(|font_map| {
            font_map.list_families().iter().any(|family| {
                family.name().eq_ignore_ascii_case(requested_family) && family.is_monospace()
            })
        })
}
