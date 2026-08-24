use std::{sync::Arc, time::Instant};

use rshell_core::{
    CellPosition, Color, CursorShape, KeyCode, KeyModifiers, MouseButton, MouseEventKind,
    RenderFrame, ResolvedTerminalProfile, SearchQuery, SelectionRange, TerminalInput,
    TerminalMouseEvent, TerminalOverrides, TerminalSettingsV1, TerminalSize, Viewport,
};
use rshell_session::{DefaultTerminalEngine, TerminalEngine};

const FIXTURE: &[u8] = include_bytes!("fixtures/compatibility.ansi");

fn size(cols: u16, rows: u16) -> TerminalSize {
    TerminalSize {
        cols,
        rows,
        pixel_width: u32::from(cols) * 8,
        pixel_height: u32::from(rows) * 16,
        dpi: 96,
    }
}

fn profile(scrollback_lines: usize) -> ResolvedTerminalProfile {
    TerminalSettingsV1 {
        scrollback_lines,
        ..TerminalSettingsV1::default()
    }
    .resolve(&TerminalOverrides::default())
}

fn viewport(top_stable_row: i64, rows: u16) -> Viewport {
    Viewport {
        top_stable_row,
        rows,
    }
}

fn frame_text(frame: &RenderFrame) -> String {
    frame
        .rows
        .iter()
        .map(|row| row.cells.iter().map(|cell| cell.text.as_str()).collect())
        .collect::<Vec<String>>()
        .join("\n")
}

#[test]
fn fixture_converts_styles_unicode_wrap_title_cursor_and_mouse_mode() {
    let mut engine = DefaultTerminalEngine::new(&profile(20_000), size(16, 8)).unwrap();
    engine.input(FIXTURE).unwrap();

    let frame = engine.snapshot(viewport(0, 8), None);
    let styled = frame
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .find(|cell| cell.text.contains('S'))
        .expect("styled cell");
    assert_eq!(styled.foreground, Color::Ansi(1));
    assert_eq!(styled.background, Color::Ansi(4));
    assert!(styled.attributes.bold);
    assert!(styled.attributes.italic);
    assert!(styled.attributes.underline);
    assert!(styled.attributes.strike);
    assert!(styled.attributes.reverse);

    let text = frame_text(&frame);
    assert!(text.contains('界'));
    assert!(text.contains("é"), "combining mark must stay with its cell");
    assert!(frame.rows.iter().any(|row| row.wrapped));
    assert_eq!(frame.title, "rsHell contract title");
    assert!(frame.mouse_reporting);
    assert!(!frame.alternate_screen);

    let cursor = frame
        .cursor
        .expect("cursor state is represented even when hidden");
    assert_eq!(cursor.position.column, 3);
    assert_eq!(cursor.position.stable_row, 3);
    assert_eq!(cursor.shape, CursorShape::Beam);
    assert!(!cursor.visible);
}

#[test]
fn alternate_screen_is_isolated_and_restores_primary_screen() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 4)).unwrap();
    engine.input(b"primary").unwrap();
    engine.input(b"\x1b[?1049hALT-SCREEN\x1b[?1002h").unwrap();

    let alternate = engine.snapshot(viewport(0, 4), None);
    assert!(alternate.alternate_screen);
    assert!(alternate.mouse_reporting);
    assert!(frame_text(&alternate).contains("ALT-SCREEN"));
    assert!(!frame_text(&alternate).contains("primary"));

    engine.input(b"\x1b[?1049l").unwrap();
    let primary = engine.snapshot(viewport(0, 4), None);
    assert!(!primary.alternate_screen);
    assert!(frame_text(&primary).contains("primary"));
    assert!(!frame_text(&primary).contains("ALT-SCREEN"));

    engine.input(b"\x1b[?1002l").unwrap();
    assert!(!engine.snapshot(viewport(0, 4), None).mouse_reporting);
}

