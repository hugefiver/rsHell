use std::{cell::RefCell, rc::Rc, time::Instant};

use gtk::prelude::*;
use relm4::gtk;

use crate::ShellLayoutMode;

#[cfg(any(target_os = "linux", test))]
#[path = "resize_trace_observer.rs"]
mod observer;
#[cfg(any(target_os = "linux", test))]
use observer::Observer;

const DETAIL_LIMIT: u64 = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Before,
    NotifyWidth,
    NotifyHeight,
    #[cfg(any(target_os = "linux", test))]
    Map,
    #[cfg(any(target_os = "linux", test))]
    Unmap,
    AfterRoute,
    Allocation,
    RefreshEnter,
    RefreshExit,
    TickEnter,
    TickExit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    pub default: Option<(i32, i32)>,
    pub widget: Option<(i32, i32)>,
    pub surface: Option<(i32, i32, i32)>,
    pub widget_scale: Option<i32>,
    pub mapped: Option<bool>,
    pub realized: Option<bool>,
    pub mode: Option<ShellLayoutMode>,
}

impl Snapshot {
    pub fn read(window: Option<&gtk::ApplicationWindow>, mode: Option<ShellLayoutMode>) -> Self {
        let Some(window) = window else {
            return Self {
                default: None,
                widget: None,
                surface: None,
                widget_scale: None,
                mapped: None,
                realized: None,
                mode,
            };
        };
        Self {
            default: Some(window.default_size()),
            widget: Some((window.width(), window.height())),
            surface: window
                .surface()
                .map(|surface| (surface.width(), surface.height(), surface.scale_factor())),
            widget_scale: Some(window.scale_factor()),
            mapped: Some(window.is_mapped()),
            realized: Some(window.is_realized()),
            mode,
        }
    }
}

pub(super) struct Ledger {
    requested: (i32, i32),
    expected: ShellLayoutMode,
    previous: Option<Snapshot>,
    last_periodic: [Option<Snapshot>; 4],
    total: u64,
    details: u64,
    dropped: u64,
    repeated: u64,
    notify_width: bool,
    notify_height: bool,
    first_observed_default_change: Option<(u64, u128)>,
    ended: bool,
}

impl Ledger {
    pub fn new(requested: (i32, i32), expected: ShellLayoutMode) -> Self {
        Self {
            requested,
            expected,
            previous: None,
            last_periodic: [None; 4],
            total: 0,
            details: 0,
            dropped: 0,
            repeated: 0,
            notify_width: false,
            notify_height: false,
            first_observed_default_change: None,
            ended: false,
        }
    }

    pub fn record(&mut self, event: Event, ms: u128, snapshot: Snapshot) -> Option<String> {
        if self.ended {
            return None;
        }
        self.total = self.total.saturating_add(1);
        self.notify_width |= event == Event::NotifyWidth;
        self.notify_height |= event == Event::NotifyHeight;
        let old = self.previous.and_then(|sample| sample.default);
        if self.first_observed_default_change.is_none()
            && old.is_some()
            && snapshot.default.is_some()
            && old != snapshot.default
        {
            self.first_observed_default_change = Some((self.total, ms));
        }
        let periodic = match event {
            Event::RefreshEnter => Some(0),
            Event::RefreshExit => Some(1),
            Event::TickEnter => Some(2),
            Event::TickExit => Some(3),
            _ => None,
        };
        let repeat = periodic.is_some_and(|index| {
            self.last_periodic[index] == Some(snapshot) && self.previous == Some(snapshot)
        });
        if let Some(index) = periodic {
            self.last_periodic[index] = Some(snapshot);
        }
        self.previous = Some(snapshot);
        if repeat {
            self.repeated = self.repeated.saturating_add(1);
            return None;
        }
        if self.details >= DETAIL_LIMIT {
            self.dropped = self.dropped.saturating_add(1);
            return None;
        }
        self.details += 1;
        Some(format!(
            "P0_RESIZE_TRACE event={event:?} seq={} elapsed_ms={ms} requested={:?} previous_observed_default={old:?} default={:?} widget={:?} surface_wh_scale={:?} widget_scale={:?} mapped={:?} realized={:?} mode={:?} expected={:?}",
            self.total,
            self.requested,
            snapshot.default,
            snapshot.widget,
            snapshot.surface,
            snapshot.widget_scale,
            snapshot.mapped,
            snapshot.realized,
            snapshot.mode,
            self.expected
        ))
    }

    pub fn terminal(
        &mut self,
        ms: u128,
        snapshot: Snapshot,
        phase: &'static str,
    ) -> Option<String> {
        if self.ended {
            return None;
        }
        self.ended = true;
        let old = self.previous.and_then(|sample| sample.default);
        if self.first_observed_default_change.is_none()
            && old.is_some()
            && snapshot.default.is_some()
            && old != snapshot.default
        {
            self.first_observed_default_change = Some((self.total + 1, ms));
        }
        Some(format!(
            "P0_RESIZE_TRACE event=terminal phase={phase} seq={} elapsed_ms={ms} requested={:?} previous_observed_default={old:?} default={:?} widget={:?} surface_wh_scale={:?} widget_scale={:?} mapped={:?} realized={:?} mode={:?} expected={:?} total={} detailed={} dropped={} repeated={} notify_width_seen={} notify_height_seen={} first_observed_default_change_seq_ms={:?}",
            self.total + 1,
            self.requested,
            snapshot.default,
            snapshot.widget,
            snapshot.surface,
            snapshot.widget_scale,
            snapshot.mapped,
            snapshot.realized,
            snapshot.mode,
            self.expected,
            self.total + 1,
            self.details,
            self.dropped,
            self.repeated,
            self.notify_width,
            self.notify_height,
            self.first_observed_default_change
        ))
    }
}

pub(super) struct ResizeTrace {
    ledger: Rc<RefCell<Ledger>>,
    started: Instant,
    #[cfg(any(target_os = "linux", test))]
    observer: Option<Observer>,
}

impl ResizeTrace {
    pub fn new(requested: (i32, i32), expected: ShellLayoutMode) -> Self {
        Self {
            ledger: Rc::new(RefCell::new(Ledger::new(requested, expected))),
            started: Instant::now(),
            #[cfg(any(target_os = "linux", test))]
            observer: None,
        }
    }

    pub fn record(&self, event: Event, snapshot: Snapshot) {
        let output =
            self.ledger
                .borrow_mut()
                .record(event, self.started.elapsed().as_millis(), snapshot);
        if let Some(output) = output {
            eprintln!("{output}");
        }
    }

    pub fn terminal(&mut self, phase: &'static str, snapshot: Snapshot) {
        #[cfg(any(target_os = "linux", test))]
        self.observer.take();
        let output =
            self.ledger
                .borrow_mut()
                .terminal(self.started.elapsed().as_millis(), snapshot, phase);
        if let Some(output) = output {
            eprintln!("{output}");
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub fn attach(&mut self, window: &gtk::ApplicationWindow) {
        self.observer = Some(Observer::attach(window, &self.ledger, self.started));
    }
}

#[cfg(test)]
#[path = "resize_trace_tests.rs"]
mod tests;
