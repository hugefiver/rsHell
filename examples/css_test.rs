use gtk::prelude::*;
use relm4::prelude::*;

struct CssTestApp;

#[derive(Debug)]
enum Msg {}

#[relm4::component]
impl SimpleComponent for CssTestApp {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        gtk::Window {
            set_title: Some("CSS Test"),
            set_default_size: (500, 400),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 20,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    add_css_class: "blue-box",
                    gtk::Label { set_label: "Box with .blue-box CSS" },
                },

                gtk::Label {
                    set_label: "Label with .red-label CSS background",
                    add_css_class: "red-label",
                },

                gtk::Button {
                    set_label: "Button with .green-button CSS",
                    add_css_class: "green-button",
                },

                gtk::DrawingArea {
                    set_size_request: (200, 40),
                    set_draw_func: |_da, cr, width, height| {
                        cr.set_source_rgb(1.0, 0.5, 0.0);
                        cr.rectangle(0.0, 0.0, width as f64, height as f64);
                        let _ = cr.fill();
                        cr.set_source_rgb(1.0, 1.0, 1.0);
                        cr.move_to(10.0, 25.0);
                        let _ = cr.show_text("DrawingArea Cairo orange");
                    },
                },

                gtk::Label {
                    set_label: "Plain label - default style",
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let provider = gtk::CssProvider::new();
        provider.load_from_data(
            r#"
            .blue-box { background-color: #0078d4; min-height: 30px; color: #fff; }
            .red-label { background-color: #ff0000; color: #ffffff; padding: 8px; }
            .green-button { background-color: #00ff00; background-image: none; color: #000; border: none; }
            window { background-color: #1e1e1e; color: #cccccc; }
            "#,
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let model = CssTestApp;
        let widgets = view_output!();
        eprintln!("CSS test init done");
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}

fn main() {
    eprintln!("Starting CSS test");
    let app = RelmApp::new("io.test.csstest");
    eprintln!("Running CSS test");
    app.run::<CssTestApp>(());
}
