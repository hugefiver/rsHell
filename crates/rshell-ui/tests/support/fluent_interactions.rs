//! Synthetic, case-bound proof through production MainWindow, never SSH evidence.
use super::{
    fluent_forms::{actions, layout},
    fluent_measure,
    fluent_native::*,
};
use gtk::prelude::*;
use relm4::ComponentController;
use rshell_core::{
    AppEvent, AuthPrompt, HostKeyPrompt, InteractionId, InteractionRequest,
    KeyboardInteractivePrompt, SessionId,
};
use rshell_ui::{MainWindow, MainWindowMsg};
use std::sync::Arc;

#[path = "fluent_interaction_auth.rs"]
mod auth;
#[path = "fluent_interaction_diagnostics.rs"]
mod diagnostics;
#[path = "fluent_interaction_layout.rs"]
mod measurements;
#[path = "fluent_interaction_port.rs"]
mod port;
use port::{Port, Response};
#[path = "fluent_interaction_fixtures.rs"]
mod fixtures;
use fixtures::{auth_request, host};

pub(crate) fn run(mode: &str, width: i32, height: i32) {
    for (case, changed, action, response) in [
        ("host-unknown-reject", false, "Reject", Response::Reject),
        (
            "host-unknown-accept",
            false,
            "Accept and store",
            Response::Accept,
        ),
        ("host-changed-close", true, "Close", Response::Reject),
        ("host-unknown-escape", false, "Escape", Response::Reject),
    ] {
        let c = Case::open(mode, width, height, case, host(changed), false);
        if changed {
            assert!(
                !descendants(&c.modal)
                    .iter()
                    .filter_map(|w| w.clone().downcast::<gtk::Label>().ok())
                    .any(|l| l.text() == "Accept and store")
            );
            button(&c.modal, "Copy diagnostics").emit_clicked();
            wait_for_frame(c.main.widget(), "diagnostics action", |_| true);
            c.port.verify(c.session, c.interaction, &[]);
            assert!(c.modal.is_mapped());
            // Clipboard is synthetic; inspect it without printing or retaining its text.
            let copied = gtk::glib::MainContext::default()
                .block_on(c.main.widget().clipboard().read_text_future())
                .unwrap()
                .unwrap();
            assert!(copied.contains("visual.example.test:2222"));
            assert!(copied.starts_with("Host key changed for"));
            // Keep the non-secret provider alive through native delayed clipboard rendering.
        }
        c.act(action);
        c.pending();
        c.port.verify(c.session, c.interaction, &[response]);
        c.shot("pending");
        c.ack();
    }
    auth::run(mode, width, height);
    println!(
        "SYNTHETIC_INTERACTIONS_PASS mode={mode} independent_cases=9 old_smoke_flags_used=false"
    );
}

struct Case {
    main: relm4::Controller<MainWindow>,
    port: Arc<Port>,
    modal: gtk::Widget,
    trigger: gtk::Button,
    focus: gtk::Widget,
    background: gtk::Widget,
    session: SessionId,
    interaction: InteractionId,
    mode: String,
    name: String,
}

