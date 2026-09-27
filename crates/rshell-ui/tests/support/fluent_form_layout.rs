use super::super::fluent_native::*;
use gtk::prelude::*;

// A static, non-secret stress label deliberately taller than the Compact viewport.
pub(crate) const LONG_ERROR: &str = concat!(
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "Validation fixture: review the highlighted fields before saving.\n",
    "The form must keep every action reachable without hiding this explanation.\n",
    "End of validation fixture."
);

pub(crate) fn modal(root: &gtk::Widget, class: &str) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|w| w.has_css_class(class) && w.is_mapped())
        .unwrap()
}

pub(super) fn fixed_actions(root: &gtk::Widget, modal: &gtk::Widget) {
    let modal_bounds = modal.compute_bounds(root).unwrap();
    let window_bounds = root.compute_bounds(root).unwrap();
    let footer = descendants(modal)
        .into_iter()
        .find(|w| w.has_css_class("dialog-footer"))
        .unwrap();
    for button in descendants(&footer)
        .into_iter()
        .filter(|w| w.is::<gtk::Button>() && w.is_visible())
    {
        let b = button.compute_bounds(root).unwrap();
        assert!(b.width() > 0.0 && b.height() > 0.0);
        assert!(
            contains(&modal_bounds, &b) && contains(&window_bounds, &b),
            "fixed action escaped modal/window: y={} bottom={} modal_bottom={} window_bottom={}",
            b.y(),
            b.y() + b.height(),
            modal_bounds.y() + modal_bounds.height(),
            window_bounds.y() + window_bounds.height()
        );
    }
    let scroller = descendants(modal)
        .into_iter()
        .find_map(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        .unwrap();
    assert!(
        scroller.height() > 0,
        "long validation eliminated scrollable body"
    );
}

fn contains(outer: &gtk::graphene::Rect, inner: &gtk::graphene::Rect) -> bool {
    inner.x() >= outer.x() - 1.0
        && inner.y() >= outer.y() - 1.0
        && inner.x() + inner.width() <= outer.x() + outer.width() + 1.0
        && inner.y() + inner.height() <= outer.y() + outer.height() + 1.0
}

fn scroller(modal: &gtk::Widget) -> gtk::ScrolledWindow {
    descendants(modal)
        .into_iter()
        .find_map(|w| w.downcast().ok())
        .unwrap()
}

pub(super) fn top(modal: &gtk::Widget) {
    let scroll = scroller(modal);
    scroll.vadjustment().set_value(0.0);
    wait_for_frame(&scroll, "body top after paint", |w| {
        w.clone()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap()
            .vadjustment()
            .value()
            == 0.0
    });
}

pub(crate) fn bottom(modal: &gtk::Widget) {
    // Inner Note scroll is traversed too, before the containing form viewport.
    let scrolls: Vec<_> = descendants(modal)
        .into_iter()
        .filter(|w| w.is_mapped())
        .filter_map(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        .collect();
    for scroll in scrolls.into_iter().rev() {
        let a = scroll.vadjustment();
        a.set_value((a.upper() - a.page_size()).max(a.lower()));
        wait_for_frame(&scroll, "body bottom after paint", |w| {
            let a = w
                .clone()
                .downcast::<gtk::ScrolledWindow>()
                .unwrap()
                .vadjustment();
            (a.value() - (a.upper() - a.page_size()).max(a.lower())).abs() <= 1.0
        });
    }
}

pub(super) fn visible_in_body(modal: &gtk::Widget, field: &impl IsA<gtk::Widget>) {
    let scroll = scroller(modal);
    assert!(
        contains(
            &scroll.compute_bounds(modal).unwrap(),
            &field.as_ref().compute_bounds(modal).unwrap()
        ),
        "last field must be reachable within the painted scroll viewport"
    );
}

pub(crate) fn reveal(modal: &gtk::Widget, field: &impl IsA<gtk::Widget>) {
    let scroll = scroller(modal);
    let a = scroll.vadjustment();
    let b = field.as_ref().compute_bounds(&scroll).unwrap();
    a.set_value((a.value() + f64::from(b.y()) - 12.0).max(0.0));
    wait_for_frame(&scroll, "field scroll paint", |_| true);
}

pub(crate) fn long_error(modal: &gtk::Widget, error: &gtk::Label) {
    assert_eq!(
        error.text(),
        LONG_ERROR,
        "all non-secret validation text is retained"
    );
    assert!(error.wraps());
    assert_eq!(error.ellipsize(), gtk::pango::EllipsizeMode::None);
    assert_eq!(error.lines(), -1);
    let scroll = scroller(modal);
    assert!(
        error.is_ancestor(&scroll),
        "long validation must be in the scrolling body"
    );
    let b = error.compute_bounds(&scroll).unwrap();
    assert!(
        b.y() + b.height() <= scroll.compute_bounds(&scroll).unwrap().height() + 1.0,
        "validation end unreachable"
    );
    assert!(b.y() + b.height() > 0.0);
    assert!(
        error.height() >= error.layout().pixel_size().1,
        "validation must not be truncated"
    );
}

pub(crate) fn verify(root: &gtk::Widget, modal: &gtk::Widget) {
    fixed_actions(root, modal);
    for widget in descendants(modal).into_iter().filter(|w| w.is_mapped()) {
        if widget.has_css_class("dialog-error") {
            let color = widget.style_context().color();
            assert!(
                (color.red() - 1.0).abs() < 0.005
                    && (color.green() - 0.6).abs() < 0.005
                    && (color.blue() - 164.0 / 255.0).abs() < 0.005,
                "rendered error must use rshell_error, not ordinary grid label color"
            );
        }
        let compound_child = widget.ancestor(gtk::DropDown::static_type()).is_some()
            || widget.ancestor(gtk::SpinButton::static_type()).is_some();
        let control = widget.is::<gtk::Entry>()
            || widget.is::<gtk::PasswordEntry>()
            || widget.is::<gtk::SpinButton>()
            || widget.is::<gtk::DropDown>()
            || widget.is::<gtk::Button>();
        if control && !compound_child {
            let b = widget.compute_bounds(modal).unwrap();
            assert!(
                b.width() > 0.0 && (36.0..=40.0).contains(&b.height()),
                "outer {} height={} must be 36–40 logical px",
                widget.type_().name(),
                b.height()
            );
            font(&widget, 15);
        }
        if widget.has_css_class("dialog-header") {
            font(&widget, 18);
        }
        if widget.has_css_class("dialog-instruction") {
            font(&widget, 14);
        }
        if widget.has_css_class("dialog-body") {
            font(&widget, 15);
        }
        if let Ok(grid) = widget.clone().downcast::<gtk::Grid>() {
            aligned(&grid);
        }
        if (widget.is::<gtk::Grid>() || widget.is::<gtk::Box>()) && !compound_child {
            siblings(&widget);
        }
    }
}

fn font(widget: &gtk::Widget, size: i32) {
    let context = widget.pango_context();
    let font = context.font_description().unwrap();
    assert!(font.is_size_absolute());
    assert_eq!(font.size(), size * gtk::pango::SCALE);
    let requested = font.family().unwrap();
    assert!(
        requested_ui_stack(&requested),
        "requested UI font stack must retain design order, got {requested}"
    );
    let resolved = context.load_font(&font).unwrap().describe();
    let resolved_family = resolved.family().unwrap();
    let available = context.list_families();
    if let Some(preferred) = ["Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI"]
        .into_iter()
        .find(|name| {
            available
                .iter()
                .any(|family| family.name().eq_ignore_ascii_case(name))
        })
    {
        assert!(
            resolved_family.eq_ignore_ascii_case(preferred),
            "requested preferred UI font {preferred} is available, but resolved {resolved_family}"
        );
    }
    // With no Segoe family installed, the requested system-ui/sans-serif fallback
    // may resolve to any locally available font; load_font above still proves it loads.
}

fn requested_ui_stack(family: &str) -> bool {
    family.split(',').map(str::trim).eq([
        "Segoe UI Variable Text",
        "Segoe UI Variable",
        "Segoe UI",
        "system-ui",
        "sans-serif",
    ])
}

#[test]
fn requested_ui_stack_rejects_wrong_family_or_order() {
    assert!(requested_ui_stack(
        "Segoe UI Variable Text,Segoe UI Variable,Segoe UI,system-ui,sans-serif"
    ));
    assert!(!requested_ui_stack(
        "Segoe UI Variable,Segoe UI Variable Text,Segoe UI,system-ui,sans-serif"
    ));
    assert!(!requested_ui_stack(
        "Arial,Segoe UI Variable,Segoe UI,system-ui,sans-serif"
    ));
}

fn children(parent: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut children = Vec::new();
    let mut child = parent.first_child();
    while let Some(w) = child {
        if w.is_mapped() {
            children.push(w.clone());
        }
        child = w.next_sibling();
    }
    children
}

fn aligned(grid: &gtk::Grid) {
    let mut edges = std::collections::BTreeMap::<i32, f32>::new();
    let mut rows = std::collections::BTreeMap::new();
    for child in children(grid.upcast_ref()) {
        if !(child.is::<gtk::Entry>()
            || child.is::<gtk::DropDown>()
            || child.is::<gtk::SpinButton>()
            || child.is::<gtk::PasswordEntry>())
        {
            continue;
        }
        let (column, row, _, _) = grid.query_child(&child);
        let bounds = child.compute_bounds(grid).unwrap();
        rows.insert((column, row), bounds);
        let x = bounds.x();
        if let Some(previous) = edges.insert(column, x) {
            assert!(
                (x - previous).abs() <= 1.0,
                "input column drift exceeds 1px"
            );
        }
    }
    for (&(column, row), bounds) in &rows {
        if let Some(previous) = rows.get(&(column, row - 1)) {
            assert_eq!(
                bounds.y() - previous.y() - previous.height(),
                8.0,
                "actual consecutive input row gap must be the 8px token"
            );
        }
    }
}

fn siblings(parent: &gtk::Widget) {
    // Compare siblings in their own content coordinate system. Offscreen body
    // content is not compared with fixed headers/footers across a viewport.
    let children = children(parent);
    for (index, left) in children.iter().enumerate() {
        let a = left.compute_bounds(parent).unwrap();
        if a.width() <= 0.0 || a.height() <= 0.0 {
            continue;
        }
        for right in &children[index + 1..] {
            let b = right.compute_bounds(parent).unwrap();
            let dx = (a.x() + a.width()).min(b.x() + b.width()) - a.x().max(b.x());
            let dy = (a.y() + a.height()).min(b.y() + b.height()) - a.y().max(b.y());
            assert!(
                dx <= 1.0 || dy <= 1.0,
                "unintended sibling overlap {} / {}: {dx}x{dy}",
                left.type_().name(),
                right.type_().name()
            );
        }
    }
}
