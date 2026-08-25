use std::time::Instant;

use rshell_core::{ConnectionId, InteractionId};

use crate::{
    SmokeAction, SmokeCounters, SmokeDriverInit, SmokeFailure, SmokeFieldStatus, SmokeReportHandle,
    SmokeScenarioState, SmokeStepState,
    smoke_driver_completion::{CompletionContext, action_is_complete},
    smoke_driver_observation::SmokeObservation,
    smoke_driver_progress::{SmokeProgress, emit_progress},
    smoke_driver_routing::action_route_ready,
};

pub(crate) enum SmokeDecision {
    Route(SmokeAction),
    Quit,
}

pub(crate) struct CurrentStep {
    pub(crate) index: usize,
    pub(crate) action: SmokeAction,
    pub(crate) started: Instant,
    baseline: SmokeCounters,
    routed: bool,
    auth_route: Option<AuthRoute>,
}

#[derive(Clone, Copy)]
struct AuthRoute {
    interaction: InteractionId,
    submits: bool,
}

pub(crate) struct SmokeDriver {
    scenario: crate::SmokeScenario,
    pub(crate) report: SmokeReportHandle,
    started: Instant,
    pub(crate) current: Option<CurrentStep>,
    next: usize,
    selected_connection: Option<ConnectionId>,
    selection_target: Option<ConnectionId>,
    last_submitted_interaction: Option<InteractionId>,
    shutdown_sent: bool,
    pub(crate) complete: bool,
}

impl SmokeDriver {
    pub(crate) fn new(init: SmokeDriverInit, report: SmokeReportHandle) -> Self {
        let scenario = init.scenario;
        let now = Instant::now();
        let valid = scenario.validate().is_ok();
        report.mutate(|value| {
            value.state = if valid {
                SmokeScenarioState::Running
            } else {
                SmokeScenarioState::Failed
            };
            if !valid {
                value.failure = Some(SmokeFailure {
                    step: None,
                    code: "invalid_scenario",
                });
                for step in &mut value.steps {
                    step.state = SmokeStepState::Skipped;
                }
            }
        });
        Self {
            scenario,
            report,
            started: now,
            current: None,
            next: 0,
            selected_connection: None,
            selection_target: None,
            last_submitted_interaction: None,
            shutdown_sent: false,
            complete: !valid,
        }
    }

    pub(crate) fn selected_connection(&self) -> Option<ConnectionId> {
        self.selected_connection
    }
    pub(crate) fn record_shutdown_sent(&mut self) {
        self.shutdown_sent = true;
    }

    pub(crate) fn shutdown_sent(&self) -> bool {
        self.shutdown_sent
    }

    pub(crate) fn defer_current_route(&mut self) {
        if let Some(current) = &mut self.current {
            current.routed = false;
        }
    }

    pub(crate) fn record_selection_target(&mut self, connection: ConnectionId) {
        self.selection_target = Some(connection);
    }

    pub(crate) fn record_auth_route(&mut self, interaction: InteractionId, submits: bool) {
        if let Some(current) = &mut self.current {
            current.auth_route = Some(AuthRoute {
                interaction,
                submits,
            });
        }
    }

    pub(crate) fn current_binding_request(
        &self,
    ) -> Option<crate::smoke_driver_observation::SmokeBindingRequest> {
        let current = self.current.as_ref()?;
        let step = self.scenario.actions.get(current.index)?;
        Some(crate::smoke_driver_observation::SmokeBindingRequest {
            action: current.action.clone(),
            surface: step.surface.clone(),
            connection: step.connection.clone(),
        })
    }

