use std::{cell::Cell, rc::Rc, time::Duration};

use gtk::prelude::*;
use relm4::gtk;

use crate::MainWindowMsg;

pub(crate) fn schedule_after_frame(
    widget: &impl IsA<gtk::Widget>,
    sender: relm4::Sender<MainWindowMsg>,
) {
    let pending = Rc::new(Cell::new(true));
    let tick_pending = Rc::clone(&pending);
    let tick_sender = sender.clone();
    widget.add_tick_callback(move |_, _| {
        if tick_pending.replace(false) {
            let _ = tick_sender.send(MainWindowMsg::SmokeTick);
        }
        gtk::glib::ControlFlow::Break
    });
    gtk::glib::timeout_add_local_once(Duration::from_millis(100), move || {
        if pending.replace(false) {
            let _ = sender.send(MainWindowMsg::SmokeTick);
        }
    });
}
