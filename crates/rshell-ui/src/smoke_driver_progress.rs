use crate::{SmokeActionKind, smoke_driver_state::SmokeDriver};

#[derive(Clone, Copy)]
pub(crate) enum SmokeProgress {
    Started,
    Passed,
    Failed,
}

pub(crate) fn emit_progress(
    state: SmokeProgress,
    step: usize,
    action: SmokeActionKind,
    code: Option<&'static str>,
) {
    eprintln!("{}", progress_line(state, step, action, code));
}

pub(crate) fn progress_line(
    state: SmokeProgress,
    step: usize,
    action: SmokeActionKind,
    code: Option<&'static str>,
) -> String {
    let state = match state {
        SmokeProgress::Started => "started",
        SmokeProgress::Passed => "passed",
        SmokeProgress::Failed => "failed",
    };
    match code {
        Some(code) => format!(
            "P0_SMOKE state={state} step={step} action={} code={code}",
            action.as_str()
        ),
        None => format!(
            "P0_SMOKE state={state} step={step} action={}",
            action.as_str()
        ),
    }
}

impl SmokeDriver {
    pub(crate) fn is_active(&self) -> bool {
        !self.complete
    }

    pub(crate) fn record_png_path(&self, path: std::path::PathBuf) {
        self.report.mutate(|report| report.png_path = Some(path));
    }

    pub(crate) fn record_png_error(&self, error: &'static str) {
        self.report.mutate(|report| report.png_error = Some(error));
    }
}
