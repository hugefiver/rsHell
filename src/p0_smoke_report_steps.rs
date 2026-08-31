use rshell_ui::{SmokeFieldStatus, SmokeReport, SmokeStepState};
use serde::Serialize;

use crate::p0_smoke_report_terminal::{
    FrameEvidence, TerminalEvidence, frame_evidence, terminal_evidence,
};

#[derive(Serialize)]
pub(crate) struct P0StepReport {
    index: usize,
    action: &'static str,
    surface: Option<String>,
    connection: Option<String>,
    binding: Option<BindingEvidence>,
    state: &'static str,
    elapsed_ms: u128,
    evidence: CounterEvidence,
    field_status: Option<&'static str>,
}

#[derive(Serialize)]
struct BindingEvidence {
    verified: bool,
    component_verified: bool,
    actual_label: Option<String>,
    connection_id: Option<String>,
    profile_name: Option<String>,
    endpoint: Option<String>,
    pane_id: Option<String>,
    session_id: Option<String>,
    local: bool,
}

#[derive(Serialize)]
struct CounterEvidence {
    tabs: usize,
    panes: usize,
    sessions: usize,
    frames: usize,
    active_session: Option<String>,
    active_session_state: Option<&'static str>,
    active_frame: Option<FrameEvidence>,
    terminal_commands: u64,
    clipboard_writes: u64,
    editor_revisions: u64,
    interaction_revisions: u64,
    import_revisions: u64,
    window_resize: Option<WindowResizeEvidence>,
    terminal: TerminalEvidence,
    imports: ImportEvidence,
}

#[derive(Serialize)]
struct WindowResizeEvidence {
    sequence: u64,
    requested_width: i32,
    requested_height: i32,
    realized_width: i32,
    realized_height: i32,
    expected_layout: &'static str,
    layout: &'static str,
}

#[derive(Serialize)]
struct ImportEvidence {
    sequence: u64,
    completed: bool,
    commit_source: Option<&'static str>,
    expected_groups: usize,
    expected_connections: usize,
    imported_groups: usize,
    imported_connections: usize,
    exact_group: bool,
    exact_connection: bool,
    authentication: Option<&'static str>,
    authentication_matches: bool,
    credential_reference_matches: bool,
    terminal_override_matches: bool,
    pending_preview_count: usize,
    cancel_pending_zero: bool,
    preview: Option<ImportPreviewEvidence>,
    cancel_sequence: u64,
    cancelled_preview_matches: bool,
}

#[derive(Serialize)]
struct ImportPreviewEvidence {
    sequence: u64,
    source: &'static str,
    expected_groups: usize,
    expected_candidates: usize,
    actual_groups: usize,
    actual_candidates: usize,
    actual_group_name: Option<String>,
    actual_candidate_name: Option<String>,
    actual_host: Option<String>,
    authentication: Option<&'static str>,
    credential_reference_present: Option<bool>,
    terminal_override_present: Option<bool>,
    importable: Option<bool>,
    wildcard: Option<bool>,
    exact_group: bool,
    exact_candidate: bool,
    authentication_matches: bool,
    credential_reference_matches: bool,
    terminal_override_matches: bool,
    importable_matches: bool,
    wildcard_matches: bool,
}

pub(crate) fn convert_steps(report: &SmokeReport) -> Vec<P0StepReport> {
    report
        .steps
        .iter()
        .map(|step| P0StepReport {
            index: step.index,
            action: step.action.as_str(),
            surface: step.surface.clone(),
            connection: step.connection.clone(),
            binding: step.binding.as_ref().map(binding_evidence),
            state: step_state(step.state),
            elapsed_ms: step.elapsed.as_millis(),
            evidence: CounterEvidence {
                tabs: step.evidence.tabs,
                panes: step.evidence.panes,
                sessions: step.evidence.sessions,
                frames: step.evidence.frames,
                active_session: step
                    .evidence
                    .active_session
                    .map(|value| value.0.to_string()),
                active_session_state: step.evidence.active_session_state.map(session_state),
                active_frame: step.evidence.latest_frame.map(frame_evidence),
                terminal_commands: step.evidence.terminal_commands,
                clipboard_writes: step.evidence.clipboard_writes,
                editor_revisions: step.evidence.editor_revisions,
                interaction_revisions: step.evidence.interaction_revisions,
                import_revisions: step.evidence.import_revisions,
                window_resize: step
                    .evidence
                    .window_resize
                    .map(|evidence| WindowResizeEvidence {
                        sequence: evidence.sequence,
                        requested_width: evidence.requested_width,
                        requested_height: evidence.requested_height,
                        realized_width: evidence.realized_width,
                        realized_height: evidence.realized_height,
                        expected_layout: evidence.expected_layout.as_str(),
                        layout: evidence.layout.as_str(),
                    }),
                terminal: terminal_evidence(&step.evidence.terminal),
                imports: import_evidence(&step.evidence.imports),
            },
            field_status: step.field_status.map(field_status),
        })
        .collect()
}

