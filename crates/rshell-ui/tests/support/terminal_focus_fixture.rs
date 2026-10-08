use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{Component, ComponentController, Controller, gtk};
use rshell_core::{
    AppBootstrapState, AppSettings, AppViewModel, PaneId, PaneLaunchTarget, PaneTree, RenderFrame,
    SessionId, SessionState, TabId, TabState, TerminalProfile, TerminalSize, UiCommandPort,
};
use rshell_ui::{MainWindow, MainWindowInit, MainWindowMsg};

pub struct FocusWindow {
    pub controller: Controller<MainWindow>,
    pub view: AppViewModel,
}

impl FocusWindow {
    pub fn launch(port: Arc<dyn UiCommandPort>) -> Self {
        gtk::init().expect("native focus regression requires installed GTK/display; not skipped");
        let view = AppViewModel::from(AppBootstrapState {
            catalog: Default::default(),
            settings: AppSettings::default(),
            terminal_profiles: vec![TerminalProfile::default()],
        });
        Self::build(MainWindowInit::new(port, view.clone()), view)
    }
    fn build(init: MainWindowInit, view: AppViewModel) -> Self {
        let controller = MainWindow::builder().launch(init).detach();
        controller.widget().set_default_size(1_000, 700);
        let owned = Self { controller, view };
        owned.root().present();
        wait(|| owned.root().is_mapped(), "owned MainWindow mapping");
        owned
    }

    pub fn root(&self) -> &gtk::ApplicationWindow {
        self.controller.widget()
    }

    pub fn publish(&self) {
        self.controller
            .emit(MainWindowMsg::ReplaceViewModel(self.view.clone()));
        drain();
    }

    pub fn new_action(&self, tab_add: bool) -> gtk::Button {
        let button = if tab_add {
            css(self.root(), "tab-add").downcast().unwrap()
        } else {
            button(self.root(), "New local terminal tab")
        };
        assert!(
            button.grab_focus(),
            "arrange native command origin, never canvas assistance"
        );
        button.emit_clicked();
        drain();
        button
    }

    pub fn add(&mut self, title: &str, session: SessionId, ready: bool) -> (TabId, PaneId) {
        let tab = TabId::new_v4();
        let pane = PaneId::new();
        self.view.workspace.tabs.push(TabState {
            id: tab,
            title: title.into(),
            pane_tree: PaneTree::with_session(pane, session),
            active_pane: pane,
        });
        self.view.workspace.active_tab = Some(tab);
        self.view
            .pane_launches
            .insert(pane, PaneLaunchTarget::Local);
        self.ready(session, ready);
        (tab, pane)
    }

    pub fn ready(&mut self, session: SessionId, ready: bool) {
        self.view.session_states.insert(
            session,
            if ready {
                SessionState::Connected
            } else {
                SessionState::Connecting
            },
        );
        if ready {
            self.view.latest_frames.insert(session, frame(1));
        }
    }

    pub fn canvas(&self) -> gtk::Widget {
        css(&css(self.root(), "active-pane"), "terminal-canvas")
    }

    pub fn assert_canvas(&self, stage: &str) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            wait(
                || {
                    find_css(self.root(), "terminal-canvas").is_some_and(|_| {
                        let canvas = self.canvas();
                        canvas.is_mapped() && focused(self.root()).as_ref() == Some(&canvas)
                    })
                },
                stage,
            )
        }));
        let focus = focused(self.root());
        let canvas = find_css(self.root(), "terminal-canvas");
        eprintln!(
            "FOCUS_ROOT stage={stage} root_type={:?} canvas={} mapped={}",
            focus.as_ref().map(|w| w.type_().name()),
            focus.as_ref() == canvas.as_ref(),
            canvas.is_some_and(|w| w.is_mapped())
        );
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }

    pub fn activate(&self, title: &str) {
        let title = button(self.root(), &format!("Activate {title} tab"));
        assert!(title.grab_focus(), "native activation origin");
        title.emit_clicked();
        drain();
    }
}

impl Drop for FocusWindow {
    fn drop(&mut self) {
        self.root().close();
        drain();
        eprintln!("FOCUS_WINDOW_CLEANUP unmapped={}", !self.root().is_mapped());
        if !std::thread::panicking() {
            assert!(!self.root().is_mapped());
        }
    }
}

pub fn frame(generation: u64) -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation,
        size: TerminalSize {
            cols: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from([]),
        cursor: None,
        title: "focus fixture".into(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

pub fn focused(root: &gtk::ApplicationWindow) -> Option<gtk::Widget> {
    gtk::prelude::RootExt::focus(root)
}

pub fn button(root: &impl IsA<gtk::Widget>, tooltip: &str) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|b| b.is_mapped() && b.is_sensitive() && b.tooltip_text().as_deref() == Some(tooltip))
        .unwrap_or_else(|| panic!("missing native button {tooltip}"))
}

pub fn css(root: &impl IsA<gtk::Widget>, class: &str) -> gtk::Widget {
    find_css(root, class).unwrap_or_else(|| panic!("missing .{class}"))
}

pub fn find_css(root: &impl IsA<gtk::Widget>, class: &str) -> Option<gtk::Widget> {
    descendants(root)
        .into_iter()
        .find(|w| w.has_css_class(class))
}

pub fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    fn collect(widget: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        let mut child = widget.first_child();
        while let Some(current) = child {
            out.push(current.clone());
            collect(&current, out);
            child = current.next_sibling();
        }
    }
    let mut out = Vec::new();
    collect(root.as_ref(), &mut out);
    out
}

pub fn drain() {
    let context = gtk::glib::MainContext::default();
    for _ in 0..1_024 {
        if !context.iteration(false) {
            return;
        }
    }
}

pub fn wait(mut condition: impl FnMut() -> bool, stage: &str) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        drain();
        if condition() {
            return;
        }
        assert!(Instant::now() < deadline, "focus regression: {stage}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
