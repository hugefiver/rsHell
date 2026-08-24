use std::sync::Arc;

use gtk::cairo::{Context, Format, ImageSurface};
use rshell_core::{
    CellAttributes, CellPosition, Color, CursorShape, RenderCell, RenderCursor, RenderFrame,
    RenderRow, SearchMatch, TerminalOverrides, TerminalSettingsV1, TerminalSize,
};
use rshell_ui::{FontMetrics, TerminalDecorations, TerminalRenderCache, TerminalRenderer};

#[test]
fn offscreen_renderer_paints_deterministic_geometry_cursor_and_overlays() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);
    let frame = fixture_frame();
    let decorations = TerminalDecorations::new(
        vec![SearchMatch {
            start: CellPosition {
                stable_row: 0,
                column: 2,
            },
            end: CellPosition {
                stable_row: 0,
                column: 3,
            },
        }],
        Some(0),
    );
    let surface = ImageSurface::create(Format::ARgb32, 36, 36).unwrap();
    let context = Context::new(&surface).unwrap();

    let stats = renderer
        .draw(&context, &frame, &decorations, 36, 36)
        .unwrap();
    assert_eq!(stats.rows, 2);
    assert_eq!(stats.text_runs, 4);
    assert_eq!(stats.wide_cells, 1);
    assert_eq!(stats.combining_cells, 1);
    assert_eq!(stats.selected_cells, 1);
    assert_eq!(stats.search_cells, 1);
    assert_eq!(stats.cursor_shape, Some(CursorShape::Underline));
    assert_eq!(stats.cursor_width, Some(18.0));

    let mut geometry_surface = ImageSurface::create(Format::ARgb32, 36, 36).unwrap();
    let geometry_context = Context::new(&geometry_surface).unwrap();
    renderer
        .draw(&geometry_context, &geometry_frame(), &decorations, 36, 36)
        .unwrap();
    drop(geometry_context);
    geometry_surface.flush();
    assert_eq!(
        stable_pixel_hash(geometry_surface.data().unwrap().as_ref()),
        9_452_195_836_048_871_113
    );
}

#[test]
fn renderer_resolves_ansi_rgb_reverse_and_all_text_attributes() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);
    let mut surface = ImageSurface::create(Format::ARgb32, 36, 18).unwrap();
    let context = Context::new(&surface).unwrap();
    let frame = styled_frame();

    let stats = renderer
        .draw(&context, &frame, &TerminalDecorations::default(), 36, 18)
        .unwrap();
    drop(context);
    surface.flush();

    assert_eq!(stats.bold_cells, 1);
    assert_eq!(stats.italic_cells, 1);
    assert_eq!(stats.underlined_cells, 1);
    assert_eq!(stats.struck_cells, 1);
    assert_eq!(stats.reversed_cells, 1);
    let bytes = surface.data().unwrap();
    assert!(
        bytes
            .as_ref()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| *pixel != [0, 0, 0, 0])
    );
}

#[test]
fn renderer_handles_every_cursor_shape_on_a_wide_cell() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);

    for shape in [
        CursorShape::Block,
        CursorShape::Beam,
        CursorShape::Underline,
    ] {
        let mut frame = fixture_frame();
        Arc::make_mut(&mut frame).cursor.as_mut().unwrap().shape = shape;
        let surface = ImageSurface::create(Format::ARgb32, 36, 36).unwrap();
        let context = Context::new(&surface).unwrap();
        let stats = renderer
            .draw(&context, &frame, &TerminalDecorations::default(), 36, 36)
            .unwrap();
        assert_eq!(stats.cursor_shape, Some(shape));
        assert_eq!(stats.cursor_width, Some(18.0));
    }
}

#[test]
fn retained_renderer_relayouts_only_the_single_changed_row() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);
    let mut cache = TerminalRenderCache::new();
    let first = fixture_frame();

    let initial = cache
        .update(
            &renderer,
            Arc::clone(&first),
            &TerminalDecorations::default(),
            36,
            36,
            1,
        )
        .unwrap();
    assert_eq!(initial.rows, 2);
    assert_eq!(initial.text_runs, 4);

    let mut next = first.as_ref().clone();
    next.generation = 2;
    next.rows = Arc::from([
        next.rows[0].clone(),
        RenderRow {
            stable_row: 1,
            wrapped: false,
            cells: Arc::from([cell("changed", 1, false)]),
        },
    ]);
    let incremental = cache
        .update(
            &renderer,
            Arc::new(next),
            &TerminalDecorations::default(),
            36,
            36,
            1,
        )
        .unwrap();

    assert_eq!(incremental.rows, 1);
    assert_eq!(incremental.text_runs, 1);

    let surface = ImageSurface::create(Format::ARgb32, 36, 36).unwrap();
    let context = Context::new(&surface).unwrap();
    cache.paint(&context).unwrap();
}

