use std::{fmt, path::PathBuf, time::Duration};

pub use crate::smoke_driver_action_kind::SmokeActionKind;
pub use crate::smoke_driver_actions::{SmokeAction, SmokeConnectionField, SmokeImportExpectation};

pub const SMOKE_SCENARIO_VERSION: u16 = 2;
pub const DEFAULT_SMOKE_STEP_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_SMOKE_SCENARIO_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone)]
pub struct SmokeScenario {
    pub version: u16,
    pub run_nonce: String,
    pub actions: Vec<SmokeStep>,
    pub step_timeout: Duration,
    pub scenario_timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct SmokeStep {
    pub action: SmokeAction,
    pub surface: Option<String>,
    pub connection: Option<String>,
}

impl SmokeStep {
    pub fn new(action: SmokeAction) -> Self {
        Self {
            action,
            surface: None,
            connection: None,
        }
    }
}

impl SmokeScenario {
    pub fn new(actions: Vec<SmokeAction>) -> Self {
        Self::with_steps(
            "unit-smoke",
            actions.into_iter().map(SmokeStep::new).collect(),
        )
    }

    pub fn with_steps(run_nonce: impl Into<String>, actions: Vec<SmokeStep>) -> Self {
        Self {
            version: SMOKE_SCENARIO_VERSION,
            run_nonce: run_nonce.into(),
            actions,
            step_timeout: DEFAULT_SMOKE_STEP_TIMEOUT,
            scenario_timeout: DEFAULT_SMOKE_SCENARIO_TIMEOUT,
        }
    }

    pub fn validate(&self) -> Result<(), SmokeScenarioError> {
        if self.version != SMOKE_SCENARIO_VERSION {
            return Err(SmokeScenarioError::UnsupportedVersion(self.version));
        }
        if self.actions.is_empty() {
            return Err(SmokeScenarioError::Empty);
        }
        if !valid_label(&self.run_nonce) {
            return Err(SmokeScenarioError::InvalidRunNonce);
        }
        if self.step_timeout.is_zero() || self.scenario_timeout.is_zero() {
            return Err(SmokeScenarioError::ZeroTimeout);
        }
        let close = self
            .actions
            .iter()
            .enumerate()
            .filter(|(_, step)| matches!(step.action, SmokeAction::CloseAll))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if close.as_slice() != [self.actions.len() - 1] {
            return Err(SmokeScenarioError::CloseAllNotFinal);
        }
        let mut checkpoint_ids = std::collections::BTreeSet::new();
        for step in &self.actions {
            match &step.action {
                SmokeAction::VisualCheckpoint(checkpoint) => {
                    if !checkpoint.validate() {
                        return Err(SmokeScenarioError::InvalidVisualCheckpoint);
                    }
                    if !checkpoint_ids.insert(checkpoint.id.as_str()) {
                        return Err(SmokeScenarioError::DuplicateCheckpointId);
                    }
                }
                SmokeAction::ResizeWindow {
                    width,
                    height,
                    expected_mode,
                } if !crate::smoke_driver_visual_matrix::supported_dimensions(*width, *height)
                    || crate::ShellLayout::for_width(*width).mode != *expected_mode =>
                {
                    return Err(SmokeScenarioError::InvalidWindowResize);
                }
                _ => {}
            }
        }
        if self.actions.iter().any(|step| {
            step.surface
                .as_deref()
                .is_some_and(|value| !valid_label(value))
                || step
                    .connection
                    .as_deref()
                    .is_some_and(|value| !valid_label(value))
        }) {
            return Err(SmokeScenarioError::InvalidLabel);
        }
        Ok(())
    }
}

impl fmt::Debug for SmokeScenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SmokeScenario")
            .field("version", &self.version)
            .field("run_nonce", &self.run_nonce)
            .field("actions", &self.actions)
            .field("step_timeout", &self.step_timeout)
            .field("scenario_timeout", &self.scenario_timeout)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeScenarioError {
    UnsupportedVersion(u16),
    Empty,
    ZeroTimeout,
    InvalidRunNonce,
    InvalidLabel,
    CloseAllNotFinal,
    DuplicateCheckpointId,
    InvalidVisualCheckpoint,
    InvalidWindowResize,
}

fn valid_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|character| !character.is_control() && character != '\0')
}

#[derive(Debug, Clone)]
pub struct SmokeDriverInit {
    pub scenario: SmokeScenario,
    pub png_path: Option<PathBuf>,
}

impl SmokeDriverInit {
    pub fn new(scenario: SmokeScenario) -> Self {
        Self {
            scenario,
            png_path: None,
        }
    }

    pub fn with_png_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.png_path = Some(path.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_and_default_timeouts_are_stable() {
        let scenario = SmokeScenario::new(vec![SmokeAction::NewTab, SmokeAction::CloseAll]);
        assert_eq!(scenario.version, SMOKE_SCENARIO_VERSION);
        assert_eq!(scenario.step_timeout, Duration::from_secs(10));
        assert_eq!(scenario.scenario_timeout, Duration::from_secs(120));
        assert_eq!(
            SmokeScenario {
                version: 1,
                ..scenario.clone()
            }
            .validate(),
            Err(SmokeScenarioError::UnsupportedVersion(1))
        );
        let mut zero_timeout = scenario;
        zero_timeout.step_timeout = Duration::ZERO;
        assert_eq!(
            zero_timeout.validate(),
            Err(SmokeScenarioError::ZeroTimeout)
        );
    }
}
