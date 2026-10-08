use crate::{fluent_native, support::*};
use gtk::prelude::*;
use rshell_core::TabId;
use rshell_ui::SessionTabBarOutput;
use std::{cell::RefCell, rc::Rc};

pub fn assert_boundary(group: &gtk::Widget, mode: &str) {
    let children = descendants(group);
    let title = children
        .iter()
        .find(|w| w.has_css_class("tab-button"))
        .unwrap();
    let close = children
        .iter()
        .find(|w| w.has_css_class("tab-close"))
        .unwrap();
    let icon = descendants(close)
        .into_iter()
        .find(|w| w.has_css_class("product-icon"))
        .unwrap();
    let tb = title.compute_bounds(group).unwrap();
    let cb = close.compute_bounds(group).unwrap();
    let ib = icon.compute_bounds(group).unwrap();
    assert_eq!(group.clone().downcast::<gtk::Box>().unwrap().spacing(), 0);
    assert_eq!(tb.x() + tb.width(), cb.x(), "no title/close gap");
    assert_eq!(group.allocation().height(), 40);
    assert_eq!(cb.width(), 36.);
    assert_eq!((ib.width(), ib.height()), (16., 16.));
    assert_eq!(ib.x() + 8., cb.x() + cb.width() / 2.);
    assert_eq!(ib.y() + 8., cb.y() + cb.height() / 2.);
    let (rgba, width, height) = pixels(group);
    let rows = rgba
        .chunks_exact(width * 4)
        .map(|row| row.as_chunks::<4>().0.iter().filter(|p| accent(*p)).count())
        .collect::<Vec<_>>();
    println!(
        "TAB_PIXELS mode={mode} group={}x{} title={tb:?} close={cb:?} icon={ib:?} texture={width}x{height} accent_extent={}",
        group.allocation().width(),
        group.allocation().height(),
        rows.iter().max().unwrap()
    );
    assert_eq!(
        rows[height - 1],
        width,
        "shared accent must cover title, close and separator edge"
    );
    assert_eq!(rows[height - 2], width, "two-pixel bottom state boundary");
    assert_eq!(
        rows[height - 3],
        0,
        "selection boundary is exactly two pixels"
    );
    let parent = group.parent().unwrap();
    let outer = group.compute_bounds(&parent).unwrap();
    let title_outer = title.compute_bounds(&parent).unwrap();
    let close_outer = close.compute_bounds(&parent).unwrap();
    assert_eq!(
        (
            title_outer.y() - outer.y(),
            tb.height(),
            close_outer.y() - outer.y(),
            cb.height()
        ),
        (2., 36., 2., 36.)
    );
    assert_eq!(close.accessible_role(), gtk::AccessibleRole::Button);
    assert!(close.tooltip_text().is_some());
}

pub fn assert_close_states(bar: &gtk::Box, window: &gtk::ApplicationWindow, mode: &str) {
    let close = button(bar, "Close Tab 02 tab");
    assert!(close.grab_focus());
    fluent_native::wait_for_frame(window, "independent close focus", |_| true);
    assert_eq!(
        gtk::prelude::RootExt::focus(window).as_ref(),
        Some(close.upcast_ref())
    );
    let (rgba, w, h) = pixels(&close);
    assert!(
        rgba.chunks_exact(w * 4)
            .take(2)
            .all(|row| row[4 * 4..(w - 4) * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| accent(p))),
        "close owns its two-pixel focus outline inside 4px rounded corners"
    );
    assert_eq!((w, h), (36, 36));
    capture(bar, &format!("{mode}-close-focus"));
    gtk::prelude::GtkWindowExt::set_focus(window, None::<&gtk::Widget>);
    close.set_state_flags(gtk::StateFlags::PRELIGHT, false);
    fluent_native::wait_for_frame(window, "independent danger hover", |_| true);
    wait(|| {
        let (rgba, w, _) = pixels(&close);
        let pixel = &rgba[(8 * w + 8) * 4..][..4];
        pixel[0] > pixel[1] && pixel[0] > pixel[2] && pixel[3] >= 37
    });
    let (rgba, w, _) = pixels(&close);
    let pixel = &rgba[(8 * w + 8) * 4..][..4];
    assert!(
        pixel[0] > pixel[1] && pixel[0] > pixel[2] && (37..=39).contains(&pixel[3]),
        "close keeps danger hover tint"
    );
    capture(bar, &format!("{mode}-close-hover-settled"));
    close.unset_state_flags(gtk::StateFlags::PRELIGHT);
}

pub fn assert_overflow(
    bar: &gtk::Box,
    window: &gtk::ApplicationWindow,
    outputs: &Rc<RefCell<Vec<SessionTabBarOutput>>>,
    ids: &[TabId],
    mode: &str,
) {
    let menu = descendants(bar)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::MenuButton>().ok())
        .find(|w| w.has_css_class("tab-overflow"))
        .unwrap();
    menu.popup();
    let popover = menu.popover().unwrap();
    wait(|| popover.is_mapped());
    fluent_native::wait_for_frame(window, "overflow mapped", |_| true);
    let active_rows = descendants(&popover)
        .into_iter()
        .filter(|w| w.has_css_class("tab-overflow-row") && w.has_css_class("active-tab"))
        .collect::<Vec<_>>();
    assert_eq!(active_rows.len(), 1);
    assert_eq!(
        active_rows[0].tooltip_text().as_deref(),
        Some("Activate Tab 02 tab from overflow")
    );
    let last = button(&popover, "Activate Tab 20 tab from overflow");
    let scroll = descendants(&popover)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        .unwrap();
    let adjustment = scroll.vadjustment();
    adjustment.set_value(adjustment.upper() - adjustment.page_size());
    fluent_native::wait_for_frame(window, "last overflow row reachable", |_| true);
    let bounds = last.compute_bounds(&scroll).unwrap();
    assert!(bounds.y() >= 0. && bounds.y() + bounds.height() <= scroll.height() as f32);
    last.emit_clicked();
    wait(|| {
        outputs
            .borrow()
            .iter()
            .any(|o| matches!(o, SessionTabBarOutput::ActivateTab(id) if *id == ids[19]))
    });
    fluent_native::wait_for_frame(window, "last tab selected", |_| true);
    assert!(button(bar, "Activate Tab 20 tab").is_mapped());
    assert_boundary(&active_group(bar), mode);
    let strip = descendants(bar)
        .into_iter()
        .find(|w| w.has_css_class("tab-strip-scroll"))
        .unwrap()
        .downcast::<gtk::ScrolledWindow>()
        .unwrap();
    let bounds = active_group(bar).compute_bounds(&strip).unwrap();
    assert!(bounds.x() >= 0. && bounds.x() + bounds.width() <= strip.width() as f32);
    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..20 {
        visited.insert(
            active_group(bar)
                .first_child()
                .unwrap()
                .tooltip_text()
                .unwrap()
                .to_string(),
        );
        fluent_native::native_key(
            bar.upcast_ref(),
            gtk::gdk::Key::Tab,
            gtk::gdk::ModifierType::CONTROL_MASK,
        );
        fluent_native::wait_for_frame(window, "keyboard tab cycle", |_| true);
    }
    assert_eq!(visited.len(), 20);
    assert_eq!(
        button(bar, "Activate Tab 20 tab").parent().unwrap(),
        active_group(bar)
    );
}
