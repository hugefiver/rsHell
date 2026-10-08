#![cfg(not(target_os = "macos"))]
#[path = "support/completion_bridge_native.rs"]
mod bridge;
#[path = "support/completion_focus_cases.rs"]
mod cases;
#[path = "support/terminal_focus_fixture.rs"]
mod terminal_focus_fixture;

#[tokio::test(flavor = "current_thread")]
async fn producer_completion_and_real_application_bridge_focus() {
    cases::run();
    bridge::run().await;
}
