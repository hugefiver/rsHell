use std::sync::Arc;

use gtk::pango;
use pangocairo::prelude::*;
use rshell_core::{
    CellAttributes, CellPosition, Color, ColorScheme, CursorShape, RenderCell, RenderCursor,
    RenderFrame, RenderRow, SessionId, TerminalOverrides, TerminalSettingsV1, TerminalSize,
};
use rshell_ui::{
    FontMetricEnvironment, FontMetricSample, FontMetricsService, MeasuredFontMetrics,
    MetricsChange, TerminalGeometryInput, TerminalViewModel, TerminalViewMsg,
};

#[test]
fn metric_identity_invalidates_without_recreating_session() {
    let context = native_context();
    let session = SessionId::new();
    let mut profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let mut environment = FontMetricEnvironment::new(1.0, 96.0).unwrap();
    let mut service = FontMetricsService::default();
    let initial = changed(service.measure(&context, &profile, environment).unwrap());
    let mut model = TerminalViewModel::new(session, initial);
    let model_identity = std::ptr::addr_of!(model) as usize;
    model.apply_frame(grid_identity_frame());
    let frame = Arc::clone(model.frame().unwrap());
    model
        .apply_geometry(TerminalGeometryInput {
            logical_width: 500,
            logical_height: 300,
            metrics: model.metrics(),
            environment,
        })
        .unwrap();

    let mut identities = Vec::new();
    profile.font_family = "Sans".into();
    identities.push((profile.clone(), environment));
    profile.font_size += 1.0;
    identities.push((profile.clone(), environment));
    profile.color_scheme = ColorScheme::Dracula;
    identities.push((profile.clone(), environment));
    environment = FontMetricEnvironment::new(1.25, 120.0).unwrap();
    identities.push((profile.clone(), environment));
    environment = FontMetricEnvironment::new(1.25, 144.0).unwrap();
    identities.push((profile.clone(), environment));

    for (next_profile, next_environment) in identities {
        let measured = changed(
            service
                .measure(&context, &next_profile, next_environment)
                .expect("changed metric identity must remeasure"),
        );
        let emitted = model.apply_metrics(measured.clone(), None).unwrap();
        assert!(matches!(
            emitted,
            None | Some(rshell_core::UiCommand::Session { .. })
        ));
        assert!(matches!(
            service.measure(&context, &next_profile, next_environment).unwrap(),
            MetricsChange::Unchanged(current) if current == measured
        ));
        assert_eq!(std::ptr::addr_of!(model) as usize, model_identity);
        assert!(Arc::ptr_eq(model.frame().unwrap(), &frame));
    }
    assert!(matches!(
        model.copy(),
        rshell_core::UiCommand::Session { session: actual, .. } if actual == session
    ));

    let refresh = TerminalViewMsg::RefreshMetrics(environment);
    let update = TerminalViewMsg::UpdateProfile(profile);
    assert!(format!("{refresh:?}").contains("RefreshMetrics"));
    assert_eq!(format!("{update:?}"), "UpdateProfile(..)");
}

#[test]
fn pure_metric_and_geometry_matrix_is_positive_ceil_rounded_and_exact() {
    for effective_dpi in [96.0, 120.0, 144.0, 192.0] {
        for font_size in [6.0_f64, 15.0, 72.0] {
            let metrics = FontMetricSample {
                approximate_char_width: font_size * 0.57,
                ascii_advance: font_size * 0.61,
                ascent: font_size * 0.78,
                descent: font_size * 0.22,
            }
            .to_metrics()
            .expect("valid sample");
            assert!(metrics.cell_width.is_finite() && metrics.cell_width > 0.0);
            assert!(metrics.cell_height.is_finite() && metrics.cell_height > 0.0);
            assert_eq!(metrics.cell_width.fract(), 0.0);
            assert_eq!(metrics.cell_height.fract(), 0.0);

            let logical_width = (metrics.cell_width * 11.0) as i32;
            let logical_height = (metrics.cell_height * 7.0) as i32;
            let environment = FontMetricEnvironment {
                effective_scale: effective_dpi / 96.0,
                effective_dpi,
                dpi_fallback_used: false,
            };
            let size = TerminalGeometryInput {
                logical_width,
                logical_height,
                metrics,
                environment,
            }
            .terminal_size()
            .expect("valid geometry");

            assert_eq!(size.cols, 11);
            assert_eq!(size.rows, 7);
            assert_eq!(
                size.pixel_width,
                (f64::from(logical_width) * environment.effective_scale).floor() as u32
            );
            assert_eq!(
                size.pixel_height,
                (f64::from(logical_height) * environment.effective_scale).floor() as u32
            );
            assert_eq!(size.dpi, effective_dpi as u32);
        }
    }
}

