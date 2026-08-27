use std::{collections::BTreeSet, sync::Arc};

use rshell_core::{
    CellPosition, Color, CursorShape, KeyCode, KeyModifiers, MouseButton, MouseEventKind,
    RenderFrame, ResolvedTerminalProfile, SearchQuery, SelectionRange, TerminalInput,
    TerminalMouseEvent, TerminalOverrides, TerminalSettingsV1, TerminalSize, Viewport,
};
use rshell_session::{DefaultTerminalEngine, TerminalEngine};
use sha2::{Digest, Sha256};

const FIXTURE: &[u8] = include_bytes!("fixtures/compatibility.ansi");
const CANARY_FIXTURE: &str = include_str!("fixtures/vt/canary.json");

#[test]
fn compatibility_fixture_is_not_rewritten_by_platform_line_endings() {
    assert!(FIXTURE.ends_with(b"\x1b[?25l\n"));
    assert!(
        !FIXTURE.windows(2).any(|bytes| bytes == b"\r\n"),
        "terminal control fixture must retain repository LF bytes"
    );
}

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
fn indexed_rgb_colors_and_configured_answerback_use_public_adapter_state() {
    let mut engine = engine_with(|settings| settings.answerback = "rshell-ready".into());
    let delta = engine.advance(b"\x1b[38;5;123;48;2;1;2;3mX\x05").unwrap();
    assert_eq!(delta.outbound, b"rshell-ready");

    let frame = engine.snapshot(viewport(0, 3), None);
    let cell = frame
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .find(|cell| cell.text == "X")
        .unwrap();
    assert_eq!(cell.foreground, Color::Ansi(123));
    assert_eq!(cell.background, Color::Rgb(1, 2, 3));
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
fn same_chunk_primary_output_before_alternate_screen_keeps_stable_row_ids() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    engine.input(b"anchor\r\n").unwrap();
    let before = engine.search(&SearchQuery {
        needle: "anchor".into(),
        case_sensitive: true,
        regex: false,
    })[0];

    engine
        .input(b"one\r\ntwo\r\nthree\r\n\x1b[?1049hALT")
        .unwrap();
    engine.input(b"\x1b[?1049l").unwrap();
    let after = engine.search(&SearchQuery {
        needle: "anchor".into(),
        case_sensitive: true,
        regex: false,
    })[0];

    assert_eq!(after.start.stable_row, before.start.stable_row);
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
fn long_output_exposes_clamped_viewport_bounds() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(24, 3)).unwrap();
    let output = (0..20)
        .map(|index| format!("record-{index:02}\r\n"))
        .collect::<String>();
    engine.input(output.as_bytes()).unwrap();

    let bounds = engine.viewport_bounds();
    assert!(bounds.first_stable_row <= bounds.bottom_top_stable_row);

    let bottom = engine.snapshot(viewport(bounds.bottom_top_stable_row, 3), None);
    assert_eq!(bottom.viewport_top, bounds.bottom_top_stable_row);
    assert!(frame_text(&bottom).contains("record-19"));

    let clamped = engine.snapshot(viewport(i64::MAX, 3), None);
    assert_eq!(clamped.viewport_top, bounds.bottom_top_stable_row);
    let oldest = engine.snapshot(viewport(i64::MIN, 3), None);
    assert_eq!(oldest.viewport_top, bounds.first_stable_row);
}