    pub(crate) fn tick(
        &mut self,
        observed: &SmokeObservation,
        frame_contains: impl Fn(&str) -> bool,
    ) -> Option<SmokeDecision> {
        self.update_report(observed, Instant::now());
        if self.complete {
            return None;
        }
        let now = Instant::now();
        if now.duration_since(self.started) > self.scenario.scenario_timeout {
            self.fail(observed, "scenario_timeout");
            return Some(SmokeDecision::Quit);
        }
        if self.current.is_some() {
            let is_complete = {
                let current = self.current.as_ref().expect("current step checked");
                let context = CompletionContext {
                    before: &current.baseline,
                    now: observed,
                    selected_connection: self.selected_connection,
                    selection_target: self.selection_target,
                    shutdown_sent: self.shutdown_sent,
                    auth_interaction: current.auth_route.map(|route| route.interaction),
                    auth_submits: current.auth_route.is_some_and(|route| route.submits),
                    binding_required: self
                        .scenario
                        .actions
                        .get(current.index)
                        .is_some_and(|step| step.surface.is_some()),
                };
                action_is_complete(&current.action, &context, &frame_contains)
            };
            if is_complete {
                let current = self.current.as_ref().expect("current step checked");
                let action = current.action.clone();
                self.pass(observed, now);
                if matches!(action, SmokeAction::CloseAll) {
                    self.complete = true;
                    self.report.mutate(|report| {
                        report.state = SmokeScenarioState::Passed;
                        for step in report
                            .steps
                            .iter_mut()
                            .filter(|step| step.state == SmokeStepState::Pending)
                        {
                            step.state = SmokeStepState::Skipped;
                        }
                    });
                    return Some(SmokeDecision::Quit);
                }
            } else if now
                .duration_since(self.current.as_ref().expect("current step checked").started)
                > self.scenario.step_timeout
            {
                self.fail(observed, "step_timeout");
                return Some(SmokeDecision::Quit);
            } else {
                let current = self.current.as_mut().expect("current step checked");
                if !current.routed
                    && action_route_ready(
                        &current.action,
                        observed,
                        self.last_submitted_interaction,
                    )
                {
                    current.routed = true;
                    return Some(SmokeDecision::Route(current.action.clone()));
                }
                return None;
            }
        }
        if self.next == self.scenario.actions.len() {
            self.complete = true;
            self.report
                .mutate(|value| value.state = SmokeScenarioState::Passed);
            return None;
        }
        let index = self.next;
        self.next += 1;
        let action = self.scenario.actions[index].action.clone();
        let routed = !matches!(
            action,
            SmokeAction::WaitWindowRealized | SmokeAction::WaitFrameContains(_)
        ) && action_route_ready(&action, observed, self.last_submitted_interaction);
        self.current = Some(CurrentStep {
            index,
            action: action.clone(),
            started: now,
            baseline: observed.counters.clone(),
            routed,
            auth_route: None,
        });
        self.report
            .mutate(|value| value.steps[index].state = SmokeStepState::Running);
        emit_progress(SmokeProgress::Started, index, action.kind(), None);
        routed.then_some(SmokeDecision::Route(action))
    }

    fn pass(&mut self, observed: &SmokeObservation, now: Instant) {
        let current = self.current.take().expect("current step checked");
        emit_progress(
            SmokeProgress::Passed,
            current.index,
            current.action.kind(),
            None,
        );
        if let Some(route) = current.auth_route.filter(|route| route.submits) {
            self.last_submitted_interaction = Some(route.interaction);
        }
        if matches!(current.action, SmokeAction::SelectConnection(_)) {
            self.selected_connection = self.selection_target.take();
        }
        self.report.mutate(|value| {
            let report = &mut value.steps[current.index];
            report.state = SmokeStepState::Passed;
            report.elapsed = now.duration_since(current.started);
            report.evidence = observed.counters.clone();
            report.binding = observed.binding.clone();
            report.field_status = matches!(current.action, SmokeAction::SetConnectionField(_))
                .then_some(SmokeFieldStatus::Accepted);
        });
    }

    pub(crate) fn update_report(&self, observed: &SmokeObservation, now: Instant) {
        let running_step = self
            .current
            .as_ref()
            .map(|current| (current.index, now.duration_since(current.started)));
        self.report.mutate(|value| {
            value.elapsed = now.duration_since(self.started);
            value.counters = observed.counters.clone();
            if let Some((index, elapsed)) = running_step {
                let step = &mut value.steps[index];
                step.elapsed = elapsed;
                step.evidence = observed.counters.clone();
                step.binding = observed.binding.clone();
            }
        });
    }
}