#[test]
fn retained_renderer_repaints_old_and_new_search_rows() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);
    let mut cache = TerminalRenderCache::new();
    let frame = fixture_frame();
    let first = TerminalDecorations::new(vec![search_match(0)], Some(0));
    cache
        .update(&renderer, Arc::clone(&frame), &first, 36, 36, 1)
        .unwrap();

    let moved = TerminalDecorations::new(vec![search_match(1)], Some(0));
    let stats = cache.update(&renderer, frame, &moved, 36, 36, 1).unwrap();

    assert_eq!(stats.rows, 2);
}

#[test]
fn retained_renderer_never_adopts_equal_generation_content() {
    let metrics = FontMetrics::new(9.0, 18.0).unwrap();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let renderer = TerminalRenderer::new(&profile, metrics);
    let mut cache = TerminalRenderCache::new();
    let first = fixture_frame();
    cache
        .update(
            &renderer,
            Arc::clone(&first),
            &TerminalDecorations::default(),
            36,
            36,
            1,
        )
        .unwrap();

    let mut equal = first.as_ref().clone();
    equal.rows = Arc::from([
        equal.rows[0].clone(),
        RenderRow {
            stable_row: 1,
            wrapped: false,
            cells: Arc::from([cell("must-not-be-adopted", 1, false)]),
        },
    ]);
    let ignored = cache
        .update(
            &renderer,
            Arc::new(equal),
            &TerminalDecorations::default(),
            36,
            36,
            1,
        )
        .unwrap();
    assert_eq!(ignored, Default::default());

    let mut next = first.as_ref().clone();
    next.generation = 2;
    let unchanged = cache
        .update(
            &renderer,
            Arc::new(next),
            &TerminalDecorations::default(),
            36,
            36,
            1,
        )
        .unwrap();
    assert_eq!(unchanged, Default::default());
}

fn fixture_frame() -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation: 1,
        size: size(4, 2),
        viewport_top: 0,
        rows: Arc::from([
            RenderRow {
                stable_row: 0,
                wrapped: false,
                cells: Arc::from([
                    cell("界", 2, false),
                    cell("e\u{301}", 1, false),
                    cell("x", 1, true),
                ]),
            },
            RenderRow {
                stable_row: 1,
                wrapped: false,
                cells: Arc::from([cell("z", 1, false)]),
            },
        ]),
        cursor: Some(RenderCursor {
            position: CellPosition {
                stable_row: 0,
                column: 0,
            },
            shape: CursorShape::Underline,
            visible: true,
        }),
        title: "draw fixture".into(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

fn geometry_frame() -> Arc<RenderFrame> {
    let source = fixture_frame();
    Arc::new(RenderFrame {
        rows: Arc::from(
            source
                .rows
                .iter()
                .map(|row| RenderRow {
                    stable_row: row.stable_row,
                    wrapped: row.wrapped,
                    cells: Arc::from(
                        row.cells
                            .iter()
                            .map(|cell| RenderCell {
                                text: String::new(),
                                ..cell.clone()
                            })
                            .collect::<Vec<_>>(),
                    ),
                })
                .collect::<Vec<_>>(),
        ),
        ..source.as_ref().clone()
    })
}

fn styled_frame() -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation: 2,
        size: size(4, 1),
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from([
                RenderCell {
                    text: "S".into(),
                    width: 1,
                    foreground: Color::Ansi(1),
                    background: Color::Rgb(12, 34, 56),
                    attributes: CellAttributes {
                        bold: true,
                        italic: true,
                        underline: true,
                        strike: true,
                        reverse: true,
                    },
                    selected: false,
                },
                cell(" ", 1, false),
            ]),
        }]),
        cursor: None,
        title: "style fixture".into(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

fn cell(text: &str, width: u8, selected: bool) -> RenderCell {
    RenderCell {
        text: text.into(),
        width,
        foreground: Color::Default,
        background: Color::Default,
        attributes: CellAttributes::default(),
        selected,
    }
}

fn size(cols: u16, rows: u16) -> TerminalSize {
    TerminalSize {
        cols,
        rows,
        pixel_width: u32::from(cols) * 9,
        pixel_height: u32::from(rows) * 18,
        dpi: 96,
    }
}

fn search_match(stable_row: i64) -> SearchMatch {
    SearchMatch {
        start: CellPosition {
            stable_row,
            column: 0,
        },
        end: CellPosition {
            stable_row,
            column: 1,
        },
    }
}

fn stable_pixel_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