#[test]
fn scrollback_selection_and_all_search_modes_use_stable_rows() {
    let mut engine = DefaultTerminalEngine::new(&profile(20_000), size(32, 5)).unwrap();
    let mut stream = Vec::new();
    for line in 0..10_050 {
        stream.extend_from_slice(format!("record-{line:05} Alpha beta\r\n").as_bytes());
    }
    engine.input(&stream).unwrap();

    let plain = engine.search(&SearchQuery {
        needle: "record-00001".into(),
        case_sensitive: true,
        regex: false,
    });
    assert_eq!(
        plain.len(),
        1,
        "oldest retained scrollback remains searchable"
    );
    assert!(
        engine
            .search(&SearchQuery {
                needle: "alpha".into(),
                case_sensitive: false,
                regex: false,
            })
            .len()
            >= 10_000
    );
    assert!(
        engine
            .search(&SearchQuery {
                needle: r"record-1004[0-9] Alpha".into(),
                case_sensitive: true,
                regex: true,
            })
            .len()
            >= 10
    );
    assert!(
        engine
            .search(&SearchQuery {
                needle: "alpha".into(),
                case_sensitive: true,
                regex: false,
            })
            .is_empty()
    );

    let first = plain[0];
    let historical = engine.snapshot(viewport(first.start.stable_row, 1), None);
    assert_eq!(historical.rows[0].stable_row, first.start.stable_row);
    assert!(frame_text(&historical).contains("record-00001"));

    let second = engine.search(&SearchQuery {
        needle: "record-10049".into(),
        case_sensitive: true,
        regex: false,
    })[0];
    let selected = engine.selection_text(SelectionRange {
        start: first.start,
        end: second.end,
        rectangular: false,
    });
    assert!(selected.starts_with("record-00001"));
    assert!(selected.contains("record-05000 Alpha beta"));
    assert!(selected.ends_with("record-10049"));
}

#[test]
fn cursor_shape_canary_maps_fixed_revision_variants() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();

    engine.input(b"\x1b[3 q").unwrap();
    let underline = engine.snapshot(viewport(0, 3), None).cursor.unwrap();
    assert_eq!(underline.shape, CursorShape::Underline);
    assert!(underline.visible);

    engine.input(b"\x1b[1 q").unwrap();
    let block = engine.snapshot(viewport(0, 3), None).cursor.unwrap();
    assert_eq!(block.shape, CursorShape::Block);

    engine.input(b"\x1b[5 q").unwrap();
    let beam = engine.snapshot(viewport(0, 3), None).cursor.unwrap();
    assert_eq!(beam.shape, CursorShape::Beam);
}

#[test]
fn resize_preserves_content_and_updates_frame_geometry() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(12, 3)).unwrap();
    engine.input(b"preserved-content").unwrap();
    engine.resize(size(24, 6)).unwrap();

    let frame = engine.snapshot(viewport(0, 6), None);
    assert_eq!(frame.size, size(24, 6));
    assert!(frame_text(&frame).contains("preserved-content"));
}

#[test]
fn clear_scrollback_keeps_visible_content_and_reset_restores_defaults() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    engine
        .input(b"old-line\r\nkeep-1\r\nkeep-2\r\nvisible")
        .unwrap();
    assert!(
        !engine
            .search(&SearchQuery {
                needle: "old-line".into(),
                case_sensitive: true,
                regex: false,
            })
            .is_empty()
    );

    engine.clear_scrollback();
    assert!(
        engine
            .search(&SearchQuery {
                needle: "old-line".into(),
                case_sensitive: true,
                regex: false,
            })
            .is_empty()
    );
    assert!(frame_text(&engine.snapshot(viewport(0, 3), None)).contains("visible"));

    engine
        .input(b"\x1b]2;changed\x07\x1b[?1003h\x1b[?1049hdirty")
        .unwrap();
    engine.reset();
    let reset = engine.snapshot(viewport(0, 3), None);
    assert_eq!(reset.title, "rsHell");
    assert!(!reset.mouse_reporting);
    assert!(!reset.alternate_screen);
    assert!(!frame_text(&reset).contains("dirty"));
}

#[test]
fn snapshot_selection_overlay_does_not_mutate_engine_state() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    engine.input(b"select me").unwrap();
    let range = SelectionRange {
        start: CellPosition {
            stable_row: 0,
            column: 0,
        },
        end: CellPosition {
            stable_row: 0,
            column: 5,
        },
        rectangular: false,
    };
    let selected = engine.snapshot(viewport(0, 3), Some(range));
    let plain = engine.snapshot(viewport(0, 3), None);
    assert_ne!(selected, plain);
    assert_eq!(plain, engine.snapshot(viewport(0, 3), None));
}

