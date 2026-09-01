use std::{fs, path::Path};

#[test]
fn smoke_close_all_routes_one_immediate_shutdown() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_window_smoke_close.rs"),
    )
    .expect("read smoke close route");
    let route = &source[source
        .find("pub(crate) fn route_smoke_close_all")
        .expect("smoke close route must exist")..];

    assert!(route.contains("UiCommand::Shutdown"));
    assert!(!route.contains("SessionTabBarMsg::Close"));
    assert!(!route.contains("close_all_last_tabs"));
}
