use std::sync::Arc;

use gtk::gdk::{Key, ModifierType};
use rshell_core::{
    CellAttributes, CellPosition, Color, CursorShape, MouseButton, MouseEventKind, RenderCell,
    RenderCursor, RenderFrame, RenderRow, SearchMatch, SessionId, SessionUiCommand, SessionUiEvent,
    TerminalSize, UiCommand,
};
use rshell_ui::{FontMetrics, PointerEvent, TerminalClipboardAction, TerminalViewModel, ViewRect};

#[test]
fn stale_and_equal_frames_are_dropped_and_dirty_rows_track_stable_content() {
    let mut model = model();
    let first = frame(5, false, &["same", "before"], Some((1, 0)));
    let accepted = model.apply_frame(first);
    assert!(accepted.accepted());
    assert_eq!(accepted.dirty_rows(), &[0, 1]);

    assert!(
        !model
            .apply_frame(frame(4, false, &["stale"], None))
            .accepted()
    );
    assert!(
        !model
            .apply_frame(frame(5, false, &["equal"], None))
            .accepted()
    );
    let update = model.apply_frame(frame(6, false, &["same", "after"], Some((1, 0))));
    assert_eq!(update.dirty_rows(), &[1]);
    assert_eq!(model.frame().unwrap().generation, 6);
}

#[test]
fn cursor_only_move_repaints_both_old_and_new_cursor_rows() {
    let mut model = model();
    model.apply_frame(frame(1, false, &["a", "b", "c"], Some((0, 0))));

    let update = model.apply_frame(frame(2, false, &["a", "b", "c"], Some((2, 0))));

    assert_eq!(update.dirty_rows(), &[0, 2]);
}

#[test]
fn disappearing_and_shifted_viewport_rows_are_all_repainted() {
    let mut model = model();
    model.apply_frame(offset_rows(frame(1, false, &["a", "b", "c"], None), 100));

    let update = model.apply_frame(offset_rows(frame(2, false, &["b", "c"], None), 101));

    assert_eq!(update.dirty_rows(), &[0, 1, 2]);
}

#[test]
fn cursor_width_uses_the_wide_cell_under_the_cursor() {
    let mut model = model();
    model.apply_frame(wide_frame(7));

    assert_eq!(
        model.cursor_rect(),
        Some(ViewRect {
            x: 0.0,
            y: 0.0,
            width: 18.0,
            height: 18.0,
        })
    );
}

#[test]
fn hidpi_resize_emits_one_exact_cells_pixels_and_dpi_command() {
    let model = model();
    let command = model.resize(901, 541, 2.0).unwrap();

    assert!(matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Resize(TerminalSize {
                cols: 100,
                rows: 30,
                pixel_width: 1802,
                pixel_height: 1082,
                dpi: 192,
            }),
            ..
        }
    ));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(model.resize(901, 541, scale).is_err());
    }
    assert!(model.resize(0, 541, 1.0).is_err());
    assert!(model.resize(i32::MAX, i32::MAX, f64::MAX).is_err());
}

#[test]
fn resize_extremes_emit_real_1x1_and_999x999_commands() {
    let model = model();
    for (width, height, cols, rows) in [(1, 1, 1, 1), (999, 999, 111, 55)] {
        let command = model.resize(width, height, 1.0).expect("valid resize");
        assert!(matches!(
            command,
            UiCommand::Session {
                command: SessionUiCommand::Resize(TerminalSize {
                    cols: actual_cols,
                    rows: actual_rows,
                    pixel_width,
                    pixel_height,
                    dpi: 96,
                }),
                ..
            } if actual_cols == cols
                && actual_rows == rows
                && pixel_width == width as u32
                && pixel_height == height as u32
        ));
    }
}

#[test]
fn mouse_reporting_routes_mouse_while_local_scroll_and_selection_use_stable_cells() {
    let mut reporting = model();
    reporting.apply_frame(offset_rows(frame(1, true, &["row0", "row1"], None), 100));
    let command = reporting
        .mouse(PointerEvent::press(19.0, 19.0, 2.0, MouseButton::Left))
        .unwrap()
        .expect("reported mouse command");
    assert!(matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Mouse(event),
            ..
        } if event.kind == MouseEventKind::Press
            && event.cell == CellPosition { stable_row: 101, column: 2 }
            && event.viewport_row == 1
            && event.pixel_x == 38
            && event.pixel_y == 38
    ));
    let reported_scroll = reporting
        .mouse(PointerEvent::scroll(19.0, 19.0, 2.0, -3))
        .unwrap()
        .expect("reported scroll command");
    assert!(matches!(
        reported_scroll,
        UiCommand::Session {
            command: SessionUiCommand::Mouse(event),
            ..
        } if event.kind == MouseEventKind::Scroll
            && event.button == Some(MouseButton::WheelUp)
            && event.cell == CellPosition { stable_row: 101, column: 2 }
            && event.viewport_row == 1
    ));

    let mut local = model();
    local.apply_frame(frame(1, false, &["row0", "row1"], None));
    let scroll = local
        .mouse(PointerEvent::scroll(0.0, 0.0, 1.0, -3))
        .unwrap()
        .expect("local scroll command");
    assert!(matches!(
        scroll,
        UiCommand::Session {
            command: SessionUiCommand::Scroll(-3),
            ..
        }
    ));
    let selection = local.selection(1.0, 1.0, 30.0, 19.0, false).unwrap();
    assert!(matches!(
        selection,
        UiCommand::Session {
            command: SessionUiCommand::Select(range),
            ..
        } if range.start == CellPosition { stable_row: 0, column: 0 }
            && range.end == CellPosition { stable_row: 1, column: 3 }
            && !range.rectangular
    ));
    assert!(local.selection(-1.0, 0.0, 1.0, 1.0, false).is_err());
}

