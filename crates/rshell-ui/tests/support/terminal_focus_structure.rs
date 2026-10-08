use super::*;

pub(super) fn run() {
    let port = Arc::new(RecordingPort::default());
    let mut window = FocusWindow::launch(port.clone());
    action(&window, &port, false);
    let a = SessionId::new();
    add(&mut window, &port, "Focus A", a, false);
    window.publish();
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas")
    );
    window.ready(a, true);
    window.publish();
    window.assert_canvas("normal delayed native creation must focus current mapped canvas");
    let old_canvas = window.canvas();
    structural(&window, a, true);
    window.assert_canvas("owned canvas must survive structural detach/reparent");
    assert_eq!(window.canvas(), old_canvas);
    assert!(key(
        &focused(window.root()).unwrap(),
        gtk::gdk::Key::f,
        gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK
    ));
    wait(
        || focused(window.root()).is_some_and(|w| w.is::<gtk::Text>()),
        "search GtkText focus",
    );
    let search_focus = focused(window.root()).unwrap();
    structural(&window, a, false);
    wait(
        || focused(window.root()).as_ref() == Some(&search_focus),
        "owned GtkText after layout",
    );
    assert!(key(
        &css(window.root(), "terminal-search"),
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    window.assert_canvas("search Escape must retain existing canvas return");
    let canvas_before_modal = window.canvas();
    window.controller.emit(MainWindowMsg::OpenSettings);
    drain();
    let modal_focus = focused(window.root());
    structural(&window, a, true);
    assert_eq!(focused(window.root()), modal_focus);
    assert!(key(
        &css(window.root(), "settings-window"),
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    wait(
        || focused(window.root()).as_ref() == Some(&canvas_before_modal),
        "Settings returns original canvas",
    );
    action(&window, &port, true);
    let b = SessionId::new();
    add(&mut window, &port, "Focus B", b, true);
    window.publish();
    window.assert_canvas("tab-add producer focuses exact new identity");
    window.activate("Focus A");
    window.assert_canvas("switch back recreated controller");
    assert_ne!(window.canvas(), old_canvas);
    window.activate("Focus A");
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas")
    );
    window.activate("Focus B");
    window.assert_canvas("switch second identity");
    settle_toolbar(&window);
    let tip = direct_children(&css(
        &css(window.root(), "active-pane"),
        "pane-action-region",
    ))
    .into_iter()
    .find(|w| w.is::<gtk::Button>() && w.is_mapped())
    .and_then(|w| w.tooltip_text())
    .unwrap();
    for tooltip in ["Terminal settings", tip.as_str()] {
        settle_toolbar(&window);
        assert!(button(window.root(), tooltip).grab_focus());
        let foreign = focused(window.root());
        window
            .controller
            .emit(MainWindowMsg::AppEvent(AppEvent::Session {
                session: b,
                event: SessionUiEvent::Frame(frame(9)),
            }));
        drain();
        assert_eq!(
            focused(window.root()),
            foreign,
            "frame-only cannot grab action focus"
        );
        structural(&window, b, true);
        assert!(!focused(window.root()).is_some_and(|w| w.has_css_class("terminal-canvas")));
        wait(
            || focused(window.root()).is_some_and(|w| w.tooltip_text().as_deref() == Some(tooltip)),
            "equivalent action restored",
        );
        structural(&window, b, false);
    }
    let entry = descendants(&css(window.root(), "sidebar"))
        .into_iter()
        .find_map(|w| w.downcast::<gtk::SearchEntry>().ok())
        .unwrap();
    assert!(entry.grab_focus());
    let foreign = focused(window.root());
    structural(&window, b, true);
    assert_eq!(focused(window.root()), foreign);
    window
        .controller
        .emit(MainWindowMsg::AppEvent(AppEvent::Session {
            session: b,
            event: SessionUiEvent::Frame(frame(10)),
        }));
    drain();
    assert_eq!(focused(window.root()), foreign);
}

pub(super) fn direct_children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let children = widget.observe_children();
    (0..children.n_items())
        .filter_map(|i| children.item(i))
        .filter_map(|w| w.downcast::<gtk::Widget>().ok())
        .collect()
}
pub(super) fn settle_toolbar(window: &FocusWindow) {
    wait(
        || {
            let pane = css(window.root(), "active-pane");
            let region = css(&pane, "pane-action-region");
            let desired = rshell_ui::PaneActionLayout::for_width(
                &[
                    rshell_ui::PaneAction::SplitHorizontal,
                    rshell_ui::PaneAction::SplitVertical,
                    rshell_ui::PaneAction::Reconnect,
                    rshell_ui::PaneAction::Close,
                ],
                pane.width(),
            );
            let widgets = direct_children(&region);
            pane.width() > 0
                && widgets.iter().filter(|w| w.is::<gtk::Button>()).count() == desired.visible.len()
                && widgets.iter().filter(|w| w.is::<gtk::MenuButton>()).count()
                    == usize::from(!desired.overflow.is_empty())
                && widgets
                    .iter()
                    .all(|w| w.is_mapped() && w.width() > 0 && w.height() > 0)
        },
        "actual toolbar band and positive allocations",
    );
}
