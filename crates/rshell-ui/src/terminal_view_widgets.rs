use std::{cell::RefCell, rc::Rc, sync::Arc};

use gtk::{gdk, prelude::*};
use relm4::ComponentSender;
use rshell_core::RenderFrame;

use crate::{
    TerminalDecorations, TerminalRenderCache, TerminalRenderer, TerminalView, TerminalViewInit,
    TerminalViewModel, TerminalViewMsg, terminal_view_keyboard::connect_keyboard,
    terminal_view_pointer::connect_pointer,
};

struct DrawSnapshot {
    frame: Option<Arc<RenderFrame>>,
    decorations: TerminalDecorations,
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
            renderer: TerminalRenderer::new(&init.profile, init.metrics),
            cache: TerminalRenderCache::new(),
        }));
        let draw_callback = Rc::clone(&draw);
        canvas.set_draw_func(move |canvas, context, width, height| {
            let mut snapshot = draw_callback.borrow_mut();
            let DrawSnapshot {
                frame,
                decorations,
                renderer,
                cache,
            } = &mut *snapshot;
            if let Some(frame) = frame
                && cache
                    .update(
                        renderer,
                        Arc::clone(frame),
                        decorations,
                        width,
                        height,
                        canvas.scale_factor(),
                    )
                    .is_ok()
            {
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

        Self {
            root: root.clone(),
            canvas,
            search,
            im_context,
            draw,
        }
    }

    pub fn sync(&self, model: &TerminalViewModel) {
        let decorations = TerminalDecorations::new(
            model.search_matches().to_vec(),
            model.current_search_index(),
        );
        let frame = model.frame().cloned();
        let mut draw = self.draw.borrow_mut();
        let changed = draw.frame.as_ref().map(|value| value.generation)
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
    let sender = sender.clone();
    canvas.connect_resize(move |canvas, width, height| {
        sender.input(TerminalViewMsg::Resize {
            width,
            height,
            scale: f64::from(canvas.scale_factor()),
        });
    });
}
