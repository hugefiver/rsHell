use crate::{
    SmokeActionKind,
    smoke_driver_progress::{SmokeProgress, progress_line},
};

#[test]
fn progress_lines_expose_only_static_action_metadata() {
    assert_eq!(
        progress_line(
            SmokeProgress::Started,
            7,
            SmokeActionKind::PasteTextFromEnv,
            None
        ),
        "P0_SMOKE state=started step=7 action=paste_text_from_env"
    );
    assert_eq!(
        progress_line(
            SmokeProgress::Failed,
            7,
            SmokeActionKind::PasteTextFromEnv,
            Some("step_timeout")
        ),
        "P0_SMOKE state=failed step=7 action=paste_text_from_env code=step_timeout"
    );
}
