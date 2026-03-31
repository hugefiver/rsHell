use gtk::{gdk, CssProvider};

pub fn apply_global_css() {
    let display = gdk::Display::default().expect("GTK display is not available");

    let settings = gtk::Settings::for_display(&display);
    settings.set_gtk_application_prefer_dark_theme(true);

    let provider = CssProvider::new();
    provider.load_from_data(include_str!("../resources/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