#[test]
fn search_shortcut_results_and_next_previous_are_deterministic() {
    let mut model = model();
    model.apply_frame(frame(1, false, &["alpha", "alpha"], None));
    let shortcut = model
        .key(
            Key::from_name("f").unwrap(),
            ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK,
        )
        .unwrap();
    assert!(shortcut.is_none());
    assert!(model.search_is_open());
    assert!(matches!(
        model.search("alpha", false, false).unwrap(),
        UiCommand::Session {
            command: SessionUiCommand::Search(_),
            ..
        }
    ));
    model.apply_search_results(vec![search_match(0), search_match(1)]);
    assert_eq!(model.current_search_match(), Some(search_match(0)));

    let next = model
        .key(Key::Return, ModifierType::empty())
        .unwrap()
        .unwrap();
    assert_selects(next, search_match(1));
    let previous = model
        .key(Key::Return, ModifierType::SHIFT_MASK)
        .unwrap()
        .unwrap();
    assert_selects(previous, search_match(0));
}

#[test]
fn copy_waits_for_session_copy_event_before_requesting_gtk_write() {
    let mut model = model();
    assert!(matches!(
        model.copy(),
        UiCommand::Session {
            command: SessionUiCommand::CopySelection,
            ..
        }
    ));
    assert_eq!(model.take_clipboard_action(), None);
    model.apply_session_event(SessionUiEvent::State(rshell_core::SessionState::Connected));
    assert_eq!(model.take_clipboard_action(), None);
    model.apply_session_event(SessionUiEvent::Copy("selected text".into()));
    assert_eq!(
        model.take_clipboard_action(),
        Some(TerminalClipboardAction::Write("selected text".into()))
    );
    assert!(
        !format!(
            "{:?}",
            TerminalClipboardAction::Write("selected text".into())
        )
        .contains("selected text")
    );
    assert_eq!(model.take_clipboard_action(), None);
}

fn model() -> TerminalViewModel {
    TerminalViewModel::new(SessionId::new(), FontMetrics::new(9.0, 18.0).unwrap())
}

fn frame(
    generation: u64,
    mouse_reporting: bool,
    lines: &[&str],
    cursor: Option<(i64, u16)>,
) -> Arc<RenderFrame> {
    let rows = lines
        .iter()
        .enumerate()
        .map(|(stable_row, text)| RenderRow {
            stable_row: stable_row as i64,
            wrapped: false,
            cells: Arc::from(text.chars().map(cell).collect::<Vec<_>>()),
        })
        .collect::<Vec<_>>();
    Arc::new(RenderFrame {
        generation,
        size: TerminalSize {
            cols: 8,
            rows: lines.len() as u16,
            pixel_width: 72,
            pixel_height: lines.len() as u32 * 18,
            dpi: 96,
        },
        viewport_top: 0,
        rows: Arc::from(rows),
        cursor: cursor.map(|(stable_row, column)| RenderCursor {
            position: CellPosition { stable_row, column },
            shape: CursorShape::Block,
            visible: true,
        }),
        title: "fixture".into(),
        alternate_screen: false,
        mouse_reporting,
    })
}

fn wide_frame(generation: u64) -> Arc<RenderFrame> {
    let mut frame = frame(generation, false, &[""], Some((0, 0)));
    Arc::get_mut(&mut frame).unwrap().rows = Arc::from([RenderRow {
        stable_row: 0,
        wrapped: false,
        cells: Arc::from([RenderCell {
            text: "界".into(),
            width: 2,
            foreground: Color::Default,
            background: Color::Default,
            attributes: CellAttributes::default(),
            selected: false,
        }]),
    }]);
    frame
}

fn offset_rows(mut frame: Arc<RenderFrame>, offset: i64) -> Arc<RenderFrame> {
    let frame = Arc::make_mut(&mut frame);
    frame.viewport_top = offset;
    frame.rows = Arc::from(
        frame
            .rows
            .iter()
            .map(|row| RenderRow {
                stable_row: row.stable_row + offset,
                ..row.clone()
            })
            .collect::<Vec<_>>(),
    );
    frame.clone().into()
}

fn cell(character: char) -> RenderCell {
    RenderCell {
        text: character.to_string(),
        width: 1,
        foreground: Color::Default,
        background: Color::Default,
        attributes: CellAttributes::default(),
        selected: false,
    }
}

fn search_match(row: i64) -> SearchMatch {
    SearchMatch {
        start: CellPosition {
            stable_row: row,
            column: 0,
        },
        end: CellPosition {
            stable_row: row,
            column: 4,
        },
    }
}

fn assert_selects(command: UiCommand, expected: SearchMatch) {
    assert!(matches!(
        command,
        UiCommand::Session {
            command: SessionUiCommand::Select(range),
            ..
        } if range.start == expected.start && range.end == expected.end
    ));
}
