use gtk::prelude::*;
use relm4::gtk;

pub(crate) fn request_layout_frame(widget: &impl IsA<gtk::Widget>) {
    widget.queue_resize();
    widget.queue_allocate();
    widget.queue_draw();
    if let Some(layout) = widget.layout_manager() {
        layout.layout_changed();
    }

    let Some(frame_clock) = widget.frame_clock() else {
        return;
    };
    frame_clock.request_phase(
        gtk::gdk::FrameClockPhase::UPDATE
            | gtk::gdk::FrameClockPhase::LAYOUT
            | gtk::gdk::FrameClockPhase::PAINT,
    );
}
