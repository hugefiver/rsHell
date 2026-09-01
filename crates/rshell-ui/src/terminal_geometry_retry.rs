use std::{cell::Cell, rc::Rc};

use gtk::prelude::*;
use relm4::ComponentSender;

use super::{TerminalView, TerminalViewMsg};

#[derive(Clone, Copy)]
struct RetryState {
    mapped: bool,
    acknowledged: bool,
    callback_armed: bool,
    channel_open: bool,
}

impl Default for RetryState {
    fn default() -> Self {
        Self {
            mapped: false,
            acknowledged: false,
            callback_armed: false,
            channel_open: true,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct TerminalGeometryRetry {
    state: Rc<Cell<RetryState>>,
}

impl TerminalGeometryRetry {
    pub(crate) fn mapped(&self) {
        self.update(|state| state.mapped = true);
    }

    pub(crate) fn unmapped(&self) {
        self.update(|state| state.mapped = false);
    }

    pub(crate) fn pending(&self) {
        self.update(|state| state.acknowledged = false);
    }

    pub(crate) fn acknowledged(&self) {
        self.update(|state| state.acknowledged = true);
    }

    pub(crate) fn output_failed(&self) {
        self.update(|state| state.channel_open = false);
    }

    pub(crate) fn arm_callback(&self) -> bool {
        let mut state = self.state.get();
        if !state.mapped || state.acknowledged || state.callback_armed || !state.channel_open {
            return false;
        }
        state.callback_armed = true;
        self.state.set(state);
        true
    }

    pub(crate) fn callback_fired(&self, mapped: bool) -> bool {
        let mut state = self.state.get();
        state.callback_armed = false;
        state.mapped = mapped;
        let refresh = state.mapped && !state.acknowledged && state.channel_open;
        self.state.set(state);
        refresh
    }

    fn update(&self, change: impl FnOnce(&mut RetryState)) {
        let mut state = self.state.get();
        change(&mut state);
        self.state.set(state);
    }
}

impl TerminalView {
    pub(super) fn finish_geometry_attempt(
        &self,
        output_open: bool,
        sender: &ComponentSender<Self>,
    ) {
        if !output_open {
            self.geometry_retry.output_failed();
            return;
        }
        if !self.model.has_positive_emitted_geometry() {
            self.geometry_retry.pending();
            self.schedule_geometry_retry(sender);
        }
    }

    pub(super) fn schedule_geometry_retry(&self, sender: &ComponentSender<Self>) {
        if !self.geometry_retry.arm_callback() {
            return;
        }
        let retry = self.geometry_retry.clone();
        let input = sender.input_sender().clone();
        let _ = self.metric_widget.add_tick_callback(move |canvas, _| {
            if retry.callback_fired(canvas.is_mapped())
                && input.send(TerminalViewMsg::RefreshGeometry).is_err()
            {
                retry.output_failed();
            }
            gtk::glib::ControlFlow::Break
        });
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalGeometryRetry;

    #[test]
    fn one_callback_rearms_until_typed_acknowledgement() {
        let retry = TerminalGeometryRetry::default();
        retry.mapped();
        retry.pending();

        assert!(retry.arm_callback());
        assert!(!retry.arm_callback());
        assert!(retry.callback_fired(true));
        assert!(retry.arm_callback());

        retry.acknowledged();
        assert!(!retry.callback_fired(true));
        assert!(!retry.arm_callback());
    }

    #[test]
    fn queued_callback_after_acknowledgement_emits_nothing() {
        let retry = TerminalGeometryRetry::default();
        retry.mapped();
        retry.pending();
        assert!(retry.arm_callback());

        retry.acknowledged();

        assert!(!retry.callback_fired(true));
        assert!(!retry.arm_callback());
    }

    #[test]
    fn unmap_and_output_failure_stop_safely() {
        let retry = TerminalGeometryRetry::default();
        retry.mapped();
        retry.pending();
        assert!(retry.arm_callback());

        retry.unmapped();
        assert!(!retry.callback_fired(false));
        assert!(!retry.arm_callback());

        retry.mapped();
        assert!(retry.arm_callback());
        retry.output_failed();
        assert!(!retry.callback_fired(true));
        assert!(!retry.arm_callback());
    }
}
