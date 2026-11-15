use std::path::Path;

use super::{ExportDocument, OntologyError};

pub fn export_session(_session_root: &Path, _session_id: &str) -> Result<ExportDocument, OntologyError> {
    Err(OntologyError::Unsupported(
        "session export is not implemented yet".into(),
    ))
}

pub fn export_journal(_journal_path: &Path) -> Result<ExportDocument, OntologyError> {
    Err(OntologyError::Unsupported(
        "journal export is not implemented yet".into(),
    ))
}
