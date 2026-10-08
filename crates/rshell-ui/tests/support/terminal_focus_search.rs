use super::*;

pub(super) fn inactive_search() {
    let port = Arc::new(Port::default());
    let mut window = FocusWindow::launch(port.clone());
    window.new_action(false);
    let a = SessionId::new();
    let (tab, pane) = add(&mut window, &port, "Two pane", a);
    window.publish();
    window.assert_canvas("two-pane initial A focus");
    let b = SessionId::new();
    let b_pane = rshell_core::PaneId::new();
    let tree = window
        .view
        .workspace
        .tabs
        .iter_mut()
        .find(|t| t.id == tab)
        .unwrap();
    tree.pane_tree = tree
        .pane_tree
        .clone()
        .split(pane, rshell_core::SplitAxis::Horizontal, b_pane, 0.5)
        .unwrap();
    tree.pane_tree.replace_session(b_pane, Some(b)).unwrap();
    window
        .view
        .pane_launches
        .insert(b_pane, rshell_core::PaneLaunchTarget::Local);
    window.ready(b, true);
    window.publish();
    window.assert_canvas("split retains selected A canvas");
    let inactive = descendants(window.root())
        .into_iter()
        .find(|w| w.has_css_class("pane-surface") && !w.has_css_class("active-pane"))
        .unwrap();
    let b_canvas = css(&inactive, "terminal-canvas");
    assert!(
        key(
            &b_canvas,
            gtk::gdk::Key::f,
            gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK
        ),
        "native B search action, no canvas grab"
    );
    let search = css(&inactive, "terminal-search")
        .downcast::<gtk::SearchEntry>()
        .unwrap();
    wait(|| search.is_visible(), "inactive B search opened");
    drain();
    let controllers = inactive.observe_controllers();
    let enter = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i))
        .find_map(|c| c.downcast::<gtk::EventControllerFocus>().ok())
        .unwrap();
    enter.emit_by_name::<()>("enter", &[]);
    settled(&window);
    let focus = focused(window.root()).unwrap();
    assert!(
        search.is_ancestor(&css(window.root(), "active-pane")),
        "B actually selected"
    );
    assert!(
        focus.is_ancestor(&search),
        "selected B keeps GtkText search"
    );
    search.set_text("inactive-focus-query");
    wait(
        || {
            port.commands.lock().unwrap().iter().any(|c| matches!(c,
        UiCommand::Session { session, command: SessionUiCommand::Search(q) } if *session == b && q.needle == "inactive-focus-query"))
        },
        "query Search(B)",
    );
    assert!(
        port.commands.lock().unwrap().iter().all(|c| !matches!(
            c,
            UiCommand::Session {
                command: SessionUiCommand::Input(_),
                ..
            }
        )),
        "query is not terminal Input"
    );
}
