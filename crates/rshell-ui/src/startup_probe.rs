use std::{cell::RefCell, rc::Rc};

use relm4::gtk::prelude::ApplicationExt;
use rshell_core::{AppViewModel, PaneLaunchTarget, RenderFrame, SessionState};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupReport {
    pub window_realized: bool,
    pub local_session_connected: bool,
    pub non_empty_render_frame: bool,
    pub shutdown_clean: bool,
    pub embedded_css_loaded: bool,
    pub embedded_icons_renderable: bool,
    pub embedded_icon_backend: &'static str,
}

impl StartupReport {
    pub fn is_complete(self) -> bool {
        self.window_realized
            && self.local_session_connected
            && self.non_empty_render_frame
            && self.shutdown_clean
            && self.embedded_css_loaded
            && self.embedded_icons_renderable
            && matches!(self.embedded_icon_backend, "gtk_svg" | "internal_vector")
    }
}

#[derive(Debug, Default)]
struct ProbeState {
    window_realized: bool,
    local_session_connected: bool,
    non_empty_render_frame: bool,
    completion_notified: bool,
}

/// Observes the production GTK and application state used by startup smoke mode.
#[derive(Clone, Debug)]
pub struct StartupProbe {
    state: Rc<RefCell<ProbeState>>,
    on_complete: Option<fn()>,
}

impl StartupProbe {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(ProbeState::default())),
            on_complete: None,
        }
    }

    pub fn for_gtk() -> Self {
        Self {
            state: Rc::new(RefCell::new(ProbeState::default())),
            on_complete: Some(quit_gtk_application),
        }
    }

    pub fn observe_window_realized(&self) {
        self.update(|state| state.window_realized = true);
    }

    pub fn observe_local_session_state(&self, state: SessionState) {
        if state == SessionState::Connected {
            self.update(|probe| probe.local_session_connected = true);
        }
    }

    pub fn observe_view_model(&self, view_model: &AppViewModel) {
        let local_session_connected = view_model.workspace.tabs.iter().any(|tab| {
            let mut connected = false;
            tab.pane_tree.visit_leaves(&mut |pane, session| {
                if matches!(
                    view_model.pane_launches.get(&pane),
                    Some(PaneLaunchTarget::Local)
                ) && session
                    .and_then(|id| view_model.session_states.get(&id))
                    .is_some_and(|state| *state == SessionState::Connected)
                {
                    connected = true;
                }
            });
            connected
        });
        if local_session_connected {
            self.observe_local_session_state(SessionState::Connected);
        }
    }

    pub fn observe_render_frame(&self, frame: &RenderFrame) {
        if frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .any(|cell| !cell.text.is_empty())
        {
            self.update(|state| state.non_empty_render_frame = true);
        }
    }

    pub fn report(&self, shutdown_clean: bool) -> StartupReport {
        let state = self.state.borrow();
        StartupReport {
            window_realized: state.window_realized,
            local_session_connected: state.local_session_connected,
            non_empty_render_frame: state.non_empty_render_frame,
            shutdown_clean,
            embedded_css_loaded: !crate::embedded_theme_css().trim().is_empty(),
            embedded_icons_renderable: crate::embedded_icons_ready(),
            embedded_icon_backend: crate::ProductIcon::backend().as_str(),
        }
    }

    fn update(&self, update: impl FnOnce(&mut ProbeState)) {
        let notify = {
            let mut state = self.state.borrow_mut();
            update(&mut state);
            let complete = state.window_realized
                && state.local_session_connected
                && state.non_empty_render_frame;
            if complete && !state.completion_notified {
                state.completion_notified = true;
                true
            } else {
                false
            }
        };
        if notify && let Some(on_complete) = self.on_complete {
            on_complete();
        }
    }
}

impl Default for StartupProbe {
    fn default() -> Self {
        Self::new()
    }
}

fn quit_gtk_application() {
    relm4::main_application().quit();
}