impl Case {
    fn open(
        mode: &str,
        width: i32,
        height: i32,
        name: &str,
        request: InteractionRequest,
        ordinary_first: bool,
    ) -> Self {
        let port = Arc::new(Port::default());
        let main = launch_with_port(width, height, port.clone());
        let root = main.widget().upcast_ref::<gtk::Widget>();
        let trigger = actions::trigger(root, "Terminal settings");
        if ordinary_first {
            let (modal, ordinary_trigger) =
                actions::open(root, "Terminal settings", "settings-window");
            actions::escape(root, &modal, &ordinary_trigger);
        }
        assert!(trigger.grab_focus());
        wait_for_frame(
            &trigger,
            "actual trigger focus before interaction",
            focus_within,
        );
        let focus = gtk::prelude::RootExt::focus(main.widget()).unwrap();
        assert_eq!(focus, trigger.clone().upcast::<gtk::Widget>());
        let background = descendants(root)
            .into_iter()
            .find(|w| w.has_css_class("modal-background"))
            .unwrap();
        let session = SessionId::new();
        let interaction = match &request {
            InteractionRequest::HostKey(p) => p.id,
            InteractionRequest::Password(p) | InteractionRequest::PrivateKeyPassphrase(p) => p.id,
            InteractionRequest::KeyboardInteractive(p) => p.id,
        };
        main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
            session,
            request,
        }));
        diagnostics::wait(root, mode, name, |root| {
            modal_ready(root, "interaction-dialog") && measurements::first_open_ready(root)
        });
        let modal = layout::modal(root, "interaction-dialog");
        assert!(!background.is_sensitive());
        assert!(!trigger.is_sensitive());
        assert!(focus_within(&modal));
        port.verify(session, interaction, &[]);
        let c = Self {
            main,
            port,
            modal,
            trigger,
            focus,
            background,
            session,
            interaction,
            mode: mode.into(),
            name: name.into(),
        };
        c.shot("open");
        measurements::verify(&c, width, height);
        c.contain();
        // A stale acknowledgement cannot close a new interaction or lend it completion.
        c.main
            .emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
                session,
                interaction: InteractionId::new(),
            }));
        wait_for_frame(c.main.widget(), "wrong acknowledgement processed", |_| true);
        assert!(c.modal.is_mapped() && !c.background.is_sensitive());
        assert_ne!(
            gtk::prelude::RootExt::focus(c.main.widget()).as_ref(),
            Some(&c.focus)
        );
        c.port.verify(session, interaction, &[]);
        println!(
            "SYNTHETIC_OPEN case={name} session={session:?} interaction={interaction:?} actual_focus_saved=true cancel=false restore=false wrong_ack_ignored=true ordinary_escape_preceded={ordinary_first}"
        );
        c
    }

    fn contain(&self) {
        for modifiers in [
            gtk::gdk::ModifierType::empty(),
            gtk::gdk::ModifierType::SHIFT_MASK,
        ] {
            for _ in 0..12 {
                native_key(&self.modal, gtk::gdk::Key::Tab, modifiers);
                wait_for_frame(&self.modal, "contained native Tab", focus_within);
                assert!(focus_within(&self.modal));
            }
        }
    }

    fn act(&self, action: &str) {
        if action == "Escape" {
            native_key(
                &self.modal,
                gtk::gdk::Key::Escape,
                gtk::gdk::ModifierType::empty(),
            );
        } else {
            let b = button(&self.modal, action);
            assert!(b.is_sensitive());
            b.emit_clicked();
        }
    }

    fn pending(&self) {
        wait_for_frame(&self.modal, "pending action sensitivity", |modal| {
            descendants(modal)
                .iter()
                .filter(|w| w.is::<gtk::Button>())
                .all(|w| !w.is_sensitive())
        });
        assert!(self.modal.is_mapped() && !self.background.is_sensitive());
        assert_ne!(
            gtk::prelude::RootExt::focus(self.main.widget()).as_ref(),
            Some(&self.focus)
        );
        for input in descendants(&self.modal) {
            if let Ok(p) = input.clone().downcast::<gtk::PasswordEntry>() {
                assert!(!p.is_sensitive());
                assert!(p.text().is_empty());
            }
            if let Ok(p) = input.downcast::<gtk::Entry>() {
                assert!(!p.is_sensitive());
                assert!(p.text().is_empty());
            }
        }
        native_key(
            &self.modal,
            gtk::gdk::Key::Return,
            gtk::gdk::ModifierType::empty(),
        );
        wait_for_frame(
            &self.modal,
            "pending repeated submit remains contained",
            |_| true,
        );
    }

    fn shot(&self, state: &str) {
        capture(
            self.main.widget(),
            &self.mode,
            &format!("synthetic-{}-{state}", self.name),
        );
    }

    fn ack(self) {
        self.main
            .emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
                session: self.session,
                interaction: self.interaction,
            }));
        actions::closed(self.main.widget().upcast_ref(), &self.modal, &self.trigger);
        assert!(self.background.is_sensitive() && self.trigger.is_sensitive());
        assert_eq!(
            gtk::prelude::RootExt::focus(self.main.widget()).as_ref(),
            Some(&self.focus)
        );
        self.shot("ack-restored");
        println!(
            "SYNTHETIC_CLOSE case={} session={:?} interaction={:?} ack=true hidden=true background_sensitive=true exact_focus_restored=true",
            self.name, self.session, self.interaction
        );
        close_window(self.main.widget());
    }
}
