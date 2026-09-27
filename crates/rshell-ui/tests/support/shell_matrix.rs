use super::fluent_native::{capture, close_window, descendants, wait_for_frame};
use super::{AcceptingPort, find_by_css_class, visual_fixture};
use gtk::prelude::*;
use relm4::{Component, ComponentController};
use rshell_core::{
    DisplayRecoveryNotice, ErrorPaneView, PaneId, PaneLaunchTarget, PaneTree, SessionFailure,
    SessionId, SessionState, TabId, TabState,
};
use rshell_ui::{MainWindow, MainWindowInit, MainWindowMsg};
use std::sync::Arc;

pub(super) fn run() {
    let mut defects = Vec::new();
    for (mode, width, height) in [
        ("compact", 800, 600),
        ("standard", 1360, 860),
        ("wide", 1920, 1080),
    ] {
        let mut view = visual_fixture();
        let pane = view.workspace.tabs[0].active_pane;
        let session = view.workspace.tabs[0].pane_tree.session_ids()[0];
        let main = MainWindow::builder()
            .launch(MainWindowInit::new(Arc::new(AcceptingPort), view.clone()))
            .detach();
        main.widget().set_default_size(width, height);
        main.widget().present();
        let class = format!("shell-{mode}");
        wait_for_frame(main.widget(), "true shell mode", move |root| {
            find_by_css_class(root, &class).is_some()
        });
        state(&main, mode, "shell-connected", &mut defects);
        for i in 1..20 {
            let p = PaneId::new();
            let s = SessionId::new();
            view.workspace.tabs.push(TabState {
                id: TabId::new_v4(),
                title: format!("Workspace {:02}", i + 1),
                pane_tree: PaneTree::with_session(p, s),
                active_pane: p,
            });
            view.pane_launches.insert(p, PaneLaunchTarget::Local);
            view.session_states.insert(s, SessionState::Connected);
            view.latest_frames
                .insert(s, view.latest_frames[&session].clone());
        }
        main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
        wait_for_frame(main.widget(), "twenty tabs", |root| {
            descendants(root)
                .iter()
                .any(|w| w.has_css_class("tab-overflow-row"))
        });
        state(&main, mode, "shell-twenty-tabs", &mut defects);
        let menu = descendants(main.widget().upcast_ref())
            .into_iter()
            .find_map(|w| {
                w.has_css_class("tab-overflow")
                    .then(|| w.downcast::<gtk::MenuButton>().ok())
                    .flatten()
            })
            .unwrap();
        menu.popup();
        wait_for_frame(main.widget(), "tab overflow open", |root| {
            descendants(root)
                .iter()
                .any(|w| w.is_mapped() && w.has_css_class("tab-overflow-row"))
        });
        let popover = menu.popover().unwrap();
        let row = descendants(popover.upcast_ref())
            .into_iter()
            .find(|w| w.is_mapped() && w.has_css_class("tab-overflow-row"))
            .expect("mapped overflow button in native popover");
        let bounds = row
            .compute_bounds(&row.parent().expect("overflow button parent"))
            .expect("mapped overflow button bounds in its parent");
        assert!(
            bounds.width() >= 36.0 && bounds.height() >= 36.0,
            "{mode} overflow button must have a 36px target: {bounds:?}"
        );
        state(&main, mode, "shell-tabs-overflow", &mut defects);
        super::surface_capture::capture(&popover, &format!("{mode}-shell-tabs-popover"));
        menu.popdown();
        if mode == "compact" {
            let drawer = descendants(main.widget().upcast_ref())
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Button>().ok())
                .find(|b| b.tooltip_text().as_deref() == Some("Navigation"))
                .expect("drawer trigger");
            drawer.emit_clicked();
            wait_for_frame(main.widget(), "drawer open", |root| {
                find_by_css_class(root, "sidebar").is_some_and(|w| w.is_mapped())
            });
            state(&main, mode, "shell-drawer", &mut defects);
            drawer.emit_clicked();
        }
        let connection = *view.catalog.connections.keys().next().unwrap();
        view.pane_launches.insert(
            pane,
            PaneLaunchTarget::Connection {
                id: connection,
                host: "safe.example.test".into(),
            },
        );
        view.session_states.insert(session, SessionState::Failed);
        view.error_panes.insert(
            session,
            ErrorPaneView {
                failure: SessionFailure::Authentication,
                diagnostic: "Synthetic authentication failed",
                host: Some("safe.example.test".into()),
                timestamp_unix_seconds: 0,
            },
        );
        main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
        wait_for_frame(main.widget(), "failure page", |root| {
            find_by_css_class(root, "pane-error-actions").is_some()
        });
        state(&main, mode, "shell-failure", &mut defects);
        view.error_panes.clear();
        view.session_states.insert(session, SessionState::Connected);
        view.display_recovery.insert(
            session,
            DisplayRecoveryNotice {
                interrupted_generation: 1,
                observed_generation: 2,
                modes: rshell_core::TerminalDisplayModes {
                    alternate_screen: true,
                    ..Default::default()
                },
            },
        );
        main.emit(MainWindowMsg::ReplaceViewModel(view.clone()));
        wait_for_frame(main.widget(), "recovery notice", |root| {
            find_by_css_class(root, "display-recovery-notice").is_some()
        });
        state(&main, mode, "shell-recovery", &mut defects);
        view.workspace = Default::default();
        view.latest_frames.clear();
        view.session_states.clear();
        view.display_recovery.clear();
        view.pane_launches.clear();
        main.emit(MainWindowMsg::ReplaceViewModel(view));
        wait_for_frame(main.widget(), "empty workspace", |root| {
            !descendants(root)
                .iter()
                .any(|w| w.has_css_class("pane-surface"))
        });
        state(&main, mode, "shell-empty", &mut defects);
        close_window(main.widget());
    }
    assert!(defects.is_empty(), "shell target defects: {defects:#?}");
}

