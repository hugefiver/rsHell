use std::collections::BTreeSet;
use std::path::PathBuf;

use rshell_core::{
    ImportCandidateId, ImportCandidateView, ImportPreviewId, ImportPreviewView, ImportSourceKind,
    ImportWarningView, UiCommand,
};
use rshell_ui::ImportViewModel;

#[test]
fn import_preview_cannot_commit_wildcards_and_displays_all_warnings_before_confirm() {
    let production = candidate("production", true, Vec::new());
    let wildcard_warning = warning("wildcard_template", "Wildcard templates cannot be imported");
    let wildcard = candidate("*.corp", false, vec![wildcard_warning.clone()]);
    let production_id = production.id;
    let wildcard_id = wildcard.id;
    let preview = ImportPreviewView {
        id: ImportPreviewId::new(),
        source: ImportSourceKind::OpenSshConfig,
        groups: Vec::new(),
        candidates: vec![production, wildcard],
        warnings: vec![wildcard_warning.clone()],
    };

    let vm = ImportViewModel::from(preview);

    assert!(!vm.candidate(wildcard_id).expect("wildcard row").selectable);
    assert_eq!(vm.visible_warnings(), &[wildcard_warning]);
    match vm.commit_command().expect("selected candidate") {
        UiCommand::CommitImport { selected, .. } => {
            assert_eq!(selected, BTreeSet::from([production_id]));
            assert!(!selected.contains(&wildcard_id));
        }
        command => panic!("unexpected command: {command:?}"),
    }
}

#[test]
fn retry_always_repreviews_remembered_source_and_active_preview_cancels_once() {
    let first = candidate("production", true, Vec::new());
    let preview = ImportPreviewView {
        id: ImportPreviewId::new(),
        source: ImportSourceKind::LegacyRshellJson,
        groups: Vec::new(),
        candidates: vec![first],
        warnings: Vec::new(),
    };
    let old_id = preview.id;
    let mut vm = ImportViewModel::from(preview);
    vm.remember_source(
        ImportSourceKind::LegacyRshellJson,
        PathBuf::from("private-user-path.json"),
    );
    assert!(
        matches!(vm.begin_commit(), Some(UiCommand::CommitImport { preview, .. }) if preview == old_id)
    );
    vm.failed("import operation failed");

    let retry = vm.retry_command().expect("fresh preview command");
    assert!(matches!(
        retry,
        UiCommand::PreviewImport {
            source: ImportSourceKind::LegacyRshellJson,
            ..
        }
    ));
    assert!(!format!("{vm:?}").contains("private-user-path"));
    assert!(
        vm.commit_command().is_none(),
        "consumed ID must not be committed again"
    );

    let next = ImportPreviewView {
        id: ImportPreviewId::new(),
        source: ImportSourceKind::OpenSshConfig,
        groups: Vec::new(),
        candidates: vec![candidate("next", true, Vec::new())],
        warnings: Vec::new(),
    };
    vm.accept_preview(next);
    assert!(matches!(
        vm.cancel_command(),
        Some(UiCommand::CancelImport { .. })
    ));
    assert!(
        vm.cancel_command().is_none(),
        "close cancellation is exactly once"
    );
}

#[test]
fn expired_preview_clears_dialog_state() {
    let preview = ImportPreviewView {
        id: ImportPreviewId::new(),
        source: ImportSourceKind::LegacyRshellJson,
        groups: Vec::new(),
        candidates: vec![candidate("preview", true, Vec::new())],
        warnings: Vec::new(),
    };
    let mut vm = ImportViewModel::from(preview);
    assert!(vm.begin_commit().is_some());

    vm.expired();

    assert!(vm.preview_id().is_none());
    assert!(!vm.is_pending());
    assert!(vm.commit_command().is_none());
    assert!(vm.cancel_command().is_none());
    assert_eq!(
        vm.error(),
        Some("Import preview expired; preview the file again")
    );
}

fn candidate(
    name: &str,
    selectable: bool,
    warnings: Vec<ImportWarningView>,
) -> ImportCandidateView {
    ImportCandidateView {
        id: ImportCandidateId::new(),
        name: name.into(),
        host: format!("{name}.example.test"),
        port: 22,
        username: "operator".into(),
        source_label: name.into(),
        has_secret: name == "production",
        selectable,
        authentication: rshell_core::AuthenticationKind::Agent,
        credential_reference_present: false,
        terminal_override_present: false,
        importable: selectable,
        wildcard: !selectable,
        warnings,
    }
}

fn warning(code: &str, message: &str) -> ImportWarningView {
    ImportWarningView {
        code: code.into(),
        message: message.into(),
    }
}
