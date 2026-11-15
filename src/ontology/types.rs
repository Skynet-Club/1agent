use serde_json::Value;

use super::OntologyError;

pub fn builtin_types() -> Value {
    serde_json::from_str(super::read_builtin_types()).unwrap_or_else(|_| Value::Null)
}

pub fn builtin_bindings() -> Value {
    serde_json::from_str(super::read_builtin_bindings()).unwrap_or_else(|_| Value::Null)
}

pub fn registry_sha256() -> String {
    super::sha256_hex(super::read_builtin_types().as_bytes())
}

pub fn ensure_builtin_types_parse() -> Result<Value, OntologyError> {
    serde_json::from_str(super::read_builtin_types()).map_err(|error| OntologyError::Json(error.to_string()))
}

pub fn ensure_builtin_bindings_parse() -> Result<Value, OntologyError> {
    serde_json::from_str(super::read_builtin_bindings()).map_err(|error| OntologyError::Json(error.to_string()))
}
