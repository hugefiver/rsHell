use std::{cell::RefCell, rc::Rc, sync::Arc};

use gtk::{gdk, prelude::*};
use relm4::ComponentSender;
use rshell_core::RenderFrame;

use crate::{
    FontMetricKey, TerminalDecorations, TerminalRenderCache, TerminalRenderer, TerminalView,
    TerminalViewInit, TerminalViewModel, TerminalViewMsg,
    terminal_view_keyboard::connect_keyboard,
    terminal_view_metrics::connect_metric_refresh,
    terminal_view_pointer::connect_pointer,
    visual_contract::{record_terminal_metrics, record_terminal_render_quality},
};

struct DrawSnapshot {
    frame: Option<Arc<RenderFrame>>,
    decorations: TerminalDecorations,
    metric_key: FontMetricKey,
    fallback_used: bool,
    renderer: TerminalRenderer,
    cache: TerminalRenderCache,
}

pub struct TerminalViewWidgets {
    pub root: gtk::Overlay,
    pub canvas: gtk::DrawingArea,
    search: gtk::SearchEntry,
    im_context: gtk::IMMulticontext,
    draw: Rc<RefCell<DrawSnapshot>>,
}

impl TerminalViewWidgets {
    pub fn build(
        root: &gtk::Overlay,
        init: &TerminalViewInit,
        sender: &ComponentSender<TerminalView>,
    ) -> Self {
        let canvas = gtk::DrawingArea::new();
        canvas.add_css_class("terminal-canvas");
        canvas.add_css_class("terminal-geometry-pending");
        canvas.set_hexpand(true);
        canvas.set_vexpand(true);
        canvas.set_focusable(true);
        canvas.update_property(&[
            gtk::accessible::Property::Label("Terminal"),
            gtk::accessible::Property::Description("Interactive terminal session"),
        ]);
        root.set_child(Some(&canvas));

        let search = gtk::SearchEntry::new();
        search.add_css_class("terminal-search");
        search.update_property(&[gtk::accessible::Property::Label("Search terminal output")]);
        search.set_halign(gtk::Align::End);
        search.set_valign(gtk::Align::Start);
        search.set_margin_top(8);
        search.set_margin_end(8);
        search.set_visible(false);
        root.add_overlay(&search);

        let draw = Rc::new(RefCell::new(DrawSnapshot {
            frame: None,
            decorations: TerminalDecorations::default(),
            metric_key: init.metrics.key.clone(),
            fallback_used: init.metrics.fallback_used,
            renderer: TerminalRenderer::from_measured(&init.profile, &init.metrics),
            cache: TerminalRenderCache::new(),
        }));
        record_terminal_metrics(&canvas, &init.metrics);
        let draw_callback = Rc::clone(&draw);
        canvas.set_draw_func(move |canvas, context, width, height| {
            let mut snapshot = draw_callback.borrow_mut();
            let DrawSnapshot {
                frame,
                decorations,
                renderer,
                cache,
                ..
            } = &mut *snapshot;
            if let Some(frame) = frame
                && let Ok(stats) = cache.update(
                    renderer,
                    Arc::clone(frame),
                    decorations,
                    width,
                    height,
                    canvas.scale_factor(),
                )
            {
                record_terminal_render_quality(canvas, &stats);
                let _ = cache.paint(context);
            }
        });

        let im_context = gtk::IMMulticontext::new();
        let realize_im = im_context.clone();
        canvas.connect_realize(move |canvas| realize_im.set_client_widget(Some(canvas)));
        let unrealize_im = im_context.clone();
        canvas.connect_unrealize(move |_| {
            unrealize_im.set_client_widget(gtk::Widget::NONE);
        });
        connect_keyboard(
            &canvas,
            &search,
            &im_context,
            &init.profile.key_bindings,
            sender,
        );
        connect_pointer(&canvas, sender);
        connect_search(&search, sender);
        connect_resize(&canvas, sender);
        connect_metric_refresh(&canvas, sender);

        Self {
            root: root.clone(),
            canvas,
            search,
            im_context,
            draw,
        }
    }

    pub fn sync(&self, model: &TerminalViewModel) {
        if model.has_positive_emitted_geometry() {
            self.canvas.remove_css_class("terminal-geometry-pending");
        }
        let decorations = TerminalDecorations::new(
            model.search_matches().to_vec(),
            model.current_search_index(),
        );
        let frame = model.frame().cloned();
        let mut draw = self.draw.borrow_mut();
        let measured = model.measured_metrics();
        record_terminal_metrics(&self.canvas, measured);
        let metrics_changed = draw.metric_key != measured.key
            || draw.fallback_used != measured.fallback_used
            || draw.renderer.metrics() != measured.metrics;
        if metrics_changed {
            draw.metric_key = measured.key.clone();
            draw.fallback_used = measured.fallback_used;
            draw.renderer = TerminalRenderer::from_measured(&model.profile, measured);
            draw.cache.invalidate_metrics();
        }
        let changed = metrics_changed
            || draw.frame.as_ref().map(|value| value.generation)
                != frame.as_ref().map(|value| value.generation)
            || draw.decorations != decorations;
        if changed {
            draw.frame = frame;
            draw.decorations = decorations;
            drop(draw);
            self.canvas.queue_draw();
        }
        let search_open = model.search_is_open();
        if self.search.is_visible() != search_open {
            self.search.set_visible(search_open);
            if search_open {
                self.search.grab_focus();
            } else {
                self.canvas.grab_focus();
            }
        }
        if let Some(rect) = model.cursor_rect() {
            self.im_context.set_cursor_location(&gdk::Rectangle::new(
                rect.x.round() as i32,
                rect.y.round() as i32,
                rect.width.round() as i32,
                rect.height.round() as i32,
            ));
        }
    }
}

fn connect_search(search: &gtk::SearchEntry, sender: &ComponentSender<TerminalView>) {
    let sender = sender.clone();
    search.connect_search_changed(move |entry| {
        sender.input(TerminalViewMsg::Search {
            text: entry.text().into(),
            case_sensitive: false,
            regex: false,
        });
    });
}

fn connect_resize(canvas: &gtk::DrawingArea, sender: &ComponentSender<TerminalView>) {
    let input = sender.input_sender().clone();
    canvas.connect_map(move |canvas| {
        if input.send(TerminalViewMsg::GeometryMapped).is_err() {
            return;
        }
        let _ = send_resize(canvas, canvas.width(), canvas.height(), &input);
    });
    let input = sender.input_sender().clone();
    canvas.connect_unmap(move |_| {
        let _ = input.send(TerminalViewMsg::GeometryUnmapped);
    });
    let input = sender.input_sender().clone();
    canvas.connect_resize(move |canvas, width, height| {
        let _ = send_resize(canvas, width, height, &input);
    });
}

fn send_resize(
    canvas: &gtk::DrawingArea,
    width: i32,
    height: i32,
    input: &relm4::Sender<TerminalViewMsg>,
) -> bool {
    if width > 0 && height > 0 {
        input
            .send(TerminalViewMsg::Resize {
                width,
                height,
                scale: f64::from(canvas.scale_factor()),
            })
            .is_ok()
    } else {
        false
    }
}