fn state(main: &relm4::Controller<MainWindow>, mode: &str, state: &str, defects: &mut Vec<String>) {
    wait_for_frame(main.widget(), "shell state paint", |_| true);
    let root = main.widget();
    assert!(find_by_css_class(root.upcast_ref(), &format!("shell-{mode}")).is_some());
    println!(
        "SHELL_STATE mode={mode} state={state} realized={}x{} scale={} font={}",
        root.width(),
        root.height(),
        root.scale_factor(),
        root.pango_context().font_description().unwrap()
    );
    for w in descendants(root.upcast_ref())
        .into_iter()
        .filter(|w| w.is_mapped())
    {
        if w.has_css_class("product-icon")
            && w.parent().is_some_and(|p| p.is::<gtk::Button>())
            && (within_class(&w, "pane-command-row") || within_class(&w, "sidebar-toolbar"))
        {
            let b = w.compute_bounds(root).unwrap();
            if b.width() > 16.0 || b.height() > 16.0 {
                defects.push(format!(
                    "{mode}/{state} stretched icon {}x{}",
                    b.width(),
                    b.height()
                ));
            }
        }
        if [
            "command-bar",
            "sidebar",
            "tab-bar",
            "pane-host",
            "pane-command-row",
            "terminal-canvas",
            "display-recovery-notice",
        ]
        .iter()
        .any(|c| w.has_css_class(c))
        {
            println!(
                "SHELL_RECT classes={:?} rect={:?}",
                w.css_classes(),
                w.compute_bounds(root)
            );
        }
        if w.is::<gtk::Button>() && w.ancestor(gtk::WindowControls::static_type()).is_none() {
            let b = button_bounds(&w, root.upcast_ref());
            if b.width() < 36.0 || b.height() < 36.0 {
                defects.push(format!(
                    "{mode}/{state} {:?} {}x{}",
                    w.css_classes(),
                    b.width(),
                    b.height()
                ));
            }
        }
    }
    capture(root, mode, state);
}

fn button_bounds(button: &gtk::Widget, root: &gtk::Widget) -> gtk::graphene::Rect {
    button
        .compute_bounds(root)
        .or_else(|| {
            button.ancestor(gtk::Popover::static_type())?;
            button.compute_bounds(&button.parent()?)
        })
        .expect("mapped button must have bounds in its window or popover parent")
}

fn within_class(widget: &gtk::Widget, class: &str) -> bool {
    let mut current = widget.parent();
    while let Some(w) = current {
        if w.has_css_class(class) {
            return true;
        }
        current = w.parent();
    }
    false
}
