//! Read-only semantic projection over 1Agent journals and sessions.
//!
//! The journal remains the source of truth. This module is a deterministic
//! exporter/view, not a runtime dependency.

use std::path::Path;

use serde_json::Value;
use thiserror::Error;

pub mod export;
pub mod ids;
pub mod types;
pub mod validate;

pub use export::{export_goal_log, export_journal, export_session};
pub use types::{BindingCoverage, binding_coverage, builtin_binding_coverage};
pub use validate::{ValidationReport, validate_document};

pub const ONTOLOGY_VERSION: &str = "0.98-draft";
pub const BINDING_VERSION: &str = "2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDocument {
    pub value: Value,
    pub sha256: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OntologyError {
    #[error("ontology io: {0}")]
    Io(String),
    #[error("ontology json: {0}")]
    Json(String),
    #[error("ontology session: {0}")]
    Session(String),
    #[error("ontology journal: {0}")]
    Journal(String),
    #[error("ontology validation: {0}")]
    Validation(String),
    #[error("ontology unsupported: {0}")]
    Unsupported(String),
}

pub fn builtin_types() -> Value {
    types::builtin_types()
}

pub fn builtin_bindings() -> Value {
    types::builtin_bindings()
}

pub fn registry_sha256() -> String {
    types::registry_sha256()
}

pub fn session_id(id: &str) -> String {
    ids::session_id(id)
}

pub fn run_id(id: &str) -> String {
    ids::run_id(id)
}

pub fn task_id(run: &str) -> String {
    ids::task_id(run)
}

pub fn tool_call_id(run: &str, call: &str) -> String {
    ids::tool_call_id(run, call)
}

pub fn observation_id(run: &str, call: &str) -> String {
    ids::observation_id(run, call)
}

pub fn answer_id(run: &str) -> String {
    ids::answer_id(run)
}

pub fn claim_id(run: &str, claim: &str) -> String {
    ids::claim_id(run, claim)
}

pub fn evidence_id(run: &str, seq: u64, index: usize) -> String {
    ids::evidence_id(run, seq, index)
}

pub fn artifact_id(sha256: &str) -> String {
    ids::artifact_id(sha256)
}

pub fn goal_id(id: &str) -> String {
    ids::goal_id(id)
}

pub fn commitment_id(id: &str) -> String {
    ids::commitment_id(id)
}

pub fn squad_member_id(run: &str, call: &str) -> String {
    ids::squad_member_id(run, call)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

/// Convenience for callers that need the exact bytes hashed by the exporter.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, OntologyError> {
    serde_json::to_vec(value).map_err(|error| OntologyError::Json(error.to_string()))
}

pub fn export_document_sha256(value: &Value) -> Result<String, OntologyError> {
    Ok(sha256_hex(&canonical_bytes(value)?))
}

pub fn read_builtin_types() -> &'static str {
    include_str!("../../ontology/types.json")
}

pub fn read_builtin_bindings() -> &'static str {
    include_str!("../../ontology/bindings.json")
}

pub fn path_text(path: &Path) -> String {
    path.display().to_string()
}
