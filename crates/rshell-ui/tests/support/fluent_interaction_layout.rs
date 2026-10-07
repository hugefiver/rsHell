use super::*;

// Kind: 0 absent, 1 PasswordEntry, 2 Entry, 3 Button, 4 Text, 5 other.
fn kind(widget: Option<&gtk::Widget>) -> u8 {
    widget.map_or(0, |w| {
        if w.is::<gtk::PasswordEntry>() {
            1
        } else if w.is::<gtk::Entry>() {
            2
        } else if w.is::<gtk::Button>() {
            3
        } else if w.is::<gtk::Text>() {
            4
        } else {
            5
        }
    })
}

pub(super) fn observe(root: &gtk::Widget) -> diagnostics::State {
    let mut state = diagnostics::State {
        modal_ready: modal_ready(root, "interaction-dialog"),
        outer_ready: first_open_ready(root),
        active: root.downcast_ref::<gtk::Window>().map(|w| w.is_active()),
        ..Default::default()
    };
    let focused = root.root().and_then(|r| gtk::prelude::RootExt::focus(&r));
    state.focus_kind = kind(focused.as_ref());
    state.focus = focused
        .as_ref()
        .map(|w| (w.is_mapped(), w.is_sensitive(), w.width(), w.height()));
    state.same_root = focused.as_ref().map(|w| w.root() == root.root());
    let Some(modal) = descendants(root)
        .into_iter()
        .find(|w| w.has_css_class("interaction-dialog"))
    else {
        return state;
    };
    state.modal = Some((modal.is_mapped(), modal.width(), modal.height()));
    state.focus_in_modal = focused
        .as_ref()
        .is_some_and(|w| *w == modal || w.is_ancestor(&modal));
    let Some(first) = descendants(&modal)
        .into_iter()
        .find(|w| w.has_css_class("modal-focus-first"))
    else {
        return state;
    };
    state.first_kind = kind(Some(&first));
    state.first = Some((
        first.is_mapped(),
        first.is_sensitive(),
        first.width(),
        first.height(),
    ));
    state.focus_relation = focused.as_ref().map_or(0, |w| {
        if *w == first {
            1
        } else if w.is_ancestor(&first) {
            2
        } else {
            3
        }
    });
    let bounds = |w: &gtk::Widget| {
        w.compute_bounds(root)
            .map(|b| [b.x(), b.y(), b.width(), b.height()])
    };
    state.input = bounds(&first);
    if let Some(viewport) = first.ancestor(gtk::Viewport::static_type()) {
        state.viewport = bounds(&viewport);
        state.viewport_allocation = Some((viewport.width(), viewport.height()));
        state.scroll_to_focus = Some(viewport.property::<bool>("scroll-to-focus"));
    }
    if let (Some(i), Some(v)) = (state.input, state.viewport) {
        state.overflow_ltrb = Some([
            (v[0] - i[0]).max(0.0),
            (v[1] - i[1]).max(0.0),
            (i[0] + i[2] - v[0] - v[2]).max(0.0),
            (i[1] + i[3] - v[1] - v[3]).max(0.0),
        ]);
    }
    if let Some(scroll) = first
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
    {
        let a = scroll.vadjustment();
        state.adjustment_luvp = Some([a.lower(), a.upper(), a.value(), a.page_size()]);
    }
    state
}

// Read-only readiness: allow GTK's post-layout focus reconciliation to paint.
// No adjustment writes or test-local reveal are permitted before this predicate.
pub(super) fn first_open_ready(root: &gtk::Widget) -> bool {
    let Some(modal) = descendants(root)
        .into_iter()
        .find(|w| w.has_css_class("interaction-dialog"))
    else {
        return false;
    };
    let Some(first) = descendants(&modal)
        .into_iter()
        .find(|w| w.has_css_class("modal-focus-first"))
    else {
        return false;
    };
    if !first.is::<gtk::Entry>() && !first.is::<gtk::PasswordEntry>() {
        return true;
    }
    let Some(viewport) = first.ancestor(gtk::Viewport::static_type()) else {
        return false;
    };
    let Some(visible) = viewport.compute_bounds(root) else {
        return false;
    };
    let Some(input) = first.compute_bounds(root) else {
        return false;
    };
    input.x() >= visible.x() - 2.0
        && input.y() >= visible.y() - 2.0
        && input.x() + input.width() <= visible.x() + visible.width() + 2.0
        && input.y() + input.height() <= visible.y() + visible.height() + 2.0
}