#[test]
fn wide_midpoint_selection_normalizes_to_the_stable_wide_cell_and_frame_overlay() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(8, 1)).unwrap();
    engine.input("A界B".as_bytes()).unwrap();
    let range = SelectionRange {
        start: CellPosition {
            stable_row: 0,
            column: 2,
        },
        end: CellPosition {
            stable_row: 0,
            column: 3,
        },
        rectangular: false,
    };

    assert_eq!(engine.selection_text(range), "界");
    let frame = engine.snapshot(viewport(0, 1), Some(range));
    let wide = frame.rows[0]
        .cells
        .iter()
        .find(|cell| cell.text == "界")
        .expect("wide cell");
    assert_eq!(wide.width, 2);
    assert!(wide.selected);
}

#[test]
fn rejects_zero_sized_terminal() {
    let error = DefaultTerminalEngine::new(&profile(1_000), size(0, 3)).unwrap_err();
    assert!(error.to_string().contains("non-zero"));
}

#[test]
fn unsupported_input_is_an_error_instead_of_a_silent_drop() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    let error = engine
        .encode_input(TerminalInput::Key {
            code: KeyCode::Character('x'),
            modifiers: KeyModifiers {
                super_key: true,
                ..KeyModifiers::default()
            },
        })
        .unwrap_err();

    assert!(error.to_string().contains("unsupported"));
}

#[test]
fn reporting_mode_encodes_viewport_relative_press_and_wheel_rows_for_transport() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    engine.input(b"\x1b[?1002h").unwrap();
    let event = TerminalMouseEvent {
        kind: MouseEventKind::Press,
        button: Some(MouseButton::Left),
        cell: CellPosition {
            stable_row: 101,
            column: 3,
        },
        viewport_row: 1,
        pixel_x: 24,
        pixel_y: 32,
        modifiers: KeyModifiers {
            shift: true,
            ..KeyModifiers::default()
        },
    };
    let encoded = engine.encode_mouse(event).unwrap();

    assert_eq!(encoded, b"\x1b[<4;4;2M");
    assert_eq!(
        engine
            .encode_mouse(TerminalMouseEvent {
                kind: MouseEventKind::Scroll,
                button: Some(MouseButton::WheelUp),
                ..event
            })
            .unwrap(),
        b"\x1b[<68;4;2M"
    );
    assert_eq!(
        engine
            .encode_mouse(TerminalMouseEvent {
                kind: MouseEventKind::Scroll,
                button: Some(MouseButton::WheelDown),
                ..event
            })
            .unwrap(),
        b"\x1b[<69;4;2M"
    );
}

#[test]
#[ignore = "release-only evidence gate; run with --release --ignored --nocapture"]
fn throughput_gate() {
    const MIN_BYTES: usize = 50 * 1024 * 1024;
    const MIN_MIB_PER_SECOND: f64 = 20.0;
    let mut engine = DefaultTerminalEngine::new(&profile(20_000), size(120, 36)).unwrap();
    let record =
        b"plain throughput payload 0123456789 abcdefghijklmnopqrstuvwxyz\x1b[31mRED\x1b[0m\r\n";
    let chunk = record.repeat((64 * 1024 / record.len()) + 1);
    let started = Instant::now();
    let mut bytes = 0;
    while bytes < MIN_BYTES {
        engine.input(&chunk).unwrap();
        bytes += chunk.len();
    }
    let elapsed = started.elapsed();
    let rate = bytes as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64();
    println!(
        "throughput: bytes={bytes} elapsed={elapsed:?} rate={rate:.2} MiB/s backend=wezterm-term"
    );
    assert!(bytes >= MIN_BYTES);
    assert!(
        rate >= MIN_MIB_PER_SECOND,
        "{rate:.2} MiB/s is below {MIN_MIB_PER_SECOND:.2} MiB/s"
    );
}

#[test]
fn snapshots_are_arc_backed_immutable_values() {
    let engine = DefaultTerminalEngine::new(&profile(1_000), size(10, 2)).unwrap();
    let frame: Arc<RenderFrame> = engine.snapshot(viewport(0, 2), None);
    assert_eq!(Arc::strong_count(&frame), 1);
}
