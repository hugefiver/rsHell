use gtk::{gdk::prelude::TextureExtManual, prelude::*};
use relm4::{Component, ComponentController};
use std::time::{Duration, Instant};

pub fn launch(width: i32, height: i32) -> (relm4::Controller<rshell_ui::MainWindow>, OwnedSurface) {
    struct Port;
    impl rshell_core::UiCommandPort for Port {
        fn try_send(&self, _: rshell_core::UiCommand) -> Result<(), rshell_core::UiPortError> {
            Ok(())
        }
    }
    let view = rshell_core::AppViewModel::from(rshell_core::AppBootstrapState {
        catalog: Default::default(),
        settings: Default::default(),
        terminal_profiles: vec![rshell_core::TerminalProfile::default()],
    });
    let main = rshell_ui::MainWindow::builder()
        .launch(rshell_ui::MainWindowInit::new(
            std::sync::Arc::new(Port),
            view,
        ))
        .detach();
    let owned = OwnedSurface::new(main.widget());
    main.widget().set_default_size(width, height);
    main.widget().present();
    let class = if width < 900 {
        "shell-compact"
    } else if width < 1440 {
        "shell-standard"
    } else {
        "shell-wide"
    };
    super::fluent_native::wait_for_frame(main.widget(), "owned native shell", |root| {
        descendants(root).iter().any(|w| w.has_css_class(class))
    });
    assert!(
        descendants(main.widget())
            .iter()
            .any(|w| w.has_css_class(class))
    );
    println!(
        "TAB_ALLOCATION requested={width}x{height} realized={}x{} mode={class} scale={}",
        main.widget().width(),
        main.widget().height(),
        main.widget().scale_factor()
    );
    (main, owned)
}

pub struct OwnedSurface {
    pub window: gtk::Window,
    display: gtk::gdk::Display,
    provider: gtk::CssProvider,
}

impl OwnedSurface {
    pub fn new(window: &impl IsA<gtk::Window>) -> Self {
        let display = gtk::gdk::Display::default().expect("native display required");
        let provider = gtk::CssProvider::new();
        provider.load_from_data(include_str!("../../../../resources/style.css"));
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        Self {
            window: window.as_ref().clone(),
            display,
            provider,
        }
    }
}

impl Drop for OwnedSurface {
    fn drop(&mut self) {
        self.window.close();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.window.is_mapped() && Instant::now() < deadline {
            gtk::glib::MainContext::default().iteration(false);
        }
        gtk::style_context_remove_provider_for_display(&self.display, &self.provider);
        eprintln!(
            "TAB_CLEANUP unmapped={} provider_removed=true",
            !self.window.is_mapped()
        );
        assert!(!self.window.is_mapped(), "owned window must unmap");
    }
}

pub fn descendants(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Widget> {
    let mut result = Vec::new();
    let mut child = root.as_ref().first_child();
    while let Some(widget) = child {
        result.push(widget.clone());
        result.extend(descendants(&widget));
        child = widget.next_sibling();
    }
    result
}

pub fn button(root: &impl IsA<gtk::Widget>, tooltip: &str) -> gtk::Button {
    descendants(root)
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::Button>().ok())
        .find(|w| w.tooltip_text().as_deref() == Some(tooltip))
        .unwrap()
}

pub fn active_group(root: &impl IsA<gtk::Widget>) -> gtk::Widget {
    descendants(root)
        .into_iter()
        .find(|w| {
            w.has_css_class("terminal-tab")
                && descendants(w)
                    .iter()
                    .any(|c| c.has_css_class("tab-button") && c.has_css_class("active-tab"))
        })
        .expect("active native group")
}

pub fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        for _ in 0..128 {
            if !gtk::glib::MainContext::default().iteration(false) {
                break;
            }
        }
        if ready() {
            return;
        }
        assert!(Instant::now() < deadline, "native state did not settle");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn texture(widget: &impl IsA<gtk::Widget>) -> gtk::gdk::Texture {
    let widget = widget.as_ref();
    let (width, height) = (widget.allocation().width(), widget.allocation().height());
    assert!(widget.is_mapped() && width > 0 && height > 0);
    let snapshot = gtk::Snapshot::new();
    gtk::WidgetPaintable::new(Some(widget)).snapshot(
        &snapshot,
        f64::from(width),
        f64::from(height),
    );
    let node = snapshot.to_node().expect("native paint node");
    let renderer = gtk::gsk::CairoRenderer::new();
    renderer.realize(None).unwrap();
    let rect = gtk::graphene::Rect::new(0., 0., width as f32, height as f32);
    let texture = renderer.render_texture(&node, Some(&rect));
    renderer.unrealize();
    texture
}

pub fn pixels(widget: &impl IsA<gtk::Widget>) -> (Vec<u8>, usize, usize) {
    let texture = texture(widget);
    let (width, height) = (texture.width() as usize, texture.height() as usize);
    let mut native = vec![0; width * height * 4];
    texture.download(&mut native, width * 4);
    (
        rshell_ui::argb32_native_to_rgba(&native, rshell_ui::NativeByteOrder::current()).unwrap(),
        width,
        height,
    )
}

pub fn accent(pixel: &[u8]) -> bool {
    super::fluent_native::is_accent(pixel.try_into().unwrap())
}

pub fn capture(widget: &impl IsA<gtk::Widget>, name: &str) {
    if let Some(dir) = std::env::var_os("RSHELL_TAB_GROUP_SNAPSHOT_DIR") {
        let path = std::path::PathBuf::from(dir).join(format!("{name}.png"));
        assert!(path.parent().unwrap().is_dir());
        if path.exists() {
            println!("TAB_SCREENSHOT_RETAINED {}", path.display());
            return;
        }
        texture(widget).save_to_png(&path).unwrap();
        println!("TAB_SCREENSHOT {}", path.display());
    }
}
