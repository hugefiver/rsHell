use rshell_core::{
    ImportPreviewId, ImportPreviewView, ImportReportView, ImportSourceKind, PaneId, SessionId,
};

use crate::{
    MainWindow, SmokeImportEvidence, SmokeImportExpectation, SmokeImportPreviewEvidence,
    SmokeReconnectEvidence,
};

pub(crate) struct PendingReconnect {
    pane: PaneId,
    old_session: SessionId,
}

#[derive(Clone, Copy)]
pub(crate) struct ImportBaseline {
    pub(crate) groups: usize,
    pub(crate) connections: usize,
}

pub(crate) struct PendingImportPreview {
    id: ImportPreviewId,
    source: ImportSourceKind,
}

impl MainWindow {
    pub(crate) fn prepare_smoke_import(
        &mut self,
        source: ImportSourceKind,
        expected: Option<SmokeImportExpectation>,
    ) {
        self.smoke_state.import_source = Some(source);
        self.smoke_state.import_expectation = expected;
        self.smoke_state.import_baseline = Some(ImportBaseline {
            groups: self.view_model.catalog.groups.len(),
            connections: self.view_model.catalog.connections.len(),
        });
    }

    pub(crate) fn observe_smoke_import_preview(&mut self, preview: &ImportPreviewView) {
        let Some(expected) = self.smoke_state.import_expectation.clone() else {
            return;
        };
        if self.smoke_state.import_source != Some(preview.source) {
            return;
        }
        let candidate = (preview.candidates.len() == 1).then(|| &preview.candidates[0]);
        let actual_group_name = (preview.groups.len() == 1).then(|| preview.groups[0].name.clone());
        let exact_group = preview.groups.len() == expected.groups
            && if expected.group_name.is_empty() {
                preview.groups.is_empty()
            } else {
                actual_group_name.as_deref() == Some(expected.group_name.as_str())
            };
        let exact_candidate = candidate.is_some_and(|candidate| {
            candidate.name == expected.connection_name && candidate.host == expected.host
        }) && preview.candidates.len() == expected.connections;
        let sequence = self.next_smoke_evidence_sequence();
        self.smoke_state.imports.preview = Some(SmokeImportPreviewEvidence {
            sequence,
            source: preview.source,
            expected_groups: expected.groups,
            expected_candidates: expected.connections,
            actual_groups: preview.groups.len(),
            actual_candidates: preview.candidates.len(),
            actual_group_name,
            actual_candidate_name: candidate.map(|candidate| candidate.name.clone()),
            actual_host: candidate.map(|candidate| candidate.host.clone()),
            authentication: candidate.map(|candidate| candidate.authentication),
            credential_reference_present: candidate
                .map(|candidate| candidate.credential_reference_present),
            terminal_override_present: candidate
                .map(|candidate| candidate.terminal_override_present),
            importable: candidate.map(|candidate| candidate.importable),
            wildcard: candidate.map(|candidate| candidate.wildcard),
            exact_group,
            exact_candidate,
            authentication_matches: candidate
                .is_some_and(|candidate| candidate.authentication == expected.authentication),
            credential_reference_matches: candidate.is_some_and(|candidate| {
                candidate.credential_reference_present == expected.credential_reference_present
            }),
            terminal_override_matches: candidate.is_some_and(|candidate| {
                candidate.terminal_override_present == expected.terminal_override_present
            }),
            importable_matches: candidate
                .is_some_and(|candidate| candidate.importable == expected.importable),
            wildcard_matches: candidate
                .is_some_and(|candidate| candidate.wildcard == expected.wildcard),
        });
        self.smoke_state.pending_import_preview = Some(PendingImportPreview {
            id: preview.id,
            source: preview.source,
        });
    }

