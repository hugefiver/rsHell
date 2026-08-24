#[cfg(target_os = "macos")]
#[path = "actor_panic_gtk_survival.rs"]
mod scenario;

#[cfg(target_os = "macos")]
fn main() {
    scenario::run_actor_panic_scenario();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
