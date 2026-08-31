use gtk::{gdk, prelude::*};
use relm4::{ComponentParts, ComponentSender, SimpleComponent, gtk};
use rshell_core::{MouseButton, MouseEventKind, UiCommand};

use crate::{
    FontMetricsService, PointerEvent, TerminalClipboardAction, TerminalViewError,
    TerminalViewModel, terminal_view_clipboard as clipboard_io,
    terminal_view_metrics as metric_refresh, terminal_view_widgets::TerminalViewWidgets,
};

pub use crate::terminal_view_message::{TerminalViewInit, TerminalViewMsg, TerminalViewOutput};

pub struct TerminalView {
    model: TerminalViewModel,
    metrics_service: FontMetricsService,
    metric_widget: gtk::DrawingArea,
    clipboard: gdk::Clipboard,
    selection_anchor: Option<(f64, f64)>,
    pressed_button: Option<MouseButton>,
}

impl SimpleComponent for TerminalView {
    type Init = TerminalViewInit;
    type Input = TerminalViewMsg;
    type Output = TerminalViewOutput;
    type Root = gtk::Overlay;
    type Widgets = TerminalViewWidgets;

    fn init_root() -> Self::Root {
        let root = gtk::Overlay::new();
        root.add_css_class("terminal-view");
        root.add_css_class("terminal-container");
        root
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = TerminalViewWidgets::build(&root, &init, &sender);
        let metrics_service = FontMetricsService::from_measured(init.metrics.clone());
        let model = Self {
            model: TerminalViewModel::with_profile(
                init.pane,
                init.session,
                init.profile,
                init.metrics,
            ),
            metrics_service,
            metric_widget: widgets.canvas.clone(),
            clipboard: root.display().clipboard(),
            selection_anchor: None,
            pressed_button: None,
        };
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            TerminalViewMsg::ApplyFrame(frame) => {
                self.model.apply_frame(frame);
            }
            TerminalViewMsg::RefreshMetrics(environment) => {
                output_optional(
                    metric_refresh::refresh_metrics(
                        &mut self.metrics_service,
                        &self.metric_widget,
                        &mut self.model,
                        environment,
                    ),
                    &sender,
                );
            }
            TerminalViewMsg::RefreshGeometry => output_optional(
                metric_refresh::refresh_current_geometry(
                    &mut self.metrics_service,
                    &self.metric_widget,
                    &mut self.model,
                ),
                &sender,
            ),
            TerminalViewMsg::ReplayGeometry => output_optional(
                metric_refresh::replay_current_geometry(
                    &mut self.metrics_service,
                    &self.metric_widget,
                    &mut self.model,
                ),
                &sender,
            ),
            TerminalViewMsg::UpdateProfile(profile) => output_optional(
                metric_refresh::refresh_profile(
                    &mut self.metrics_service,
                    &self.metric_widget,
                    &mut self.model,
                    profile,
                ),
                &sender,
            ),
            TerminalViewMsg::Key { key, state } => self.handle_key(key, state, &sender),
            TerminalViewMsg::KeyReleased(key) => self.model.key_released(key),
            TerminalViewMsg::FocusLost => self.model.focus_lost(),
            TerminalViewMsg::CommittedText(text) => {
                let result = self.model.committed_text(&text);
                output_result(result, &sender);
            }
            TerminalViewMsg::Pointer(event) => self.handle_pointer(event, &sender),
            TerminalViewMsg::Resize {
                width,
                height,
                scale,
            } => output_optional(
                metric_refresh::refresh_geometry(
                    &mut self.metrics_service,
                    &self.metric_widget,
                    &mut self.model,
                    width,
                    height,
                    scale,
                ),
                &sender,
            ),
            TerminalViewMsg::Selection {
                start_x,
                start_y,
                end_x,
                end_y,
                rectangular,
            } => output_result(
                self.model
                    .selection(start_x, start_y, end_x, end_y, rectangular),
                &sender,
            ),
            TerminalViewMsg::Search {
                text,
                case_sensitive,
                regex,
            } => output_result(self.model.search(&text, case_sensitive, regex), &sender),
            TerminalViewMsg::PasteText(text) => {
                output_result(self.model.paste(&text), &sender);
            }
            TerminalViewMsg::ReadClipboard => clipboard_io::read(&self.clipboard, &sender),
            TerminalViewMsg::Copy => output_command(self.model.copy(), &sender),
            TerminalViewMsg::SessionEvent(event) => {
                self.model.apply_session_event(event);
                if let Some(TerminalClipboardAction::Write(text)) =
                    self.model.take_clipboard_action()
                {
                    let bytes = text.len();
                    self.clipboard.set_text(&text);
                    let _ = sender.output(TerminalViewOutput::ClipboardWritten { bytes });
                }
            }
        }
    }

    fn update_view(&self, widgets: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        widgets.sync(&self.model);
    }
}

