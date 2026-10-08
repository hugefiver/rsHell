use super::*;
use rshell_core::{ApplicationHandle, SessionState, UiCommand};

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Healthy,
    Foreign,
    Sidebar,
    Search,
    Pane,
    AwayBack,
    Modal,
    TabChanged,
    Close,
    Drop,
}
impl Case {
    pub const ALL: &[Self] = &[
        Self::Healthy,
        Self::Foreign,
        Self::Sidebar,
        Self::Search,
        Self::Pane,
        Self::AwayBack,
        Self::Modal,
        Self::TabChanged,
        Self::Close,
        Self::Drop,
    ];
}

pub(super) fn anchor(window: &FocusWindow) -> gtk::Widget {
    css(window.root(), "pane-host").first_child().unwrap()
}
pub(super) async fn painted(window: &FocusWindow) {
    let clock = window.root().frame_clock().unwrap();
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let flag = done.clone();
    let id = clock.connect_after_paint(move |_| flag.set(true));
    clock.request_phase(gtk::gdk::FrameClockPhase::AFTER_PAINT);
    until(|| done.get(), "startup cancellation actual paint").await;
    clock.disconnect(id);
}
async fn ready(window: &FocusWindow, app: &ApplicationHandle, initial: rshell_core::SessionId) {
    until(
        || {
            let view = app.view_model();
            view.session_states.get(&initial) == Some(&SessionState::Connected)
                && view.latest_frames.contains_key(&initial)
                && find_css(window.root(), "terminal-canvas")
                    .is_some_and(|c| c.is_mapped() && c.width() > 0 && c.height() > 0)
        },
        "initial Connected release reached actual mapped canvas",
    )
    .await;
    painted(window).await;
}

pub(super) async fn run(window: FocusWindow, app: &ApplicationHandle, ports: &Ports, case: Case) {
    let initial = app
        .initial_view_model()
        .workspace
        .active_tab()
        .unwrap()
        .pane_tree
        .session_ids()[0];
    assert_eq!(
        app.view_model().session_states.get(&initial),
        Some(&SessionState::Created)
    );
    assert!(!app.view_model().latest_frames.contains_key(&initial));
    let anchor = anchor(&window);
    until(
        || anchor.is_mapped() && focused(window.root()).as_ref() == Some(&anchor),
        "initial Pending constructor anchor, no canvas assistance",
    )
    .await;
    assert!(anchor.is_focusable());
    assert!(find_css(window.root(), "terminal-canvas").is_none());
    let foreign = button(window.root(), "Terminal settings");
    let mut expected = None;
    let mut pane_tooltip = None;
    match case {
        Case::Healthy => {}
        Case::Foreign | Case::AwayBack => {
            assert!(foreign.grab_focus());
            expected = focused(window.root());
        }
        Case::Sidebar => {
            assert!(button(window.root(), "Create a connection").grab_focus());
            expected = focused(window.root());
        }
        Case::Search => {
            let search = css(window.root(), "connection-search");
            assert!(search.grab_focus());
            expected = focused(window.root());
            assert!(expected.as_ref().is_some_and(|w| w.is::<gtk::Text>()));
        }
        Case::Pane => {
            let action = descendants(&anchor)
                .into_iter()
                .find(|w| {
                    w.is::<gtk::Button>()
                        && !w.is::<gtk::ToggleButton>()
                        && w.has_css_class("pane-action-btn")
                        && w.is_mapped()
                        && w.is_sensitive()
                        && w.width() > 0
                })
                .expect("actual Pending pane action");
            pane_tooltip = action.tooltip_text();
            assert!(action.grab_focus());
            expected = focused(window.root());
            assert!(expected.as_ref().is_some_and(|w| w.is_ancestor(&anchor)));
        }
        Case::Modal => {
            foreign.emit_clicked();
            until(|| !anchor.is_sensitive(), "native Settings modal open").await;
            expected = focused(window.root());
        }
        Case::TabChanged => {
            app.ui_port().try_send(UiCommand::NewLocalTab).unwrap();
            until(
                || ports.state.lock().unwrap().attempts == 2,
                "plain Core B started",
            )
            .await;
            ports.release.try_send(true).unwrap();
            until(
                || app.view_model().workspace.tabs.len() == 2 && !anchor.is_focusable(),
                "snapshot selection cannot authorize B or retain initial A grant",
            )
            .await;
        }
        Case::Close => {
            window.root().close();
            drain();
        }
        Case::Drop => {
            drop(window);
            assert!(
                !anchor.is_focusable(),
                "Drop restores original anchor property"
            );
            ports.ready_initial();
            until(
                || app.view_model().session_states.get(&initial) == Some(&SessionState::Connected),
                "producer can finish after window Drop",
            )
            .await;
            eprintln!("STARTUP_GATED case=Drop restored=true window_unmapped=true");
            return;
        }
    }
    if !matches!(case, Case::Healthy) {
        assert!(
            !anchor.is_focusable(),
            "departure cancels and restores anchor synchronously"
        );
    }
    if matches!(case, Case::AwayBack) {
        // Cancellation restored false already. Temporarily enable only to arrange a return
        // to the original source; it must not revive the consumed constructor generation.
        anchor.set_focusable(true);
        gtk::prelude::GtkWindowExt::set_focus(window.root(), Some(&anchor));
        assert_eq!(focused(window.root()).as_ref(), Some(&anchor));
        expected = Some(anchor.clone());
    }
    ports.ready_initial();
    if matches!(case, Case::Close) {
        until(
            || app.view_model().session_states.get(&initial) == Some(&SessionState::Connected),
            "producer completes after unmap",
        )
        .await;
        assert!(!window.root().is_mapped());
        assert!(!anchor.is_focusable());
        return;
    }
    ready(&window, app, initial).await;
    if matches!(case, Case::Healthy) {
        startup::observe(&window, app, ports).await;
        assert!(!anchor.is_focusable(), "consume restores original false");
        assert!(foreign.grab_focus());
        expected = focused(window.root());
    } else {
        assert_ne!(
            focused(window.root()).as_ref(),
            Some(&window.canvas()),
            "no cancelled startup canvas grant"
        );
        if let Some(tooltip) = &pane_tooltip {
            until(
                || {
                    focused(window.root())
                        .is_some_and(|w| w.tooltip_text().as_ref() == Some(tooltip))
                },
                "owned pane action restoration never substitutes canvas",
            )
            .await;
            expected = focused(window.root());
        } else if expected.is_some() {
            assert_eq!(
                focused(window.root()),
                expected,
                "exact foreign/search/modal/returned-anchor focus retained"
            );
        }
        assert!(ports.state.lock().unwrap().inputs.is_empty());
    }
    ports.later_initial_frame();
    until(
        || {
            app.view_model()
                .latest_frames
                .get(&initial)
                .is_some_and(|f| f.generation == 2)
        },
        "later initial producer frame",
    )
    .await;
    painted(&window).await;
    if expected.is_some() {
        assert_eq!(
            focused(window.root()),
            expected,
            "later frame never reauthorizes startup"
        );
    }
    assert_ne!(focused(window.root()).as_ref(), Some(&window.canvas()));
    if matches!(case, Case::AwayBack) {
        anchor.set_focusable(false);
    }
    assert!(!anchor.is_focusable());
    eprintln!(
        "STARTUP_GATED case={case:?} exact_initial=true restored=true no_frame_steal=true scale={} physical=false",
        window.root().scale_factor()
    );
}
