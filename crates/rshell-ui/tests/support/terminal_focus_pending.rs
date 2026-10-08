use super::*;

pub(super) fn run() {
    let port = Arc::new(RecordingPort::default());
    let mut window = FocusWindow::launch(port.clone());
    action(&window, &port, false);
    let a = SessionId::new();
    let (tab, _) = add(&mut window, &port, "Waiting A", a, false);
    window.publish();
    action(&window, &port, true);
    let b = SessionId::new();
    add(&mut window, &port, "Waiting B", b, false);
    window.publish();
    window.activate("Waiting A");
    window.activate("Waiting B");
    window.activate("Waiting A");
    window.ready(b, true);
    window.publish();
    assert!(
        !focused(window.root())
            .unwrap()
            .has_css_class("terminal-canvas")
    );
    window.ready(a, true);
    window
        .controller
        .emit(MainWindowMsg::AppEvent(AppEvent::Session {
            session: a,
            event: SessionUiEvent::State(rshell_core::SessionState::Connected),
        }));
    window
        .controller
        .emit(MainWindowMsg::AppEvent(AppEvent::Session {
            session: a,
            event: SessionUiEvent::Frame(frame(1)),
        }));
    window.publish();
    window.assert_canvas("A/B/A ignores old target callbacks");
    window.activate("Waiting B");
    window.assert_canvas("latest controller B");
    let settings = button(window.root(), "Terminal settings");
    assert!(settings.grab_focus());
    settings.emit_clicked();
    drain();
    let modal_focus = focused(window.root());
    structural(&window, b, true);
    assert_eq!(focused(window.root()), modal_focus);
    window.controller.emit(MainWindowMsg::NewLocalTab);
    drain();
    add(&mut window, &port, "Modal", SessionId::new(), true);
    window.publish();
    assert_eq!(focused(window.root()), modal_focus);
    assert!(key(
        &css(window.root(), "settings-window"),
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    drain();
    assert_eq!(focused(window.root()).as_ref(), Some(settings.upcast_ref()));
    window.controller.emit(MainWindowMsg::Sidebar(
        rshell_ui::ConnectionSidebarOutput::OpenCreate(None),
    ));
    drain();
    let editor = css(window.root(), "editor-dialog");
    let name = descendants(&editor)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Label>().ok())
        .find(|l| l.text().as_str() == "Name")
        .and_then(|l| l.next_sibling())
        .and_then(|w| w.downcast::<gtk::Entry>().ok())
        .unwrap();
    assert!(name.grab_focus());
    name.set_text("Unsaved focus regression draft");
    let editor_focus = focused(window.root());
    window.controller.emit(MainWindowMsg::NewLocalTab);
    drain();
    add(&mut window, &port, "Editor", SessionId::new(), true);
    window.publish();
    assert_eq!(focused(window.root()), editor_focus);
    assert_eq!(name.text(), "Unsaved focus regression draft");
    assert!(key(
        &editor,
        gtk::gdk::Key::Escape,
        gtk::gdk::ModifierType::empty()
    ));
    drain();
    action(&window, &port, false);
    let closed = SessionId::new();
    add(&mut window, &port, "Closed pending", closed, false);
    window.publish();
    window
        .view
        .workspace
        .tabs
        .retain(|t| t.title != "Closed pending");
    window.view.workspace.active_tab = Some(tab);
    window.publish();
    window
        .controller
        .emit(MainWindowMsg::AppEvent(AppEvent::Session {
            session: closed,
            event: SessionUiEvent::Frame(frame(20)),
        }));
    drain();
    assert!(!focused(window.root()).is_some_and(|w| w.has_css_class("terminal-canvas")));
    action(&window, &port, false);
    window.root().close();
    drain();
    assert!(port.receipts.receiver_closed(port.receipts.count() - 1));
    window.ready(a, true);
    window.publish();
    assert!(!window.root().is_mapped());
}