#[test]
fn invalid_samples_and_environments_fail_closed_without_numeric_cell_fallback() {
    assert_eq!(
        FontMetricEnvironment::fallback_for_scale(1.25).unwrap(),
        FontMetricEnvironment {
            effective_scale: 1.25,
            effective_dpi: 120.0,
            dpi_fallback_used: true,
        }
    );
    for sample in [
        FontMetricSample {
            approximate_char_width: f64::NAN,
            ascii_advance: f64::NAN,
            ascent: 10.0,
            descent: 3.0,
        },
        FontMetricSample {
            approximate_char_width: 0.0,
            ascii_advance: -1.0,
            ascent: 10.0,
            descent: 3.0,
        },
        FontMetricSample {
            approximate_char_width: 8.0,
            ascii_advance: 8.0,
            ascent: f64::INFINITY,
            descent: 3.0,
        },
    ] {
        assert!(sample.to_metrics().is_err());
    }

    let metrics = FontMetricSample {
        approximate_char_width: 8.0,
        ascii_advance: 9.0,
        ascent: 13.0,
        descent: 4.0,
    }
    .to_metrics()
    .unwrap();
    for environment in [
        FontMetricEnvironment {
            effective_scale: 0.0,
            effective_dpi: 96.0,
            dpi_fallback_used: false,
        },
        FontMetricEnvironment {
            effective_scale: 1.0,
            effective_dpi: f64::NAN,
            dpi_fallback_used: false,
        },
    ] {
        assert!(
            TerminalGeometryInput {
                logical_width: 900,
                logical_height: 600,
                metrics,
                environment,
            }
            .terminal_size()
            .is_err()
        );
    }
}

#[test]
fn native_pango_measurement_builds_exact_key_and_caches_unchanged_identity() {
    let context = native_context();
    let profile = TerminalSettingsV1 {
        font_family: "Monospace".into(),
        font_size: 15.0,
        color_scheme: ColorScheme::TokyoNight,
        ..TerminalSettingsV1::default()
    }
    .resolve(&TerminalOverrides::default());
    let environment = FontMetricEnvironment {
        effective_scale: 1.5,
        effective_dpi: 144.0,
        dpi_fallback_used: false,
    };
    let mut service = FontMetricsService::default();

    let first = service
        .measure(&context, &profile, environment)
        .expect("native Pango metrics");
    let measured = changed(first);
    assert_eq!(measured.key.family, profile.font_family);
    assert_eq!(measured.key.font_size_bits, profile.font_size.to_bits());
    assert_eq!(
        measured.key.effective_scale_bits,
        environment.effective_scale.to_bits()
    );
    assert_eq!(
        measured.key.effective_dpi_bits,
        environment.effective_dpi.to_bits()
    );
    assert_eq!(measured.key.color_scheme, profile.color_scheme);
    assert_eq!(measured.environment, environment);
    assert!(!measured.fallback_used);

    assert!(matches!(
        service.measure(&context, &profile, environment).unwrap(),
        MetricsChange::Unchanged(value) if value == measured
    ));
}

#[test]
fn native_environment_combines_context_dpi_with_widget_scale() {
    let font_map = pangocairo::FontMap::new();
    font_map
        .clone()
        .dynamic_cast::<pangocairo::FontMap>()
        .expect("PangoCairo font map")
        .set_resolution(120.0);
    let context = pango::Context::new();
    context.set_font_map(Some(&font_map));

    assert_eq!(
        FontMetricEnvironment::from_context(&context, 1.5).unwrap(),
        FontMetricEnvironment {
            effective_scale: 1.5,
            effective_dpi: 180.0,
            dpi_fallback_used: false,
        }
    );
}

#[test]
fn unusable_requested_description_uses_only_the_measured_monospace_fallback() {
    let context = native_context();
    let mut unusable = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    unusable.font_family.clear();
    let environment = FontMetricEnvironment {
        effective_scale: 1.0,
        effective_dpi: 96.0,
        dpi_fallback_used: false,
    };
    let fallback = changed(
        FontMetricsService::default()
            .measure(&context, &unusable, environment)
            .expect("measured fallback"),
    );

    let fallback_profile = TerminalSettingsV1 {
        font_family: "Monospace".into(),
        font_size: 10.0,
        ..TerminalSettingsV1::default()
    }
    .resolve(&TerminalOverrides::default());
    let direct = changed(
        FontMetricsService::default()
            .measure(&context, &fallback_profile, environment)
            .expect("direct fallback profile"),
    );

    assert!(fallback.fallback_used);
    assert_eq!(fallback.metrics, direct.metrics);
    assert!(fallback.metrics.cell_width > 0.0);
    assert!(fallback.metrics.cell_height > 0.0);
}

