use gtk::prelude::*;
use relm4::{ComponentSender, gtk};
use rshell_core::{ResolvedTerminalProfile, UiCommand};

use crate::{
    FontMetricEnvironment, FontMetricsService, MetricsChange, TerminalGeometryInput, TerminalView,
    TerminalViewError, TerminalViewModel, TerminalViewMsg,
};

const GTK_FONT_DPI_PROPERTY: &str = "gtk-xft-dpi";

pub(crate) fn connect_metric_refresh(
    canvas: &gtk::DrawingArea,
    sender: &ComponentSender<TerminalView>,
) {
    let input = sender.input_sender().clone();
    canvas.connect_notify_local(Some("scale-factor"), move |canvas, _| {
        send_refresh(canvas, &input);
    });

    let input = sender.input_sender().clone();
    canvas.connect_realize(move |canvas| {
        let _ = canvas.pango_context();
        send_refresh(canvas, &input);
    });

    if let Some(settings) = gtk::Settings::default()
        && settings.find_property(GTK_FONT_DPI_PROPERTY).is_some()
    {
        let weak_canvas = canvas.downgrade();
        let input = sender.input_sender().clone();
        settings.connect_notify_local(Some(GTK_FONT_DPI_PROPERTY), move |_, _| {
            if let Some(canvas) = weak_canvas.upgrade() {
                send_refresh(&canvas, &input);
            }
        });
    }
}

pub(crate) fn metric_environment(
    widget: &impl IsA<gtk::Widget>,
) -> Result<FontMetricEnvironment, crate::TerminalViewError> {
    let scale = f64::from(widget.scale_factor());
    if let Some(base_dpi) = gtk_settings_base_dpi() {
        FontMetricEnvironment::new(scale, base_dpi * scale)
    } else {
        FontMetricEnvironment::fallback_for_scale(scale)
    }
}

pub(crate) fn refresh_metrics(
    service: &mut FontMetricsService,
    widget: &gtk::DrawingArea,
    model: &mut TerminalViewModel,
    environment: FontMetricEnvironment,
) -> Result<Option<UiCommand>, TerminalViewError> {
    match service.measure(&widget.pango_context(), &model.profile, environment)? {
        MetricsChange::Unchanged(_) => Ok(None),
        MetricsChange::Changed(measured) => model.apply_metrics(measured, None),
    }
}

pub(crate) fn refresh_profile(
    service: &mut FontMetricsService,
    widget: &gtk::DrawingArea,
    model: &mut TerminalViewModel,
    profile: ResolvedTerminalProfile,
) -> Result<Option<UiCommand>, TerminalViewError> {
    if model.profile == profile {
        return Ok(None);
    }
    model.profile = profile;
    refresh_metrics(service, widget, model, metric_environment(widget)?)
}

pub(crate) fn refresh_current_geometry(
    service: &mut FontMetricsService,
    widget: &gtk::DrawingArea,
    model: &mut TerminalViewModel,
) -> Result<Option<UiCommand>, TerminalViewError> {
    refresh_geometry(
        service,
        widget,
        model,
        widget.width(),
        widget.height(),
        f64::from(widget.scale_factor()),
    )
}

pub(crate) fn replay_current_geometry(
    service: &mut FontMetricsService,
    widget: &gtk::DrawingArea,
    model: &mut TerminalViewModel,
) -> Result<Option<UiCommand>, TerminalViewError> {
    Ok(refresh_current_geometry(service, widget, model)?.or_else(|| model.replay_geometry()))
}

pub(crate) fn refresh_geometry(
    service: &mut FontMetricsService,
    widget: &gtk::DrawingArea,
    model: &mut TerminalViewModel,
    width: i32,
    height: i32,
    scale: f64,
) -> Result<Option<UiCommand>, TerminalViewError> {
    let current = model.measured_metrics().environment;
    let metric_command = if current.effective_scale.to_bits() == scale.to_bits() {
        None
    } else {
        refresh_metrics(
            service,
            widget,
            model,
            FontMetricEnvironment {
                effective_scale: scale,
                effective_dpi: (current.effective_dpi / current.effective_scale) * scale,
                dpi_fallback_used: current.dpi_fallback_used,
            },
        )?
    };
    let geometry_command = model.apply_geometry(TerminalGeometryInput {
        logical_width: width,
        logical_height: height,
        metrics: model.metrics(),
        environment: model.measured_metrics().environment,
    })?;
    Ok(geometry_command.or(metric_command))
}

fn gtk_settings_base_dpi() -> Option<f64> {
    let settings = gtk::Settings::default()?;
    settings.find_property(GTK_FONT_DPI_PROPERTY)?;
    let raw = settings.property::<i32>(GTK_FONT_DPI_PROPERTY);
    (raw > 0).then_some(f64::from(raw) / 1024.0)
}

fn send_refresh(widget: &impl IsA<gtk::Widget>, input: &relm4::Sender<TerminalViewMsg>) {
    if let Ok(environment) = metric_environment(widget) {
        let _ = input.send(TerminalViewMsg::RefreshMetrics(environment));
    }
}
