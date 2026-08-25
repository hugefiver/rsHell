use std::time::Instant;

use crate::{
    SmokeAction, SmokeFailure, SmokeFieldStatus, SmokeScenarioState, SmokeStepState,
    smoke_driver_observation::SmokeObservation,
    smoke_driver_progress::{SmokeProgress, emit_progress},
    smoke_driver_state::SmokeDriver,
};

impl SmokeDriver {
    pub(crate) fn fail(&mut self, observed: &SmokeObservation, code: &'static str) {
        if self.complete {
            return;
        }
        self.complete = true;
        let now = Instant::now();
        self.update_report(observed, now);
        let failed_step = self.current.as_ref().map(|current| {
            (
                current.index,
                now.duration_since(current.started),
                matches!(&current.action, SmokeAction::SetConnectionField(_)),
                current.action.kind(),
            )
        });
        if let Some((index, _, _, action)) = failed_step {
            emit_progress(SmokeProgress::Failed, index, action, Some(code));
        }
        self.report.mutate(|value| {
            value.state = SmokeScenarioState::Failed;
            value.failure = Some(SmokeFailure {
                step: failed_step.map(|(index, _, _, _)| index),
                code,
            });
            if let Some((index, elapsed, is_field, _)) = failed_step {
                let report = &mut value.steps[index];
                report.state = SmokeStepState::Failed;
                report.elapsed = elapsed;
                report.evidence = observed.counters.clone();
                report.field_status = is_field.then_some(SmokeFieldStatus::Rejected);
            }
            for report in value
                .steps
                .iter_mut()
                .filter(|report| report.state == SmokeStepState::Pending)
            {
                report.state = SmokeStepState::Skipped;
            }
        });
    }
}
