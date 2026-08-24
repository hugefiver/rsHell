use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rshell_ui::{SMOKE_SCENARIO_VERSION, SmokeScenario, SmokeStep};
use serde::Deserialize;

use crate::{
    p0_smoke_actions::RawAction,
    p0_smoke_evidence::{ExternalObservationRequest, SmokeSurface},
};

pub(crate) struct ParsedScenario {
    pub(crate) scenario: SmokeScenario,
    pub(crate) external_observations: Vec<ExternalObservationRequest>,
    pub(crate) secret_environment: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScenarioError {
    Read,
    Parse,
    Invalid,
}

pub(crate) fn read(path: &Path) -> Result<ParsedScenario, ScenarioError> {
    fs::read_to_string(path)
        .map_err(|_| ScenarioError::Read)
        .and_then(|json| parse(&json))
}

pub(crate) fn parse(json: &str) -> Result<ParsedScenario, ScenarioError> {
    let raw: RawScenario = serde_json::from_str(json).map_err(|_| ScenarioError::Parse)?;
    if raw.version != SMOKE_SCENARIO_VERSION {
        return Err(ScenarioError::Invalid);
    }
    let decoded = raw
        .actions
        .into_iter()
        .map(RawStep::decode)
        .collect::<Result<Vec<_>, _>>()?;
    let secret_environment = decoded
        .iter()
        .filter_map(|step| step.action.secret_environment_name())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let actions = decoded
        .into_iter()
        .map(DecodedStep::into_step)
        .collect::<Result<Vec<_>, _>>()?;
    let mut scenario = SmokeScenario::with_steps(raw.run_nonce.clone(), actions);
    scenario.step_timeout = raw
        .step_timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(scenario.step_timeout);
    scenario.scenario_timeout = raw
        .scenario_timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(scenario.scenario_timeout);
    scenario.validate().map_err(|_| ScenarioError::Invalid)?;
    Ok(ParsedScenario {
        scenario,
        external_observations: raw
            .external_observations
            .into_iter()
            .map(|request| ExternalObservationRequest {
                surface: request.surface,
                path: request.path,
                run_nonce: raw.run_nonce.clone(),
                fixture: request.fixture,
                connection: request.connection,
                endpoint: request.endpoint,
            })
            .collect(),
        secret_environment,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScenario {
    version: u16,
    run_nonce: String,
    actions: Vec<RawStep>,
    #[serde(default)]
    step_timeout_ms: Option<u64>,
    #[serde(default)]
    scenario_timeout_ms: Option<u64>,
    #[serde(default)]
    external_observations: Vec<RawExternalObservation>,
}

#[derive(Deserialize)]
struct RawStep {
    #[serde(flatten)]
    action: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    surface: Option<SmokeSurface>,
    #[serde(default)]
    #[serde(rename = "connection_label")]
    connection: Option<String>,
}

impl RawStep {
    fn decode(self) -> Result<DecodedStep, ScenarioError> {
        let action =
            serde_json::from_value(serde_json::Value::Object(self.action.into_iter().collect()))
                .map_err(|_| ScenarioError::Parse)?;
        Ok(DecodedStep {
            action,
            surface: self.surface,
            connection: self.connection,
        })
    }
}

struct DecodedStep {
    action: RawAction,
    surface: Option<SmokeSurface>,
    connection: Option<String>,
}

impl DecodedStep {
    fn into_step(self) -> Result<SmokeStep, ScenarioError> {
        Ok(SmokeStep {
            action: self.action.into_action()?,
            surface: self.surface.map(|surface| surface.as_str().to_owned()),
            connection: self.connection,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExternalObservation {
    surface: SmokeSurface,
    path: PathBuf,
    fixture: String,
    connection: String,
    endpoint: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn static_fixture_parses_every_ui_action_name() {
        let fixture = include_str!("../tests/fixtures/smoke/p0-scenario.json");
        let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        for (index, action) in value["actions"].as_array().unwrap().iter().enumerate() {
            serde_json::from_value::<crate::p0_smoke_actions::RawAction>(action.clone())
                .unwrap_or_else(|error| {
                    panic!("static fixture action {index} must parse: {error}")
                });
        }
        let scenario = super::parse(fixture).expect("fixture must remain a valid scenario");
        assert!(scenario.scenario.actions.len() >= rshell_ui::SmokeAction::ALL.len());
    }

    #[test]
    fn secret_actions_accept_only_environment_variable_names() {
        let json = r#"{"version":1,"run_nonce":"unit","actions":[{"action":"paste_text_from_env","env_var":"literal secret","effect_marker":"effect"},{"action":"close_all"}]}"#;
        assert!(matches!(
            super::parse(json),
            Err(super::ScenarioError::Invalid)
        ));
    }

    #[test]
    fn collects_only_secret_environment_names_for_root_state_scan() {
        let parsed = super::parse(
            r#"{"version":1,"run_nonce":"unit","actions":[
                {"action":"respond_auth","prompt":0,"env_var":"AUTH"},
                {"action":"paste_text_from_env","env_var":"PASTE","effect_marker":"effect"},
                {"action":"set_connection_field","field":{"kind":"secret_from_env","env_var":"AUTH"}},
                {"action":"close_all"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(parsed.secret_environment, ["AUTH", "PASTE"]);
    }

    #[test]
    fn close_all_is_required_exactly_once_and_last() {
        for json in [
            r#"{"version":1,"run_nonce":"unit","actions":[{"action":"new_tab"}]}"#,
            r#"{"version":1,"run_nonce":"unit","actions":[{"action":"close_all"},{"action":"new_tab"}]}"#,
            r#"{"version":1,"run_nonce":"unit","actions":[{"action":"close_all"},{"action":"close_all"}]}"#,
        ] {
            assert_eq!(
                super::parse(json).err(),
                Some(super::ScenarioError::Invalid),
                "CloseAll must occur exactly once as the final action"
            );
        }
    }

    #[test]
    fn scenario_accepts_non_secret_surface_binding_and_run_nonce() {
        let json = r#"{
            "version":1,
            "run_nonce":"review-run-123",
            "actions":[
                {"action":"wait_window_realized","surface":"gtk","connection_label":"unit-window"},
                {"action":"close_all","surface":"cleanup"}
            ]
        }"#;
        assert!(
            super::parse(json).is_ok(),
            "scenario steps must carry a run-bound surface label"
        );
    }
}
