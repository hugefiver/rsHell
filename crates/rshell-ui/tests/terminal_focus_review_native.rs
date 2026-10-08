#![cfg(not(target_os = "macos"))]
#[path = "support/terminal_focus_review.rs"]
mod review;
#[path = "support/terminal_focus_fixture.rs"]
mod terminal_focus_fixture;

#[test]
fn reviewed_focus_identity_and_creation_contracts_on_one_gtk_thread() {
    review::run();
}