impl TerminalView {
    fn handle_key(
        &mut self,
        key: gdk::Key,
        state: gdk::ModifierType,
        sender: &ComponentSender<Self>,
    ) {
        let value = key.to_unicode().map(|value| value.to_ascii_lowercase());
        let control_shift = state.contains(gdk::ModifierType::CONTROL_MASK)
            && state.contains(gdk::ModifierType::SHIFT_MASK);
        if control_shift && value == Some('c') {
            output_command(self.model.copy(), sender);
        } else if control_shift && value == Some('v') {
            clipboard_io::read(&self.clipboard, sender);
        } else {
            match self.model.key(key, state) {
                Ok(Some(command)) => output_command(command, sender),
                Ok(None) => {}
                Err(error) => output_error(error, sender),
            }
        }
    }

    fn handle_pointer(&mut self, mut event: PointerEvent, sender: &ComponentSender<Self>) {
        if event.kind == MouseEventKind::Move {
            event.button = self.pressed_button;
        }
        let reports_mouse = self.model.reports_mouse();
        if reports_mouse {
            if event.kind == MouseEventKind::Press {
                self.pressed_button = event.button;
            } else if event.kind == MouseEventKind::Release {
                self.pressed_button = None;
            }
            output_optional(self.model.mouse(event), sender);
            return;
        }
        match event.kind {
            MouseEventKind::Press if event.button == Some(MouseButton::Left) => {
                self.selection_anchor = Some((event.x, event.y));
            }
            MouseEventKind::Move => {
                if let Some((start_x, start_y)) = self.selection_anchor {
                    output_result(
                        self.model
                            .selection(start_x, start_y, event.x, event.y, false),
                        sender,
                    );
                }
            }
            MouseEventKind::Release => {
                if let Some((start_x, start_y)) = self.selection_anchor.take() {
                    output_result(
                        self.model
                            .selection(start_x, start_y, event.x, event.y, false),
                        sender,
                    );
                }
            }
            MouseEventKind::Scroll => output_optional(self.model.mouse(event), sender),
            MouseEventKind::Press => {}
        }
    }
}

fn output_optional(
    result: Result<Option<UiCommand>, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(Some(command)) => output_command(command, sender),
        Ok(None) => {}
        Err(error) => output_error(error, sender),
    }
}

fn output_result(
    result: Result<UiCommand, TerminalViewError>,
    sender: &ComponentSender<TerminalView>,
) {
    match result {
        Ok(command) => output_command(command, sender),
        Err(error) => output_error(error, sender),
    }
}

fn output_command(command: UiCommand, sender: &ComponentSender<TerminalView>) {
    let _ = sender.output(TerminalViewOutput::Command(Box::new(command)));
}

fn output_error(error: TerminalViewError, sender: &ComponentSender<TerminalView>) {
    let _ = sender.output(TerminalViewOutput::Error(error));
}

#[cfg(test)]
use clipboard_io::map_clipboard_read_result;

#[cfg(test)]
#[path = "terminal_view_tests.rs"]
mod tests;
