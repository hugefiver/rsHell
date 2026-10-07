#[cfg(target_os = "macos")]
#[path = "actor_panic_gtk_survival.rs"]
mod scenario;

#[cfg(target_os = "macos")]
fn main() {
    if cfg!(target_os = "macos") {
        println!(
            "ACTOR_PANIC_GTK_SURVIVAL_SKIP platform=macos reason=native_gui_integration_explicitly_skipped"
        );
        return;
    }
    scenario::run_actor_panic_scenario();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
