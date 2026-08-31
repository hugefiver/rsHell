use std::{cell::RefCell, rc::Rc};

use relm4::gtk::prelude::ApplicationExt;
use rshell_core::{AppViewModel, PaneLaunchTarget, RenderFrame, SessionState, TerminalSize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupReport {
    pub window_realized: bool,
    pub local_session_connected: bool,
    pub non_empty_render_frame: bool,
    pub shutdown_clean: bool,
    pub embedded_css_loaded: bool,
    pub embedded_icons_renderable: bool,
    pub embedded_icon_backend: &'static str,
    pub measured_terminal_geometry_ready: bool,
    pub scale_aware_icons_ready: bool,
    pub icon_backend: &'static str,
    pub icon_count: usize,
    pub adaptive_layout_modes: usize,
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
            && self.measured_terminal_geometry_ready
            && self.scale_aware_icons_ready
            && matches!(self.icon_backend, "gtk_svg" | "internal_vector")
            && self.icon_backend == self.embedded_icon_backend
            && self.icon_count == crate::ProductIcon::ALL.len()
            && self.adaptive_layout_modes == adaptive_layout_modes()
    }
}

#[derive(Debug, Default)]
struct ProbeState {
    window_realized: bool,
    local_session_connected: bool,
    non_empty_render_frame: bool,
    measured_terminal_geometry_ready: bool,
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
        let non_empty_render_frame = frame
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .any(|cell| !cell.text.is_empty());
        self.update(|state| state.non_empty_render_frame |= non_empty_render_frame);
        self.observe_terminal_geometry(frame.size);
    }

    pub fn observe_terminal_geometry(&self, size: TerminalSize) {
        if terminal_geometry_ready(size) {
            self.update(|state| state.measured_terminal_geometry_ready = true);
        }
    }

    pub fn report(&self, shutdown_clean: bool) -> StartupReport {
        let state = self.state.borrow();
        let icon_backend = crate::ProductIcon::backend().as_str();
        StartupReport {
            window_realized: state.window_realized,
            local_session_connected: state.local_session_connected,
            non_empty_render_frame: state.non_empty_render_frame,
            shutdown_clean,
            embedded_css_loaded: !crate::embedded_theme_css().trim().is_empty(),
            embedded_icons_renderable: crate::embedded_icons_ready(crate::IconRenderRequest {
                logical_size: 16,
                effective_scale: 1.0,
            }),
            embedded_icon_backend: icon_backend,
            measured_terminal_geometry_ready: state.measured_terminal_geometry_ready,
            scale_aware_icons_ready: scale_aware_icons_ready(),
            icon_backend,
            icon_count: crate::ProductIcon::ALL.len(),
            adaptive_layout_modes: adaptive_layout_modes(),
        }
    }

    fn update(&self, update: impl FnOnce(&mut ProbeState)) {
        let notify = {
            let mut state = self.state.borrow_mut();
            update(&mut state);
            let complete = state.window_realized
                && state.local_session_connected
                && state.non_empty_render_frame
                && state.measured_terminal_geometry_ready;
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

fn scale_aware_icons_ready() -> bool {
    [1.0, 1.25, 1.5, 2.0].into_iter().all(|effective_scale| {
        crate::embedded_icons_ready(crate::IconRenderRequest {
            logical_size: 16,
            effective_scale,
        })
    })
}

const fn terminal_geometry_ready(size: TerminalSize) -> bool {
    size.cols > 0 && size.rows > 0 && size.pixel_width > 0 && size.pixel_height > 0 && size.dpi > 0
}

const fn adaptive_layout_modes() -> usize {
    [
        crate::ShellLayoutMode::Compact,
        crate::ShellLayoutMode::Standard,
        crate::ShellLayoutMode::Wide,
    ]
    .len()
}
