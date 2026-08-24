use std::{path::PathBuf, rc::Rc};

use rshell_core::{
    AppFailure, ImportCandidateId, ImportPreviewId, ImportPreviewView, ImportReportView,
    ImportSourceKind, UiCommand, UiPortError,
};
use rshell_platform::{FileSelectionResult, FileSelectionService};

pub struct ImportDialogInit {
    pub file_selection: Rc<dyn FileSelectionService>,
}

impl std::fmt::Debug for ImportDialogInit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImportDialogInit")
            .field("file_selection", &"FileSelectionService")
            .finish()
    }
}

#[derive(Debug)]
pub enum ImportDialogMsg {
    Open,
    Choose(ImportSourceKind),
    FileSelected(u64, ImportSourceKind, FileSelectionResult),
    PreviewPath(ImportSourceKind, PathBuf),
    Preview(ImportPreviewView),
    Toggle(ImportCandidateId, bool),
    Commit,
    Retry,
    Close,
    Completed(ImportReportView),
    Cancelled(ImportPreviewId),
    OperationFailed(AppFailure),
    CommandRejected(UiPortError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportDialogState {
    pub visible: bool,
    pub pending: bool,
    pub preview_ready: bool,
    pub has_error: bool,
    pub revision: u64,
}

#[derive(Debug)]
pub enum ImportDialogOutput {
    Command(Box<UiCommand>),
    Closed,
    StateChanged(ImportDialogState),
}
