use rshell_core::{ImportSourceKind, SessionState};

use crate::{
    SmokeCounters, SmokeImportExpectation, smoke_driver_observation::SmokeObservation,
    smoke_driver_sequences::import_preview_sequence,
};

pub(crate) fn host_key_outcome_complete(state: Option<SessionState>, accepted: bool) -> bool {
    if accepted {
        matches!(
            state,
            Some(SessionState::AwaitingAuthentication | SessionState::Connected)
        )
    } else {
        matches!(
            state,
            Some(SessionState::Exited | SessionState::Failed | SessionState::Crashed)
        )
    }
}

pub(crate) fn preview_complete(
    source: ImportSourceKind,
    expected: &SmokeImportExpectation,
    before: &SmokeCounters,
    now: &SmokeObservation,
) -> bool {
    now.import_preview_ready
        && now.counters.import_revisions > before.import_revisions
        && now
            .counters
            .imports
            .preview
            .as_ref()
            .is_some_and(|evidence| {
                evidence.sequence > import_preview_sequence(before)
                    && evidence.source == source
                    && evidence.expected_groups == expected.groups
                    && evidence.expected_candidates == expected.connections
                    && evidence.actual_groups == expected.groups
                    && evidence.actual_candidates == expected.connections
                    && evidence.exact_group
                    && evidence.exact_candidate
                    && evidence.authentication_matches
                    && evidence.credential_reference_matches
                    && evidence.terminal_override_matches
                    && evidence.importable_matches
                    && evidence.wildcard_matches
            })
}

pub(crate) fn commit_complete(before: &SmokeCounters, now: &SmokeObservation) -> bool {
    let evidence = &now.counters.imports;
    now.counters.import_completions > before.import_completions
        && evidence.sequence > before.imports.sequence
        && evidence.completed
        && evidence.commit_source == Some(ImportSourceKind::LegacyRshellJson)
        && evidence.imported_groups == evidence.expected_groups
        && evidence.imported_connections == evidence.expected_connections
        && evidence.exact_group
        && evidence.exact_connection
        && evidence.authentication_matches
        && evidence.credential_reference_matches
        && evidence.terminal_override_matches
}