pub(super) fn verify(c: &Case, width: i32, height: i32) {
    let root = c.main.widget().upcast_ref::<gtk::Widget>();
    assert!(
        (root.width() - width).abs() <= 2 && (root.height() - height).abs() <= 2,
        "long secure text must not enlarge realized window"
    );
    assert!(
        descendants(root)
            .iter()
            .any(|w| w.has_css_class(&format!("shell-{}", c.mode)))
    );
    let b = c.modal.compute_bounds(root).unwrap();
    println!(
        "SYNTHETIC_MODAL case={} requested={width}x{height} actual={}x{} mode={} bounds={b:?}",
        c.name,
        root.width(),
        root.height(),
        c.mode
    );
    assert!(
        b.width() <= 682.0,
        "long host summary/prompt must not grow modal beyond cap"
    );
    fluent_measure::record(root, &c.mode, "interaction-dialog");
    layout::verify(root, &c.modal);
    let inputs = descendants(&c.modal);
    let masked = inputs
        .iter()
        .filter(|w| w.is::<gtk::PasswordEntry>())
        .count();
    let echo = inputs.iter().filter(|w| w.is::<gtk::Entry>()).count();
    assert_eq!(masked, usize::from(!c.name.starts_with("host-")));
    assert_eq!(echo, usize::from(c.name == "keyboard"));
    let instruction = inputs
        .iter()
        .find(|w| w.has_css_class("dialog-instruction"))
        .unwrap();
    let contrast = pixels(instruction).maximum_text_contrast();
    assert!(
        contrast >= 4.5,
        "secure summary must retain readable token contrast"
    );
    println!(
        "SYNTHETIC_TYPE case={} masked={masked} echo={echo} instruction_contrast={contrast:.2}",
        c.name
    );
    for label in descendants(&c.modal)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
    {
        if label.text().len() > 180 {
            assert!(label.wraps(), "long secure label must wrap");
            assert!(
                label.hexpands(),
                "long secure label must use available width"
            );
            assert!(
                label.height() >= label.layout().pixel_size().1,
                "long secure label is cropped"
            );
        }
    }
    let first = descendants(&c.modal)
        .into_iter()
        .find(|w| w.has_css_class("modal-focus-first"))
        .unwrap();
    assert!(focus_within(&first));
    if first.is::<gtk::Entry>() || first.is::<gtk::PasswordEntry>() {
        let viewport = descendants(&c.modal)
            .into_iter()
            .find(|w| w.is::<gtk::Viewport>())
            .unwrap();
        let visible = viewport.compute_bounds(root).unwrap();
        let input = first.compute_bounds(root).unwrap();
        println!(
            "SYNTHETIC_FIRST_OPEN case={} mode={} viewport={visible:?} focused_input={input:?} actual_focus=true test_scroll=false native_scroll_to_focus={}",
            c.name,
            c.mode,
            viewport.property::<bool>("scroll-to-focus")
        );
        assert!(
            input.x() >= visible.x() - 2.0
                && input.y() >= visible.y() - 2.0
                && input.x() + input.width() <= visible.x() + visible.width() + 2.0
                && input.y() + input.height() <= visible.y() + visible.height() + 2.0,
            "first-open focused input outer bounds must be fully visible before any test scrolling (2px boundary tolerance)"
        );
    }
    c.shot("first-open-focus");
    let p = pixels(root);
    let b = first.compute_bounds(root).unwrap();
    assert!(
        is_accent(p.at(b.x() as i32 + 1, (b.y() + b.height() / 2.0) as i32)),
        "secure first control needs 2px accent focus"
    );
    println!(
        "SYNTHETIC_LAYOUT case={} cap=true body_footer=true pango=15/14/18 focus_accent=true",
        c.name
    );
}
