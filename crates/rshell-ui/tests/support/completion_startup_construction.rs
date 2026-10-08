use super::*;
use rshell_core::{ApplicationHandle, UiCommand};

pub(super) async fn run(app: &ApplicationHandle, ports: &Ports, foreign: bool) {
    if !foreign {
        let tab = app.initial_view_model().workspace.active_tab.unwrap();
        app.ui_port().try_send(UiCommand::CloseTab(tab)).unwrap();
        until(
            || app.view_model().workspace.tabs.is_empty(),
            "actual Core removed initial session before construction",
        )
        .await;
    }
    let entry = gtk::Entry::new();
    let builder = MainWindow::builder().update_root(|root| {
        if foreign {
            let header = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            header.append(&entry);
            root.set_titlebar(Some(&header));
            gtk::prelude::GtkWindowExt::set_focus(root, Some(&entry));
            assert!(
                focused(root).is_some(),
                "explicit existing constructor foreign focus"
            );
        }
    });
    let controller = builder
        .launch(MainWindowInit::from_application(app))
        .detach();
    controller.widget().set_default_size(1_000, 700);
    let window = FocusWindow {
        controller,
        view: app.view_model(),
    };
    let anchor = startup_cases::anchor(&window);
    assert!(
        !anchor.is_focusable(),
        "foreign/no-initial-session never installs anchor"
    );
    window.root().present();
    until(
        || window.root().is_mapped(),
        "constructor safeguard window mapped",
    )
    .await;
    if foreign {
        let focus = focused(window.root()).unwrap();
        assert!(focus == entry.clone().upcast::<gtk::Widget>() || focus.is_ancestor(&entry));
        ports.ready_initial();
        until(
            || {
                find_css(window.root(), "terminal-canvas")
                    .is_some_and(|c| c.is_mapped() && c.width() > 0)
            },
            "foreign constructor initial canvas arrives",
        )
        .await;
        startup_cases::painted(&window).await;
        assert_eq!(focused(window.root()).as_ref(), Some(&focus));
        ports.later_initial_frame();
        startup_cases::painted(&window).await;
        assert_eq!(focused(window.root()).as_ref(), Some(&focus));
        assert!(ports.state.lock().unwrap().inputs.is_empty());
    } else {
        assert!(find_css(window.root(), "terminal-canvas").is_none());
        assert_ne!(focused(window.root()).as_ref(), Some(&anchor));
        startup_cases::painted(&window).await;
        assert!(!anchor.is_focusable());
    }
    eprintln!(
        "STARTUP_CONSTRUCTION existing_foreign={foreign} initial_present={foreign} no_grant=true physical=false"
    );
}