fn import_evidence(value: &rshell_ui::SmokeImportEvidence) -> ImportEvidence {
    ImportEvidence {
        sequence: value.sequence,
        completed: value.completed,
        commit_source: value.commit_source.map(import_source),
        expected_groups: value.expected_groups,
        expected_connections: value.expected_connections,
        imported_groups: value.imported_groups,
        imported_connections: value.imported_connections,
        exact_group: value.exact_group,
        exact_connection: value.exact_connection,
        authentication: value.authentication.map(authentication),
        authentication_matches: value.authentication_matches,
        credential_reference_matches: value.credential_reference_matches,
        terminal_override_matches: value.terminal_override_matches,
        pending_preview_count: value.pending_preview_count,
        cancel_pending_zero: value.cancel_pending_zero,
        preview: value.preview.as_ref().map(|preview| ImportPreviewEvidence {
            sequence: preview.sequence,
            source: import_source(preview.source),
            expected_groups: preview.expected_groups,
            expected_candidates: preview.expected_candidates,
            actual_groups: preview.actual_groups,
            actual_candidates: preview.actual_candidates,
            actual_group_name: preview.actual_group_name.clone(),
            actual_candidate_name: preview.actual_candidate_name.clone(),
            actual_host: preview.actual_host.clone(),
            authentication: preview.authentication.map(authentication),
            credential_reference_present: preview.credential_reference_present,
            terminal_override_present: preview.terminal_override_present,
            importable: preview.importable,
            wildcard: preview.wildcard,
            exact_group: preview.exact_group,
            exact_candidate: preview.exact_candidate,
            authentication_matches: preview.authentication_matches,
            credential_reference_matches: preview.credential_reference_matches,
            terminal_override_matches: preview.terminal_override_matches,
            importable_matches: preview.importable_matches,
            wildcard_matches: preview.wildcard_matches,
        }),
        cancel_sequence: value.cancel_sequence,
        cancelled_preview_matches: value.cancelled_preview_matches,
    }
}

const fn import_source(value: rshell_core::ImportSourceKind) -> &'static str {
    match value {
        rshell_core::ImportSourceKind::LegacyRshellJson => "legacy_rshell_json",
        rshell_core::ImportSourceKind::OpenSshConfig => "open_ssh_config",
    }
}

fn binding_evidence(value: &rshell_ui::SmokeBindingEvidence) -> BindingEvidence {
    BindingEvidence {
        verified: value.verified,
        component_verified: value.component_verified,
        actual_label: value.actual_label.clone(),
        connection_id: value.connection_id.map(|id| id.0.to_string()),
        profile_name: value.profile_name.clone(),
        endpoint: value.endpoint.clone(),
        pane_id: value.pane_id.map(|id| id.0.to_string()),
        session_id: value.session_id.map(|id| id.0.to_string()),
        local: value.local,
    }
}

const fn authentication(value: rshell_core::AuthenticationKind) -> &'static str {
    match value {
        rshell_core::AuthenticationKind::Password => "password",
        rshell_core::AuthenticationKind::PublicKey => "public_key",
        rshell_core::AuthenticationKind::Agent => "agent",
        rshell_core::AuthenticationKind::KeyboardInteractive => "keyboard_interactive",
    }
}

const fn step_state(value: SmokeStepState) -> &'static str {
    match value {
        SmokeStepState::Pending => "pending",
        SmokeStepState::Running => "running",
        SmokeStepState::Passed => "passed",
        SmokeStepState::Failed => "failed",
        SmokeStepState::Skipped => "skipped",
    }
}

const fn session_state(value: rshell_core::SessionState) -> &'static str {
    match value {
        rshell_core::SessionState::Created => "created",
        rshell_core::SessionState::Connecting => "connecting",
        rshell_core::SessionState::AwaitingHostKey => "awaiting_host_key",
        rshell_core::SessionState::AwaitingAuthentication => "awaiting_authentication",
        rshell_core::SessionState::Connected => "connected",
        rshell_core::SessionState::Reconnecting => "reconnecting",
        rshell_core::SessionState::Closing => "closing",
        rshell_core::SessionState::Exited => "exited",
        rshell_core::SessionState::Failed => "failed",
        rshell_core::SessionState::Crashed => "crashed",
    }
}

const fn field_status(value: SmokeFieldStatus) -> &'static str {
    match value {
        SmokeFieldStatus::Accepted => "accepted",
        SmokeFieldStatus::Rejected => "rejected",
        SmokeFieldStatus::NotObserved => "not_observed",
    }
}