#[test]
fn native_ascii_combining_cjk_and_emoji_stay_on_protocol_grid_identity() {
    let context = native_context();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    let environment = FontMetricEnvironment {
        effective_scale: 2.0,
        effective_dpi: 192.0,
        dpi_fallback_used: false,
    };
    let mut service = FontMetricsService::default();
    let measured = changed(
        service
            .measure(&context, &profile, environment)
            .expect("native Pango metrics"),
    );

    let advances = ["M", "e\u{301}", "界", "🙂"].map(|text| {
        let layout = pango::Layout::new(&context);
        layout.set_font_description(Some(&measured.font_description));
        layout.set_text(text);
        layout.pixel_size().0
    });
    assert!(advances.into_iter().all(|advance| advance > 0));

    let mut model = TerminalViewModel::new(SessionId::new(), measured.clone());
    model.apply_frame(grid_identity_frame());
    let rect = model.cursor_rect().expect("wide CJK cursor");
    assert_eq!(rect.x, measured.metrics.cell_width * 2.0);
    assert_eq!(rect.width, measured.metrics.cell_width * 2.0);

    let emoji_x = measured.metrics.cell_width * 4.5;
    let selection = model
        .selection(emoji_x, 1.0, emoji_x, 1.0, false)
        .expect("emoji selection");
    assert!(matches!(
        selection,
        rshell_core::UiCommand::Session {
            command: rshell_core::SessionUiCommand::Select(range),
            ..
        } if range.start.column == 4 && range.end.column == 4
    ));
}

#[test]
fn default_terminal_face_is_scale_stable_and_occupies_its_grid() {
    let context = native_context();
    let profile = TerminalSettingsV1::default().resolve(&TerminalOverrides::default());
    assert_eq!(profile.font_family, "Cascadia Mono");
    let mut measured_metrics = Vec::new();
    for (scale, dpi) in [(1.0, 96.0), (2.0, 192.0)] {
        let measured = changed(
            FontMetricsService::default()
                .measure(
                    &context,
                    &profile,
                    FontMetricEnvironment {
                        effective_scale: scale,
                        effective_dpi: dpi,
                        dpi_fallback_used: false,
                    },
                )
                .expect("default terminal face metrics"),
        );
        let layout = pango::Layout::new(&context);
        layout.set_font_description(Some(&measured.font_description));
        layout.set_text("MMMMMMMM");
        let average_advance = f64::from(layout.size().0) / f64::from(pango::SCALE) / 8.0;
        let occupancy = average_advance / measured.metrics.cell_width;
        assert!(
            occupancy >= 0.72,
            "default terminal face is too loosely spaced: occupancy={occupancy}, metrics={:?}",
            measured.metrics
        );
        measured_metrics.push(measured.metrics);
    }
    assert_eq!(measured_metrics[0], measured_metrics[1]);
}

fn native_context() -> pango::Context {
    let font_map = pangocairo::FontMap::new();
    let context = pango::Context::new();
    context.set_font_map(Some(&font_map));
    context
}

fn changed(change: MetricsChange) -> MeasuredFontMetrics {
    match change {
        MetricsChange::Changed(measured) => measured,
        MetricsChange::Unchanged(_) => panic!("first measurement must be changed"),
    }
}

fn grid_identity_frame() -> Arc<RenderFrame> {
    Arc::new(RenderFrame {
        generation: 1,
        size: TerminalSize {
            cols: 5,
            rows: 1,
            pixel_width: 1,
            pixel_height: 1,
            dpi: 192,
        },
        viewport_top: 0,
        rows: Arc::from([RenderRow {
            stable_row: 0,
            wrapped: false,
            cells: Arc::from([
                cell("M", 1),
                cell("e\u{301}", 1),
                cell("界", 2),
                cell("🙂", 1),
            ]),
        }]),
        cursor: Some(RenderCursor {
            position: CellPosition {
                stable_row: 0,
                column: 2,
            },
            shape: CursorShape::Block,
            visible: true,
        }),
        title: String::new(),
        display_modes: Default::default(),
        alternate_screen: false,
        mouse_reporting: false,
    })
}

fn cell(text: &str, width: u8) -> RenderCell {
    RenderCell {
        text: text.into(),
        width,
        foreground: Color::Default,
        background: Color::Default,
        attributes: CellAttributes::default(),
        selected: false,
    }
}
