use super::*;

pub(super) struct Observer {
    window: gtk::glib::WeakRef<gtk::ApplicationWindow>,
    handlers: Vec<gtk::glib::SignalHandlerId>,
}

impl Observer {
    pub(super) fn attach(
        window: &gtk::ApplicationWindow,
        ledger: &Rc<RefCell<Ledger>>,
        started: Instant,
    ) -> Self {
        let weak_window = window.downgrade();
        let mut handlers = Vec::with_capacity(4);
        for (property, event) in [
            ("default-width", Event::NotifyWidth),
            ("default-height", Event::NotifyHeight),
        ] {
            let weak_ledger = Rc::downgrade(ledger);
            let weak_window = weak_window.clone();
            handlers.push(window.connect_notify_local(Some(property), move |_, _| {
                Self::emit(&weak_window, &weak_ledger, started, event);
            }));
        }
        for (event, connect) in [(Event::Map, true), (Event::Unmap, false)] {
            let weak_ledger = Rc::downgrade(ledger);
            let weak_window = weak_window.clone();
            handlers.push(if connect {
                window.connect_map(move |_| Self::emit(&weak_window, &weak_ledger, started, event))
            } else {
                window
                    .connect_unmap(move |_| Self::emit(&weak_window, &weak_ledger, started, event))
            });
        }
        Self {
            window: window.downgrade(),
            handlers,
        }
    }

    fn emit(
        window: &gtk::glib::WeakRef<gtk::ApplicationWindow>,
        ledger: &std::rc::Weak<RefCell<Ledger>>,
        started: Instant,
        event: Event,
    ) {
        let Some(window) = window.upgrade() else {
            return;
        };
        let snapshot = Snapshot::read(Some(&window), None);
        let Some(ledger) = ledger.upgrade() else {
            return;
        };
        let output = ledger
            .borrow_mut()
            .record(event, started.elapsed().as_millis(), snapshot);
        if let Some(output) = output {
            eprintln!("{output}");
        }
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(window) = self.window.upgrade() {
            for handler in self.handlers.drain(..) {
                window.disconnect(handler);
            }
        }
    }
}
