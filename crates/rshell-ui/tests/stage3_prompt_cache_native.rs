//! Native production reproduction for the GTK 4.16 empty-container mode cache.
//! GTK 4.18.5 refreshes this getter; a pass there is not older-runtime RED/GREEN.
//! Older-runtime RED is unverified: run this fixture on actual GTK 4.16.12.
//! No cache override, adjustment write, substitute renderer or external input.
#![cfg(not(target_os = "macos"))]
use gtk::prelude::*;
use relm4::ComponentController;
use rshell_core::{
    AppEvent, AuthPrompt, HostKeyPrompt, InteractionId, InteractionRequest,
    KeyboardInteractivePrompt, SessionId,
};
use rshell_ui::{MainWindowMsg, apply_global_css};

#[path = "support/stage3_prompt_cache_failure.rs"]
mod failure;
#[path = "support/fluent_interaction_fixtures.rs"]
mod fixtures;
#[path = "support/fluent_native.rs"]
#[allow(dead_code, unused_imports)]
mod native;
#[path = "support/stage3_prompt_cache_report.rs"]
mod report;

fn interaction(root: &gtk::Widget) -> gtk::Widget {
    native::descendants(root)
        .into_iter()
        .find(|w| w.has_css_class("interaction-dialog"))
        .unwrap()
}
fn first(root: &gtk::Widget) -> Option<gtk::Widget> {
    native::descendants(&interaction(root))
        .into_iter()
        .find(|w| w.has_css_class("modal-focus-first"))
}
fn outer_ready(root: &gtk::Widget) -> bool {
    let Some(input) = first(root) else {
        return false;
    };
    if !input.is::<gtk::PasswordEntry>() && !input.is::<gtk::Entry>() {
        return true;
    }
    let Some(viewport) = input.ancestor(gtk::Viewport::static_type()) else {
        return false;
    };
    let Some(v) = viewport.compute_bounds(root) else {
        return false;
    };
    let Some(b) = input.compute_bounds(root) else {
        return false;
    };
    b.x() >= v.x() - 2.0
        && b.y() >= v.y() - 2.0
        && b.x() + b.width() <= v.x() + v.width() + 2.0
        && b.y() + b.height() <= v.y() + v.height() + 2.0
}
fn open_and_close(request: InteractionRequest, action: Option<&str>, prime_empty: bool) {
    let main = native::launch(800, 600);
    let root = main.widget().upcast_ref::<gtk::Widget>();
    let trigger = native::descendants(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|w| w.is_mapped() && w.tooltip_text().as_deref() == Some("Terminal settings"))
        .unwrap();
    assert!(trigger.grab_focus());
    native::wait_for_frame(&trigger, "saved actual trigger focus", native::focus_within);
    let saved = gtk::prelude::RootExt::focus(main.widget()).unwrap();
    let id = match &request {
        InteractionRequest::HostKey(p) => p.id,
        InteractionRequest::Password(p) | InteractionRequest::PrivateKeyPassphrase(p) => p.id,
        InteractionRequest::KeyboardInteractive(p) => p.id,
    };
    if prime_empty {
        let prompts = native::descendants(&interaction(root))
            .into_iter()
            .find(|w| w.has_css_class("interaction-prompts"))
            .unwrap();
        assert!(prompts.first_child().is_none());
        // A real GTK getter establishes the legitimate empty-mode boundary.
        // In 4.16 it does not clear resize_needed; no private cache is modified.
        let mode = prompts.request_mode();
        eprintln!(
            "PROMPT_CACHE_NATIVE empty_mode={mode:?} gtk={}.{}.{} mapped={} allocation={}x{}",
            gtk::major_version(),
            gtk::minor_version(),
            gtk::micro_version(),
            prompts.is_mapped(),
            prompts.width(),
            prompts.height()
        );
        assert_eq!(mode, gtk::SizeRequestMode::ConstantSize);
    }
    let session = SessionId::new();
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionRequired {
        session,
        request,
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        native::wait_for_frame(
            main.widget(),
            "production prompt cache first-open",
            |root| native::modal_ready(root, "interaction-dialog") && outer_ready(root),
        );
    }));
    if let Err(panic) = result {
        failure::resume_after_failure(
            panic,
            || report::report(root),
            || native::close_window(main.widget()),
        );
    }
    assert!((root.width() - 800).abs() <= 2 && (root.height() - 600).abs() <= 2);
    assert!(interaction(root).compute_bounds(root).unwrap().width() <= 682.0);
    let target = first(root).unwrap();
    assert!(native::focus_within(&target));
    let b = target.compute_bounds(root).unwrap();
    let image = native::pixels(root);
    assert!(native::is_accent(
        image.at(b.x() as i32 + 1, (b.y() + b.height() / 2.0) as i32)
    ));
    let modal = interaction(root);
    if let Some(action) = action {
        if action == "Escape" {
            native::native_key(
                &modal,
                gtk::gdk::Key::Escape,
                gtk::gdk::ModifierType::empty(),
            );
        } else {
            native::button(&modal, action).emit_clicked();
        }
        native::wait_for_frame(&modal, "host response pending", |modal| {
            native::descendants(modal)
                .iter()
                .filter(|w| w.is::<gtk::Button>())
                .all(|w| !w.is_sensitive())
        });
    }
    main.emit(MainWindowMsg::AppEvent(AppEvent::InteractionResponded {
        session,
        interaction: id,
    }));
    let hidden = modal.clone();
    let restored = saved.clone();
    native::wait_for_frame(
        main.widget(),
        "exact acknowledgement restore",
        move |root| {
            !hidden.is_mapped()
                && gtk::prelude::RootExt::focus(&root.root().unwrap()).as_ref() == Some(&restored)
        },
    );
    assert!(trigger.is_sensitive());
    native::close_window(main.widget());
}

#[test]
fn production_prompt_cache_empty_then_populated_keeps_outer_input_reachable() {
    gtk::init().expect("native display required, no runtime skip");
    apply_global_css();
    let settings = gtk::Settings::default().unwrap();
    settings.set_property("gtk-enable-animations", false);
    settings.set_property("gtk-cursor-blink", false);
    for (changed, action) in [
        (false, "Reject"),
        (false, "Accept and store"),
        (true, "Close"),
        (false, "Escape"),
    ] {
        open_and_close(fixtures::host(changed), Some(action), false);
    }
    open_and_close(fixtures::auth_request("password"), None, true);
}
