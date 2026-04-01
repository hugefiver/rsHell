use relm4::RelmApp;
use rshell::app::RshellApp;

fn main() {
    suppress_gio_warnings();
    RelmApp::new("io.github.hugefiver.rshell").run::<RshellApp>(());
}

fn suppress_gio_warnings() {
    unsafe {
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "disabled:");
        std::env::set_var("G_MESSAGES_DEBUG", "");
    }
}
