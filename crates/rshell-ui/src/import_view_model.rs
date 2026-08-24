use std::{collections::BTreeSet, fmt, path::PathBuf};

use rshell_core::{
    ConnectionGroup, ImportCandidateId, ImportCandidateView, ImportPreviewId, ImportPreviewView,
    ImportReportView, ImportSourceKind, ImportWarningView, UiCommand,
};

pub struct ImportViewModel {
    preview: Option<ImportPreviewView>,
    selected: BTreeSet<ImportCandidateId>,
    remembered: Option<(ImportSourceKind, PathBuf)>,
    pending: bool,
    cancel_sent: bool,
    report: Option<ImportReportView>,
    error: Option<&'static str>,
}

impl From<ImportPreviewView> for ImportViewModel {
    fn from(preview: ImportPreviewView) -> Self {
        let selected = preview
            .candidates
            .iter()
            .filter(|candidate| candidate.selectable)
            .map(|candidate| candidate.id)
            .collect();
        Self {
            preview: Some(preview),
            selected,
            remembered: None,
            pending: false,
            cancel_sent: false,
            report: None,
            error: None,
        }
    }
}

impl ImportViewModel {
    pub fn empty() -> Self {
        Self {
            preview: None,
            selected: BTreeSet::new(),
            remembered: None,
            pending: false,
            cancel_sent: false,
            report: None,
            error: None,
        }
    }

    pub fn remember_source(&mut self, source: ImportSourceKind, path: PathBuf) {
        self.remembered = Some((source, path));
        self.error = None;
        self.report = None;
    }

    pub fn preview_command(&self) -> Option<UiCommand> {
        let (source, path) = self.remembered.as_ref()?;
        Some(UiCommand::PreviewImport {
            source: *source,
            path: path.clone(),
        })
    }

    pub fn retry_command(&mut self) -> Option<UiCommand> {
        self.clear_preview();
        self.preview_command()
    }

    pub fn accept_preview(&mut self, preview: ImportPreviewView) {
        self.selected = preview
            .candidates
            .iter()
            .filter(|candidate| candidate.selectable)
            .map(|candidate| candidate.id)
            .collect();
        self.preview = Some(preview);
        self.pending = false;
        self.cancel_sent = false;
        self.error = None;
    }

    pub fn candidate(&self, id: ImportCandidateId) -> Option<&ImportCandidateView> {
        self.preview
            .as_ref()?
            .candidates
            .iter()
            .find(|candidate| candidate.id == id)
    }

    pub fn candidates(&self) -> &[ImportCandidateView] {
        self.preview
            .as_ref()
            .map_or(&[], |preview| preview.candidates.as_slice())
    }

    pub fn preview_id(&self) -> Option<ImportPreviewId> {
        self.preview.as_ref().map(|preview| preview.id)
    }

    pub fn source(&self) -> Option<ImportSourceKind> {
        self.preview.as_ref().map(|preview| preview.source)
    }

    pub fn groups(&self) -> &[ConnectionGroup] {
        self.preview
            .as_ref()
            .map_or(&[], |preview| preview.groups.as_slice())
    }

    pub fn is_selected(&self, id: ImportCandidateId) -> bool {
        self.selected.contains(&id)
    }

    pub fn can_commit(&self) -> bool {
        self.preview.is_some() && !self.selected.is_empty() && !self.pending
    }

    pub fn visible_warnings(&self) -> &[ImportWarningView] {
        self.preview
            .as_ref()
            .map_or(&[], |preview| preview.warnings.as_slice())
    }

    pub fn set_selected(&mut self, id: ImportCandidateId, selected: bool) {
        if self.pending
            || !self
                .candidate(id)
                .is_some_and(|candidate| candidate.selectable)
        {
            return;
        }
        if selected {
            self.selected.insert(id);
        } else {
            self.selected.remove(&id);
        }
    }

    pub fn commit_command(&self) -> Option<UiCommand> {
        let preview = self.preview.as_ref()?;
        (!self.pending && !self.selected.is_empty()).then(|| UiCommand::CommitImport {
            preview: preview.id,
            selected: self.selected.clone(),
        })
    }

    pub fn begin_commit(&mut self) -> Option<UiCommand> {
        let command = self.commit_command()?;
        self.pending = true;
        Some(command)
    }

    pub fn cancel_command(&mut self) -> Option<UiCommand> {
        let preview = self.preview.as_ref()?;
        if self.cancel_sent {
            return None;
        }
        self.cancel_sent = true;
        Some(UiCommand::CancelImport {
            preview: preview.id,
        })
    }

    pub fn completed(&mut self, report: ImportReportView) {
        self.clear_preview();
        self.report = Some(report);
    }

    pub fn cancelled(&mut self, preview: ImportPreviewId) {
        if self.preview_id() == Some(preview) {
            self.clear_preview();
        }
    }

    pub fn expired(&mut self) {
        self.clear_preview();
        self.error = Some("Import preview expired; preview the file again");
    }

    pub fn failed(&mut self, context: &'static str) {
        self.pending = false;
        self.error = Some(context);
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }

    pub fn report(&self) -> Option<ImportReportView> {
        self.report
    }

    pub fn error(&self) -> Option<&'static str> {
        self.error
    }

    fn clear_preview(&mut self) {
        self.preview = None;
        self.selected.clear();
        self.pending = false;
        self.cancel_sent = false;
    }
}

impl fmt::Debug for ImportViewModel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportViewModel")
            .field("preview", &self.preview.as_ref().map(|preview| preview.id))
            .field("selected", &self.selected)
            .field(
                "source_path",
                &self.remembered.as_ref().map(|_| "[REDACTED]"),
            )
            .field("pending", &self.pending)
            .field("report", &self.report)
            .field("error", &self.error)
            .finish()
    }
}
