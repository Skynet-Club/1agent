use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub ok: bool,
    pub checks: Vec<String>,
    pub errors: Vec<String>,
}

pub fn validate_document(_document: &Value) -> ValidationReport {
    ValidationReport {
        ok: false,
        checks: Vec::new(),
        errors: vec!["validator is not implemented yet".into()],
    }
}
