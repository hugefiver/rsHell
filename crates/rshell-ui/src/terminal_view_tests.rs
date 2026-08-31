use super::*;

#[test]
fn clipboard_read_failure_maps_to_a_structured_error_without_fake_text() {
    let mapped = map_clipboard_read_result::<String, _>(Err("RAW-GDK-FAILURE"));

    assert_eq!(mapped, Err(TerminalViewError::ClipboardUnavailable));
    assert!(!format!("{mapped:?}").contains("RAW-GDK-FAILURE"));
    assert_eq!(
        map_clipboard_read_result::<String, &str>(Ok(Some("real text".into()))),
        Ok(Some("real text".into()))
    );
    assert_eq!(
        map_clipboard_read_result::<String, &str>(Ok(None)),
        Ok(None)
    );
}