    pub(crate) fn prepare_smoke_reconnect(&mut self) -> Result<(), &'static str> {
        let tab = self
            .view_model
            .workspace
            .active_tab()
            .ok_or("no_active_tab")?;
        let session = tab
            .pane_tree
            .session_id(tab.active_pane)
            .map_err(|_| "active_pane_not_found")?
            .ok_or("active_session_not_found")?;
        self.smoke_state.pending_reconnect = Some(PendingReconnect {
            pane: tab.active_pane,
            old_session: session,
        });
        self.smoke_state.terminal.reconnect = Some(SmokeReconnectEvidence {
            sequence: 0,
            old_session: session,
            new_session: None,
            old_session_absent: false,
        });
        Ok(())
    }

    pub(crate) fn observe_smoke_import_completed(&mut self, report: ImportReportView) {
        let Some(expected) = self.smoke_state.import_expectation.clone() else {
            return;
        };
        let baseline = self.smoke_state.import_baseline;
        let group_exists = self
            .view_model
            .catalog
            .groups
            .values()
            .any(|group| group.name == expected.group_name);
        let profile = self
            .view_model
            .catalog
            .connections
            .values()
            .find(|profile| profile.name == expected.connection_name)
            .cloned();
        let exact_group = group_exists
            && baseline.is_some_and(|value| {
                self.view_model.catalog.groups.len() == value.groups + expected.groups
            });
        let exact_connection = profile.is_some()
            && baseline.is_some_and(|value| {
                self.view_model.catalog.connections.len()
                    == value.connections + expected.connections
            });
        let sequence = self.next_smoke_evidence_sequence();
        self.smoke_state.imports = SmokeImportEvidence {
            sequence,
            completed: true,
            commit_source: self
                .smoke_state
                .pending_import_preview
                .as_ref()
                .map(|preview| preview.source),
            expected_groups: expected.groups,
            expected_connections: expected.connections,
            imported_groups: report.imported_groups,
            imported_connections: report.imported_connections,
            exact_group,
            exact_connection,
            authentication: profile.as_ref().map(|profile| profile.authentication),
            authentication_matches: profile
                .as_ref()
                .is_some_and(|profile| profile.authentication == expected.authentication),
            credential_reference_matches: profile.as_ref().is_some_and(|profile| {
                profile.credential_ref.is_some() == expected.credential_reference_present
            }),
            terminal_override_matches: profile.as_ref().is_some_and(|profile| {
                (profile.terminal_overrides != Default::default())
                    == expected.terminal_override_present
            }),
            pending_preview_count: self.view_model.pending_imports.len(),
            cancel_pending_zero: false,
            preview: self.smoke_state.imports.preview.clone(),
            cancel_sequence: self.smoke_state.imports.cancel_sequence,
            cancelled_preview_matches: false,
        };
    }

    pub(crate) fn observe_smoke_import_cancelled(&mut self, preview: ImportPreviewId) {
        let matches = self
            .smoke_state
            .pending_import_preview
            .as_ref()
            .is_some_and(|pending| {
                pending.id == preview && pending.source == ImportSourceKind::OpenSshConfig
            });
        self.smoke_state.imports.pending_preview_count = self.view_model.pending_imports.len();
        self.smoke_state.imports.cancel_pending_zero = self.view_model.pending_imports.is_empty();
        self.smoke_state.imports.cancelled_preview_matches = matches;
        if matches && self.smoke_state.imports.cancel_pending_zero {
            self.smoke_state.imports.cancel_sequence = self.next_smoke_evidence_sequence();
            self.smoke_state.pending_import_preview = None;
        }
    }

    pub(crate) fn refresh_smoke_reconnect(&mut self) {
        let Some(pending) = self.smoke_state.pending_reconnect.as_ref() else {
            return;
        };
        let pane = pending.pane;
        let old_session = pending.old_session;
        let new_session = self
            .view_model
            .workspace
            .tabs
            .iter()
            .find_map(|tab| tab.pane_tree.session_id(pane).ok().flatten());
        let old_absent = self
            .view_model
            .workspace
            .tabs
            .iter()
            .flat_map(|tab| tab.pane_tree.session_ids())
            .all(|session| session != old_session);
        let new_session = new_session.filter(|session| *session != old_session);
        let completed = new_session.is_some() && old_absent;
        let previous_sequence = self
            .smoke_state
            .terminal
            .reconnect
            .as_ref()
            .map_or(0, |evidence| evidence.sequence);
        let sequence = if completed && previous_sequence == 0 {
            self.next_smoke_evidence_sequence()
        } else {
            previous_sequence
        };
        self.smoke_state.terminal.reconnect = Some(SmokeReconnectEvidence {
            sequence,
            old_session,
            new_session,
            old_session_absent: old_absent,
        });
    }
}
