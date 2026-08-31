use gtk::pango;
use pangocairo::prelude::*;
use rshell_core::{ColorScheme, ResolvedTerminalProfile};

use crate::terminal_font;
use crate::{FontMetrics, TerminalViewError, terminal_input::positive_finite};

const MEASURED_FALLBACK_FAMILY: &str = "Monospace";
const MEASURED_FALLBACK_SIZE: f32 = 10.0;
pub const TERMINAL_LINE_SPACING: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetricSample {
    pub approximate_char_width: f64,
    pub ascii_advance: f64,
    pub ascent: f64,
    pub descent: f64,
}

impl FontMetricSample {
    pub fn to_metrics(self) -> Result<FontMetrics, TerminalViewError> {
        if !positive_finite(self.approximate_char_width)
            || !positive_finite(self.ascii_advance)
            || !positive_finite(self.ascent)
            || !positive_finite(self.descent)
        {
            return Err(TerminalViewError::InvalidFontMetrics);
        }
        FontMetrics::new(
            self.approximate_char_width.max(self.ascii_advance).ceil(),
            (self.ascent + self.descent + TERMINAL_LINE_SPACING).ceil(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMetricEnvironment {
    pub effective_scale: f64,
    pub effective_dpi: f64,
    pub dpi_fallback_used: bool,
}

impl FontMetricEnvironment {
    pub fn new(effective_scale: f64, effective_dpi: f64) -> Result<Self, TerminalViewError> {
        let environment = Self {
            effective_scale,
            effective_dpi,
            dpi_fallback_used: false,
        };
        environment.validate()?;
        Ok(environment)
    }

    pub fn from_context(
        context: &pango::Context,
        effective_scale: f64,
    ) -> Result<Self, TerminalViewError> {
        if !positive_finite(effective_scale) {
            return Err(TerminalViewError::InvalidScale);
        }
        let measured_dpi = context
            .font_map()
            .and_then(|font_map| font_map.dynamic_cast::<pangocairo::FontMap>().ok())
            .map(|font_map| font_map.resolution())
            .filter(|dpi| positive_finite(*dpi));
        match measured_dpi {
            Some(base_dpi) => Self::new(effective_scale, base_dpi * effective_scale),
            None => Self::fallback_for_scale(effective_scale),
        }
    }

    pub fn fallback_for_scale(effective_scale: f64) -> Result<Self, TerminalViewError> {
        let mut environment = Self::new(effective_scale, 96.0 * effective_scale)?;
        environment.dpi_fallback_used = true;
        Ok(environment)
    }

    pub(crate) fn validate(self) -> Result<(), TerminalViewError> {
        if !positive_finite(self.effective_scale) {
            return Err(TerminalViewError::InvalidScale);
        }
        if !positive_finite(self.effective_dpi) {
            return Err(TerminalViewError::InvalidDpi);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontMetricKey {
    pub family: String,
    pub font_size_bits: u32,
    pub effective_scale_bits: u64,
    pub effective_dpi_bits: u64,
    pub dpi_fallback_used: bool,
    pub color_scheme: ColorScheme,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredFontMetrics {
    pub metrics: FontMetrics,
    pub key: FontMetricKey,
    pub environment: FontMetricEnvironment,
    pub fallback_used: bool,
    pub font_description: pango::FontDescription,
    pub minimum_line_separation: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalGeometryInput {
    pub logical_width: i32,
    pub logical_height: i32,
    pub metrics: FontMetrics,
    pub environment: FontMetricEnvironment,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MetricsChange {
    Unchanged(MeasuredFontMetrics),
    Changed(MeasuredFontMetrics),
}

#[derive(Debug, Default)]
pub struct FontMetricsService {
    current: Option<MeasuredFontMetrics>,
}

impl FontMetricsService {
    pub fn from_measured(measured: MeasuredFontMetrics) -> Self {
        Self {
            current: Some(measured),
        }
    }

    pub fn measure(
        &mut self,
        context: &pango::Context,
        profile: &ResolvedTerminalProfile,
        environment: FontMetricEnvironment,
    ) -> Result<MetricsChange, TerminalViewError> {
        environment.validate()?;
        let key = FontMetricKey {
            family: profile.font_family.clone(),
            font_size_bits: profile.font_size.to_bits(),
            effective_scale_bits: environment.effective_scale.to_bits(),
            effective_dpi_bits: environment.effective_dpi.to_bits(),
            dpi_fallback_used: environment.dpi_fallback_used,
            color_scheme: profile.color_scheme,
        };
        if let Some(current) = self.current.as_ref().filter(|current| current.key == key) {
            return Ok(MetricsChange::Unchanged(current.clone()));
        }

        let (metrics, font_description, ink_height, fallback_used) = match measure_description(
            context,
            &profile.font_family,
            profile.font_size,
            environment.effective_scale,
        ) {
            Ok((metrics, description, ink_height)) => (metrics, description, ink_height, false),
            Err(_) => {
                let (metrics, description, ink_height) = measure_description(
                    context,
                    MEASURED_FALLBACK_FAMILY,
                    MEASURED_FALLBACK_SIZE,
                    environment.effective_scale,
                )?;
                (metrics, description, ink_height, true)
            }
        };
        let measured = MeasuredFontMetrics {
            metrics,
            key,
            environment,
            fallback_used,
            font_description,
            minimum_line_separation: metrics.cell_height - ink_height,
        };
        self.current = Some(measured.clone());
        Ok(MetricsChange::Changed(measured))
    }
}

fn measure_description(
    context: &pango::Context,
    family: &str,
    font_size: f32,
    effective_scale: f64,
) -> Result<(FontMetrics, pango::FontDescription, f64), TerminalViewError> {
    if family.trim().is_empty() || !positive_finite(f64::from(font_size)) {
        return Err(TerminalViewError::InvalidFontMetrics);
    }
    let description = logical_font_description(family, font_size);
    let resolved = context.metrics(Some(&description), context.language().as_ref());
    let layout = pango::Layout::new(context);
    layout.set_font_description(Some(&description));
    layout.set_text("M");
    let (ascii_width, _) = layout.size();
    let scale = f64::from(pango::SCALE);
    let mut metrics = FontMetricSample {
        approximate_char_width: f64::from(resolved.approximate_char_width()) / scale,
        ascii_advance: f64::from(ascii_width) / scale,
        ascent: f64::from(resolved.ascent()) / scale,
        descent: f64::from(resolved.descent()) / scale,
    }
    .to_metrics()?;
    let (ink_width, ink_height) = measured_render_ink(&description, effective_scale)?;
    metrics.cell_width = metrics.cell_width.max(ink_width.ceil());
    metrics.cell_height = metrics
        .cell_height
        .max((ink_height + TERMINAL_LINE_SPACING).ceil());
    Ok((metrics, description, ink_height))
}

pub fn logical_font_description(family: &str, font_size: f32) -> pango::FontDescription {
    let mut description = pango::FontDescription::new();
    description.set_family(family);
    description.set_absolute_size(f64::from(font_size) * f64::from(pango::SCALE));
    description
}

fn measured_render_ink(
    description: &pango::FontDescription,
    effective_scale: f64,
) -> Result<(f64, f64), TerminalViewError> {
    let surface = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, 1, 1)
        .map_err(|_| TerminalViewError::InvalidFontMetrics)?;
    surface.set_device_scale(effective_scale, effective_scale);
    let context =
        gtk::cairo::Context::new(&surface).map_err(|_| TerminalViewError::InvalidFontMetrics)?;
    let mut max_cell_width = 0.0_f64;
    let mut max_ink_height = 0.0_f64;
    let printable_ascii = (' '..='~').map(|value| (value.to_string(), 1.0));
    let representative = [
        ("e\u{301}".to_owned(), 1.0),
        ("界".to_owned(), 2.0),
        ("🙂".to_owned(), 2.0),
    ];
    for (text, columns) in printable_ascii.chain(representative) {
        let layout = pangocairo::functions::create_layout(&context);
        let font = terminal_font::for_text(description, &text);
        layout.set_font_description(Some(&font));
        layout.set_text(&text);
        let (ink, _) = layout.extents();
        let pango_scale = f64::from(pango::SCALE);
        max_cell_width = max_cell_width.max((f64::from(ink.width()) / pango_scale) / columns);
        max_ink_height = max_ink_height.max(f64::from(ink.height()) / pango_scale);
    }
    for (weight, style) in [
        (pango::Weight::Bold, pango::Style::Normal),
        (pango::Weight::Normal, pango::Style::Italic),
    ] {
        let mut styled = description.clone();
        styled.set_weight(weight);
        styled.set_style(style);
        let layout = pangocairo::functions::create_layout(&context);
        layout.set_font_description(Some(&styled));
        layout.set_text("M");
        let (ink, _) = layout.extents();
        let pango_scale = f64::from(pango::SCALE);
        max_cell_width = max_cell_width.max(f64::from(ink.width()) / pango_scale);
        max_ink_height = max_ink_height.max(f64::from(ink.height()) / pango_scale);
    }
    if !positive_finite(max_cell_width) || !positive_finite(max_ink_height) {
        return Err(TerminalViewError::InvalidFontMetrics);
    }
    Ok((max_cell_width, max_ink_height))
}
