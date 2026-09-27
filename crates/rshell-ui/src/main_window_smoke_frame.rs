use std::{
    cell::Cell,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use gtk::prelude::*;
use relm4::{ComponentSender, gtk};

use crate::{
    MainWindow, MainWindowMsg, main_window_smoke::queue_visual_completion_tick,
    main_window_smoke_visual::VisualCheckpointPhase, smoke_driver_state::SmokeDriver,
};

pub(crate) fn checkpoint_trace_enabled() -> bool {
    std::env::var_os("CI").is_some()
        || std::env::var("RSHELL_CHECKPOINT_TRACE").as_deref() == Ok("1")
}

// Only the named native checkpoint scenario calls this with enabled=true.
pub(crate) fn checkpoint_trace(enabled: bool, event: &str, detail: impl std::fmt::Display) {
    static LINES: AtomicUsize = AtomicUsize::new(0);
    if enabled && LINES.fetch_add(1, Ordering::Relaxed) < 64 {
        eprintln!("CHECKPOINT_TRACE at={:?} {event} {detail}", Instant::now());
    }
}

pub(crate) fn schedule_after_frame(
    widget: &impl IsA<gtk::Widget>,
    sender: relm4::Sender<MainWindowMsg>,
    trace: bool,
) {
    let pending = Rc::new(Cell::new(true));
    let tick_pending = Rc::clone(&pending);
    let tick_sender = sender.clone();
    widget.add_tick_callback(move |_, _| {
        let should_send = tick_pending.replace(false);
        checkpoint_trace(trace, "frame_fire", format_args!("pending={should_send}"));
        if should_send {
            let sent = tick_sender.send(MainWindowMsg::SmokeTick).is_ok();
            checkpoint_trace(trace, "frame_send", format_args!("sent={sent}"));
        }
        gtk::glib::ControlFlow::Break
    });
    gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || {
        let should_send = pending.replace(false);
        checkpoint_trace(
            trace,
            "fallback_fire",
            format_args!("pending={should_send}"),
        );
        if should_send {
            let sent = sender.send(MainWindowMsg::SmokeTick).is_ok();
            checkpoint_trace(trace, "fallback_send", format_args!("sent={sent}"));
        }
    });
}

impl MainWindow {
    pub(crate) fn trace_smoke_tick_consumed(&mut self) {
        if std::mem::take(&mut self.smoke_state.trace_next_tick) {
            checkpoint_trace(
                true,
                "tick_consume",
                format_args!("pending={}", self.smoke_tick_pending),
            );
        }
    }

    pub(crate) fn schedule_smoke_tick(&mut self, sender: &ComponentSender<Self>, trace: bool) {
        if self.smoke_tick_pending || !self.smoke.as_ref().is_some_and(SmokeDriver::is_active) {
            checkpoint_trace(
                trace,
                "schedule_skip",
                format_args!("pending={}", self.smoke_tick_pending),
            );
            return;
        }
        self.smoke_tick_pending = true;
        if queue_visual_completion_tick(
            &mut self.smoke_state.visual_completion_tick_pending,
            |message| sender.input(message),
        ) {
            checkpoint_trace(trace, "schedule_visual_tick", "pending=true sent=true");
            return;
        }
        let sender = sender.input_sender().clone();
        if self.smoke_state.window_resize.is_some_and(|evidence| {
            evidence.realized_width == 0
                || evidence.realized_height == 0
                || evidence.layout != evidence.expected_layout
        }) {
            checkpoint_trace(trace, "schedule_frame", "pending=true reason=window_resize");
            schedule_after_frame(&self.shell.overlay, sender, trace);
            return;
        }
        if self.smoke_state.visual_checkpoint == VisualCheckpointPhase::Opening
            && self.smoke_state.visual_paintable.is_some()
        {
            checkpoint_trace(
                trace,
                "schedule_frame",
                "pending=true reason=visual_opening",
            );
            schedule_after_frame(&self.shell.overlay, sender, trace);
            return;
        }
        checkpoint_trace(trace, "schedule_timer", "pending=true delay_ms=25");
        gtk::glib::timeout_add_local_full(
            Duration::from_millis(25),
            gtk::glib::Priority::DEFAULT,
            move || {
                checkpoint_trace(trace, "timer_fire", "pending=true");
                let sent = sender.send(MainWindowMsg::SmokeTick).is_ok();
                checkpoint_trace(trace, "timer_send", format_args!("sent={sent}"));
                gtk::glib::ControlFlow::Break
            },
        );
    }
}
