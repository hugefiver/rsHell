use gtk::glib;
use gtk::prelude::*;
use relm4::prelude::*;

struct MinimalApp {
    counter: u32,
}

#[derive(Debug)]
enum Msg {
    Increment,
    Tick,
}

#[relm4::component]
impl SimpleComponent for MinimalApp {
    type Init = ();
    type Input = Msg;
    type Output = ();

    view! {
        gtk::Window {
            set_title: Some("Minimal Test"),
            set_default_size: (400, 300),

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 10,
                set_margin_all: 20,

                gtk::Label {
                    #[watch]
                    set_label: &format!("Counter: {}", model.counter),
                },

                gtk::Button {
                    set_label: "Increment",
                    connect_clicked => Msg::Increment,
                },
            }
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        println!("init: starting");

        let model = MinimalApp { counter: 0 };
        let widgets = view_output!();

        // Start timer
        glib::timeout_add_local(std::time::Duration::from_secs(1), {
            let sender = sender.clone();
            move || {
                println!("tick!");
                sender.input(Msg::Tick);
                glib::ControlFlow::Continue
            }
        });

        println!("init: done");
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            Msg::Increment => {
                self.counter += 1;
                println!("incremented to {}", self.counter);
            }
            Msg::Tick => {
                println!("tick received");
            }
        }
    }
}

fn main() {
    println!("main: starting");
    let app = RelmApp::new("io.test.minimal");
    println!("main: running");
    app.run::<MinimalApp>(());
    println!("main: exited");
}