#[test]
fn stable_row_ids_survive_output_after_scrollback_reaches_its_limit() {
    let mut engine = DefaultTerminalEngine::new(&profile(100), size(24, 3)).unwrap();
    let first = (0..150)
        .map(|index| format!("record-{index:03}\r\n"))
        .collect::<String>();
    engine.input(first.as_bytes()).unwrap();
    let before = engine.search(&SearchQuery {
        needle: "record-100".into(),
        case_sensitive: true,
        regex: false,
    })[0];

    let additional = (150..160)
        .map(|index| format!("record-{index:03}\r\n"))
        .collect::<String>();
    engine.input(additional.as_bytes()).unwrap();
    let after = engine.search(&SearchQuery {
        needle: "record-100".into(),
        case_sensitive: true,
        regex: false,
    })[0];

    assert_eq!(after.start.stable_row, before.start.stable_row);
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
fn every_p0_key_and_supported_modifier_bit_uses_the_pinned_encoder() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    let mut keys = vec![
        KeyCode::Character('a'),
        KeyCode::Enter,
        KeyCode::Escape,
        KeyCode::Tab,
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
    ];
    keys.extend((1..=24).map(KeyCode::F));
    for code in keys {
        assert!(
            !engine
                .encode_input(key(code.clone(), KeyModifiers::default()))
                .unwrap_or_else(|error| panic!("{code:?} failed: {error}"))
                .is_empty(),
            "{code:?} encoded to no bytes"
        );
    }

    for modifiers in [
        KeyModifiers {
            shift: true,
            ..KeyModifiers::default()
        },
        control(),
        KeyModifiers {
            alt: true,
            ..KeyModifiers::default()
        },
        KeyModifiers {
            shift: true,
            control: true,
            alt: true,
            ..KeyModifiers::default()
        },
    ] {
        assert!(
            !engine
                .encode_input(key(KeyCode::Character('a'), modifiers))
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(
        engine
            .encode_input(TerminalInput::CommittedText("秘密".to_owned()))
            .unwrap(),
        "秘密".as_bytes()
    );
}

#[test]
fn keyboard_modes_follow_terminal_state_and_profile() {
    let mut default_engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    assert_eq!(
        default_engine
            .encode_input(key(KeyCode::ArrowUp, KeyModifiers::default()))
            .unwrap(),
        b"\x1b[A"
    );
    default_engine.advance(b"\x1b[?1h").unwrap();
    assert_eq!(
        default_engine
            .encode_input(key(KeyCode::ArrowUp, KeyModifiers::default()))
            .unwrap(),
        b"\x1bOA"
    );
    assert_eq!(
        DefaultTerminalEngine::new(&profile(1_000), size(20, 3))
            .unwrap()
            .encode_input(key(KeyCode::Character('['), control()))
            .unwrap(),
        b"\x1b"
    );
    assert_eq!(
        engine_with(|settings| settings.enable_csi_u = true)
            .encode_input(key(KeyCode::Character('['), control()))
            .unwrap(),
        b"\x1b[91;5u"
    );

    let mut kitty = engine_with(|settings| settings.enable_kitty_keyboard = true);
    kitty.advance(b"\x1b[>1u").unwrap();
    assert_eq!(kitty.advance(b"\x1b[?u").unwrap().outbound, b"\x1b[?1u");
    assert_eq!(
        kitty
            .encode_input(key(KeyCode::Character('a'), KeyModifiers::default()))
            .unwrap(),
        b"a"
    );
    assert_eq!(
        kitty.encode_input(key(KeyCode::Tab, control())).unwrap(),
        b"\x1b[9;5u"
    );
    assert_eq!(
        DefaultTerminalEngine::new(&profile(1_000), size(20, 3))
            .unwrap()
            .encode_input(key(
                KeyCode::Character('x'),
                KeyModifiers {
                    alt: true,
                    ..KeyModifiers::default()
                },
            ))
            .unwrap(),
        b"\x1bx"
    );
}

#[test]
fn configured_mouse_policy_can_disable_dynamic_reporting() {
    let mut allowed = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    assert!(!allowed.snapshot(viewport(0, 3), None).mouse_reporting);
    assert!(allowed.encode_mouse(left_press()).is_err());

    let mut disabled = engine_with(|settings| settings.mouse_reporting = false);
    disabled.advance(b"\x1b[?1002h\x1b[?1006h").unwrap();
    assert!(!disabled.snapshot(viewport(0, 3), None).mouse_reporting);
    assert!(disabled.encode_mouse(left_press()).is_err());
}

#[test]
fn mouse_sgr_requires_tracking_and_sgr_negotiation() {
    let mut engine = DefaultTerminalEngine::new(&profile(1_000), size(20, 3)).unwrap();
    engine.input(b"\x1b[?1002h").unwrap();
    let tracking_only = engine.encode_mouse(left_press()).unwrap();
    assert!(!tracking_only.starts_with(b"\x1b[<"));

    engine.input(b"\x1b[?1006h").unwrap();
    assert_eq!(engine.encode_mouse(left_press()).unwrap(), b"\x1b[<0;4;2M");
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> TerminalInput {
    TerminalInput::Key { code, modifiers }
}

fn control() -> KeyModifiers {
    KeyModifiers {
        control: true,
        ..KeyModifiers::default()
    }
}

fn engine_with(change: impl FnOnce(&mut TerminalSettingsV1)) -> DefaultTerminalEngine {
    let mut settings = TerminalSettingsV1::default();
    change(&mut settings);
    DefaultTerminalEngine::new(
        &settings.resolve(&TerminalOverrides::default()),
        size(20, 3),
    )
    .unwrap()
}

fn left_press() -> TerminalMouseEvent {
    TerminalMouseEvent {
        kind: MouseEventKind::Press,
        button: Some(MouseButton::Left),
        cell: CellPosition {
            stable_row: 101,
            column: 3,
        },
        viewport_row: 1,
        pixel_x: 24,
        pixel_y: 32,
        modifiers: KeyModifiers::default(),
    }
}

#[test]
fn terminal_engine_canary_verifies_exact_crlf_rows_before_candidate_hashing() {
    const ROWS: usize = 1000;
    const VIEWPORT_ROWS: u16 = 40;
    let expected = (0..ROWS)
        .map(|index| format!("scrollback-{index:04}"))
        .collect::<Vec<_>>();
    let mut input = Vec::new();
    for label in &expected {
        input.extend_from_slice(label.as_bytes());
        input.extend_from_slice(b"\r\n");
    }
    assert!(input.ends_with(b"\r\n"));
    assert_eq!(
        input.windows(2).filter(|bytes| *bytes == b"\r\n").count(),
        ROWS
    );
    assert!(
        input
            .iter()
            .enumerate()
            .all(|(index, byte)| *byte != b'\n' || input.get(index.wrapping_sub(1)) == Some(&b'\r'))
    );

    let mut engine = DefaultTerminalEngine::new(&profile(2_000), size(120, VIEWPORT_ROWS)).unwrap();
    engine.input(&input).unwrap();
    let bounds = engine.viewport_bounds();
    let mut top = bounds.first_stable_row;
    let mut seen = BTreeSet::new();
    let mut rendered = Vec::with_capacity(ROWS + 1);
    let cursor_row = loop {
        let frame = engine.snapshot(viewport(top, VIEWPORT_ROWS), None);
        assert_eq!(frame.rows.len(), usize::from(VIEWPORT_ROWS));
        for row in frame.rows.iter() {
            if seen.insert(row.stable_row) {
                let raw = row
                    .cells
                    .iter()
                    .map(|cell| cell.text.as_str())
                    .collect::<String>();
                let text = if raw.bytes().all(|byte| byte == b' ') {
                    String::new()
                } else {
                    raw.trim_end_matches(' ').to_owned()
                };
                rendered.push((row.stable_row, text));
            }
        }
        if top == bounds.bottom_top_stable_row {
            break frame.cursor.unwrap().position.stable_row;
        }
        top = top
            .saturating_add(i64::from(VIEWPORT_ROWS))
            .min(bounds.bottom_top_stable_row);
    };

    assert_eq!(rendered.len(), ROWS + 1);
    let trailing = rendered.pop().unwrap();
    assert_eq!(trailing.0, cursor_row);
    assert!(trailing.1.is_empty());
    let actual = rendered
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "rendered row equality is the hash oracle");

    let expected_canonical = expected.join("\n").into_bytes();
    let actual_canonical = actual.join("\n").into_bytes();
    assert!(!actual_canonical.ends_with(b"\n"));
    assert_eq!(actual_canonical, expected_canonical);
    let candidate = format!("{:x}", Sha256::digest(&actual_canonical));
    assert_eq!(
        candidate,
        format!("{:x}", Sha256::digest(&expected_canonical))
    );
    assert_eq!(candidate.len(), 64);
    assert!(
        candidate
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let fixture_digest = CANARY_FIXTURE
        .lines()
        .find_map(|line| line.trim().strip_prefix("\"sha256\":"))
        .unwrap()
        .trim()
        .trim_end_matches(',');
    if fixture_digest != "null" {
        assert_eq!(fixture_digest.trim_matches('"'), candidate);
    }
}

#[test]
fn snapshots_are_arc_backed_immutable_values() {
    let engine = DefaultTerminalEngine::new(&profile(1_000), size(10, 2)).unwrap();
    let frame: Arc<RenderFrame> = engine.snapshot(viewport(0, 2), None);
    assert_eq!(Arc::strong_count(&frame), 1);
}
