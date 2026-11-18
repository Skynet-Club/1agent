//! Versioned core type registry and data-binding loading.
//!
//! The embedded JSON is validated at load time. The infallible `builtin_*`
//! accessors exist for the frozen `ontology` boundary; callers that need a
//! typed failure must use `try_builtin_*` or `parse_*`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde_json::{Map, Value};

use super::{BINDING_VERSION, ONTOLOGY_VERSION, OntologyError};

/// Schema version of `ontology/types.json`.
pub const TYPE_REGISTRY_SCHEMA_VERSION: u64 = 1;
/// Schema version of `ontology/bindings.json`.
pub const BINDING_SCHEMA_VERSION: u64 = 1;

const TYPE_REGISTRY: &str = "type registry";
const DATA_BINDINGS: &str = "data bindings";

pub fn builtin_types() -> Value {
    try_builtin_types().unwrap_or(Value::Null)
}

pub fn builtin_bindings() -> Value {
    try_builtin_bindings().unwrap_or(Value::Null)
}

pub fn try_builtin_types() -> Result<Value, OntologyError> {
    parse_types(super::read_builtin_types())
}

pub fn try_builtin_bindings() -> Result<Value, OntologyError> {
    parse_bindings(super::read_builtin_bindings())
}

/// SHA-256 over the exact bytes of `ontology/types.json`.
pub fn registry_sha256() -> String {
    super::sha256_hex(super::read_builtin_types().as_bytes())
}

#[cfg(test)]
pub fn ensure_builtin_types_parse() -> Result<Value, OntologyError> {
    try_builtin_types()
}

#[cfg(test)]
pub fn ensure_builtin_bindings_parse() -> Result<Value, OntologyError> {
    try_builtin_bindings()
}

/// Parse and validate a type-registry document.
pub fn parse_types(json: &str) -> Result<Value, OntologyError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| OntologyError::Json(format!("{TYPE_REGISTRY}: {error}")))?;
    validate_types_value(&value)?;
    Ok(value)
}

/// Parse and validate a data-binding document.
pub fn parse_bindings(json: &str) -> Result<Value, OntologyError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|error| OntologyError::Json(format!("{DATA_BINDINGS}: {error}")))?;
    validate_bindings_value(&value)?;
    Ok(value)
}

/// Structural validation for `ontology/types.json`.
pub fn validate_types_value(value: &Value) -> Result<(), OntologyError> {
    let root = value
        .as_object()
        .ok_or_else(|| invalid(TYPE_REGISTRY, "root must be a JSON object"))?;
    expect_schema_version(root, TYPE_REGISTRY, TYPE_REGISTRY_SCHEMA_VERSION)?;
    expect_version(root, TYPE_REGISTRY, "ontology_version", ONTOLOGY_VERSION)?;
    expect_string(root, TYPE_REGISTRY, "namespace")?;

    let types = root
        .get("types")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(TYPE_REGISTRY, "`types` must be an array"))?;
    if types.is_empty() {
        return Err(invalid(TYPE_REGISTRY, "`types` must not be empty"));
    }

    let mut seen_ids = HashSet::new();
    for (index, definition) in types.iter().enumerate() {
        let context = format!("{TYPE_REGISTRY} types[{index}]");
        let definition = definition
            .as_object()
            .ok_or_else(|| invalid(&context, "type definition must be a JSON object"))?;

        let id = expect_string(definition, &context, "id")?;
        if !seen_ids.insert(id) {
            return Err(invalid(&context, format!("duplicate type id `{id}`")));
        }

        let kind = expect_string(definition, &context, "kind")?;
        if !matches!(kind, "object" | "action" | "binding") {
            return Err(invalid(
                &context,
                format!("unsupported kind `{kind}`; expected object, action or binding"),
            ));
        }

        expect_string(definition, &context, "owner")?;
        expect_string(definition, &context, "version")?;

        let required = definition
            .get("required")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid(&context, "`required` must be an array"))?;
        let mut seen_properties = HashSet::new();
        for (required_index, property) in required.iter().enumerate() {
            let property = property
                .as_str()
                .filter(|property| !property.is_empty())
                .ok_or_else(|| {
                    invalid(
                        &context,
                        format!("`required[{required_index}]` must be a non-empty string"),
                    )
                })?;
            if !seen_properties.insert(property) {
                return Err(invalid(
                    &context,
                    format!("duplicate required property `{property}`"),
                ));
            }
        }
    }

    Ok(())
}

/// Structural validation for `ontology/bindings.json`.
pub fn validate_bindings_value(value: &Value) -> Result<(), OntologyError> {
    let root = value
        .as_object()
        .ok_or_else(|| invalid(DATA_BINDINGS, "root must be a JSON object"))?;
    expect_schema_version(root, DATA_BINDINGS, BINDING_SCHEMA_VERSION)?;
    expect_version(root, DATA_BINDINGS, "mapping_version", BINDING_VERSION)?;

    let bindings = root
        .get("bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(DATA_BINDINGS, "`bindings` must be an array"))?;
    if bindings.is_empty() {
        return Err(invalid(DATA_BINDINGS, "`bindings` must not be empty"));
    }

    for (index, binding) in bindings.iter().enumerate() {
        let context = format!("{DATA_BINDINGS} bindings[{index}]");
        let binding = binding
            .as_object()
            .ok_or_else(|| invalid(&context, "binding entry must be a JSON object"))?;

        expect_compact_id(binding, &context, "type")?;
        expect_compact_id(binding, &context, "property")?;
        expect_binding_source(binding, &context)?;
        for optional in [
            "mappingVersion",
            "owner",
            "mapping",
            "transform",
            "required_after",
        ] {
            if binding.contains_key(optional) {
                expect_string(binding, &context, optional)?;
            }
        }
        if let Some(required) = binding.get("required")
            && !required.is_boolean()
        {
            return Err(invalid(&context, "`required` must be a boolean"));
        }
    }

    Ok(())
}

/// Cross-registry binding coverage and consistency report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BindingCoverage {
    pub total_types: usize,
    pub bound_types: usize,
    pub unbound_types: Vec<String>,
    pub missing_required: Vec<String>,
    pub errors: Vec<String>,
}

impl BindingCoverage {
    pub fn is_complete(&self) -> bool {
        self.errors.is_empty() && self.unbound_types.is_empty() && self.missing_required.is_empty()
    }
}

const BINDING_SOURCE_PREFIXES: &[&str] = &[
    "session.json#",
    "journal#",
    "subagent_registry#",
    "goal_log#",
    "ontology/",
];

/// Validate bindings against the type registry: known type, matching owner,
/// supported mapping version, known source set, and a binding for every
/// required property of every type.
pub fn binding_coverage(types: &Value, bindings: &Value) -> BindingCoverage {
    let mut coverage = BindingCoverage::default();
    let mut type_owners = BTreeMap::<String, String>::new();
    let mut required = BTreeMap::<String, Vec<String>>::new();
    let Some(definitions) = types.get("types").and_then(Value::as_array) else {
        coverage
            .errors
            .push("type registry has no types array".into());
        return coverage;
    };
    coverage.total_types = definitions.len();
    for definition in definitions {
        let Some(id) = definition.get("id").and_then(Value::as_str) else {
            continue;
        };
        let owner = definition
            .get("owner")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        type_owners.insert(id.to_string(), owner);
        let required_properties = definition
            .get("required")
            .and_then(Value::as_array)
            .map(|properties| {
                properties
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        required.insert(id.to_string(), required_properties);
    }
    let Some(entries) = bindings.get("bindings").and_then(Value::as_array) else {
        coverage
            .errors
            .push("data bindings have no bindings array".into());
        return coverage;
    };

    let mut bound = BTreeSet::new();
    let mut bound_properties = BTreeSet::<(String, String)>::new();
    for (index, entry) in entries.iter().enumerate() {
        let context = format!("bindings[{index}]");
        let Some(entry) = entry.as_object() else {
            coverage.errors.push(format!("{context}: not an object"));
            continue;
        };
        let compact = |key: &str| {
            entry
                .get(key)
                .and_then(Value::as_str)
                .map(|value| value.strip_prefix("one:").unwrap_or(value).to_string())
        };
        let (Some(type_id), Some(property)) = (compact("type"), compact("property")) else {
            coverage
                .errors
                .push(format!("{context}: type/property is missing"));
            continue;
        };
        let Some(type_owner) = type_owners.get(&type_id) else {
            coverage
                .errors
                .push(format!("{context}: unknown type {type_id}"));
            continue;
        };
        let mut valid = true;
        match entry.get("owner").and_then(Value::as_str) {
            Some(owner) if owner == type_owner => {}
            Some(owner) => {
                valid = false;
                coverage.errors.push(format!(
                    "{context}: owner {owner} does not match type owner {type_owner}"
                ));
            }
            None => {
                valid = false;
                coverage
                    .errors
                    .push(format!("{context}: owner is required"));
            }
        }
        match entry.get("mappingVersion").and_then(Value::as_str) {
            Some(version) if version == BINDING_VERSION => {}
            Some(version) => {
                valid = false;
                coverage
                    .errors
                    .push(format!("{context}: unsupported mappingVersion {version}"));
            }
            None => {
                valid = false;
                coverage
                    .errors
                    .push(format!("{context}: mappingVersion is required"));
            }
        }
        let source = entry
            .get("source")
            .or_else(|| entry.get("sourcePath"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !BINDING_SOURCE_PREFIXES
            .iter()
            .any(|prefix| source.starts_with(prefix))
        {
            valid = false;
            coverage.errors.push(format!(
                "{context}: source `{source}` is not in the known source set"
            ));
        }
        if entry.contains_key("required") && entry.contains_key("required_after") {
            valid = false;
            coverage.errors.push(format!(
                "{context}: required and required_after are mutually exclusive"
            ));
        }
        if valid {
            bound.insert(type_id.clone());
            bound_properties.insert((type_id.clone(), property.clone()));
        }
    }
    coverage.bound_types = bound.len();
    for (type_id, properties) in &required {
        if !bound.contains(type_id) {
            coverage.unbound_types.push(type_id.clone());
        }
        for property in properties {
            if !bound_properties.contains(&(type_id.clone(), property.clone())) {
                coverage
                    .missing_required
                    .push(format!("{type_id}.{property}"));
            }
        }
    }
    coverage
}

pub fn builtin_binding_coverage() -> Result<BindingCoverage, OntologyError> {
    let types = try_builtin_types()?;
    let bindings = try_builtin_bindings()?;
    Ok(binding_coverage(&types, &bindings))
}

fn expect_schema_version(
    root: &Map<String, Value>,
    context: &str,
    expected: u64,
) -> Result<(), OntologyError> {
    match root.get("schema_version").and_then(Value::as_u64) {
        Some(version) if version == expected => Ok(()),
        Some(version) => Err(invalid(
            context,
            format!("unsupported schema_version `{version}`; expected `{expected}`"),
        )),
        None => Err(invalid(
            context,
            format!("`schema_version` must be the integer `{expected}`"),
        )),
    }
}

fn expect_version(
    root: &Map<String, Value>,
    context: &str,
    key: &str,
    expected: &str,
) -> Result<(), OntologyError> {
    let version = expect_string(root, context, key)?;
    if version != expected {
        return Err(invalid(
            context,
            format!("`{key}` is `{version}`; expected `{expected}`"),
        ));
    }
    Ok(())
}

fn expect_compact_id<'a>(
    object: &'a Map<String, Value>,
    context: &str,
    key: &str,
) -> Result<&'a str, OntologyError> {
    let value = expect_string(object, context, key)?;
    let compact = value.strip_prefix("one:").unwrap_or(value);
    if compact.is_empty() {
        return Err(invalid(
            context,
            format!("`{key}` must contain a local name after the `one:` prefix"),
        ));
    }
    Ok(compact)
}

fn expect_binding_source<'a>(
    object: &'a Map<String, Value>,
    context: &str,
) -> Result<&'a str, OntologyError> {
    object
        .get("source")
        .or_else(|| object.get("sourcePath"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            invalid(
                context,
                "`source` or `sourcePath` must be a non-empty string",
            )
        })
}

fn expect_string<'a>(
    object: &'a Map<String, Value>,
    context: &str,
    key: &str,
) -> Result<&'a str, OntologyError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(context, format!("`{key}` must be a non-empty string")))
}

fn invalid(context: &str, message: impl Into<String>) -> OntologyError {
    OntologyError::Validation(format!("{context}: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_TYPE_IDS: [&str; 17] = [
        "Session",
        "Run",
        "Task",
        "ToolInvocation",
        "Observation",
        "Answer",
        "Claim",
        "Evidence",
        "Artifact",
        "SquadMember",
        "Goal",
        "Commitment",
        "ActionType",
        "ActionInvocation",
        "PolicyDecision",
        "Approval",
        "DataBinding",
    ];

    fn minimal_types() -> String {
        format!(
            r#"{{
                "schema_version": 1,
                "ontology_version": "{ONTOLOGY_VERSION}",
                "namespace": "https://1agent.dev/ontology/0.98#",
                "types": [
                    {{
                        "id": "Run",
                        "kind": "object",
                        "owner": "runtime",
                        "version": "1",
                        "required": ["hasTask", "hasOutcome"]
                    }}
                ]
            }}"#
        )
    }

    fn minimal_bindings() -> String {
        format!(
            r#"{{
                "schema_version": 1,
                "mapping_version": "{BINDING_VERSION}",
                "bindings": [
                    {{
                        "type": "one:Run",
                        "property": "one:hasTask",
                        "source": "journal#RunStarted.request",
                        "owner": "runtime",
                        "mappingVersion": "{BINDING_VERSION}",
                        "required": true
                    }}
                ]
            }}"#
        )
    }

    #[test]
    fn builtin_types_parse_and_cover_core_entities() {
        let registry = try_builtin_types().expect("embedded type registry must be valid");
        let types = registry["types"].as_array().expect("types array");
        assert_eq!(types.len(), CORE_TYPE_IDS.len());

        let ids: Vec<&str> = types
            .iter()
            .map(|definition| definition["id"].as_str().expect("type id"))
            .collect();
        for expected in CORE_TYPE_IDS {
            assert!(ids.contains(&expected), "missing core type {expected}");
        }

        for definition in types {
            assert!(definition["owner"].as_str().is_some_and(|v| !v.is_empty()));
            assert!(
                definition["version"]
                    .as_str()
                    .is_some_and(|v| !v.is_empty())
            );
            assert!(definition["required"].is_array());
        }
    }

    #[test]
    fn builtin_bindings_parse_and_reference_compact_ids() {
        let bindings = try_builtin_bindings().expect("embedded data bindings must be valid");
        let entries = bindings["bindings"].as_array().expect("bindings array");
        assert!(!entries.is_empty());
        assert_eq!(bindings["mapping_version"].as_str(), Some(BINDING_VERSION));

        for entry in entries {
            assert!(entry["type"].as_str().is_some_and(|v| !v.is_empty()));
            assert!(entry["property"].as_str().is_some_and(|v| !v.is_empty()));
            assert!(entry["source"].as_str().is_some_and(|v| !v.is_empty()));
            assert!(entry["owner"].as_str().is_some_and(|v| !v.is_empty()));
            assert_eq!(entry["mappingVersion"].as_str(), Some(BINDING_VERSION));
        }

        let coverage = builtin_binding_coverage().expect("coverage must load");
        assert!(coverage.errors.is_empty(), "{:?}", coverage.errors);
        assert!(
            coverage.unbound_types.is_empty(),
            "unbound: {:?}",
            coverage.unbound_types
        );
        assert!(
            coverage.missing_required.is_empty(),
            "missing required: {:?}",
            coverage.missing_required
        );
        assert_eq!(coverage.bound_types, coverage.total_types);
        assert!(coverage.is_complete());
    }

    #[test]
    fn binding_coverage_reports_unknown_types_owners_sources_and_versions() {
        let types: Value = serde_json::json!({
            "schema_version": 1,
            "ontology_version": ONTOLOGY_VERSION,
            "namespace": "https://1agent.dev/ontology/0.98#",
            "types": [{
                "id": "Run",
                "kind": "object",
                "owner": "runtime",
                "version": "1",
                "required": ["hasTask"]
            }]
        });
        let bindings: Value = serde_json::json!({
            "schema_version": 1,
            "mapping_version": BINDING_VERSION,
            "bindings": [
                {
                    "type": "one:Missing",
                    "property": "one:x",
                    "source": "journal#x",
                    "owner": "runtime",
                    "mappingVersion": BINDING_VERSION
                },
                {
                    "type": "one:Run",
                    "property": "one:hasTask",
                    "source": "unknown_source#x",
                    "owner": "security",
                    "mappingVersion": "999"
                },
                {
                    "type": "one:Run",
                    "property": "one:other",
                    "source": "journal#x",
                    "mappingVersion": BINDING_VERSION,
                    "required": true,
                    "required_after": "P1"
                }
            ]
        });
        let coverage = binding_coverage(&types, &bindings);
        assert!(!coverage.is_complete());
        assert!(
            coverage
                .errors
                .iter()
                .any(|error| error.contains("unknown type Missing"))
        );
        assert!(
            coverage
                .errors
                .iter()
                .any(|error| error.contains("does not match type owner"))
        );
        assert!(
            coverage
                .errors
                .iter()
                .any(|error| error.contains("unsupported mappingVersion"))
        );
        assert!(
            coverage
                .errors
                .iter()
                .any(|error| error.contains("not in the known source set"))
        );
        assert!(
            coverage
                .errors
                .iter()
                .any(|error| error.contains("mutually exclusive"))
        );
        assert_eq!(coverage.unbound_types, ["Run"]);
        assert_eq!(coverage.missing_required, ["Run.hasTask"]);
    }

    #[test]
    fn binding_source_path_is_accepted() {
        let source_path = minimal_bindings().replace(
            "\"source\": \"journal#RunStarted.request\"",
            "\"sourcePath\": \"journal#RunStarted.request\"",
        );
        assert!(parse_bindings(&source_path).is_ok());
    }

    #[test]
    fn registry_sha256_matches_exact_embedded_bytes() {
        use sha2::{Digest, Sha256};

        let expected = format!(
            "{:x}",
            Sha256::digest(super::super::read_builtin_types().as_bytes())
        );
        let digest = registry_sha256();
        assert_eq!(digest, expected);
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(digest, registry_sha256(), "registry hash must be stable");
    }

    #[test]
    fn malformed_types_json_returns_typed_json_error() {
        let error = parse_types("{\"types\": [").expect_err("truncated JSON must fail");
        assert!(matches!(error, OntologyError::Json(_)), "got {error:?}");
    }

    #[test]
    fn malformed_bindings_json_returns_typed_json_error() {
        let error = parse_bindings("not json").expect_err("malformed JSON must fail");
        assert!(matches!(error, OntologyError::Json(_)), "got {error:?}");
    }

    #[test]
    fn structurally_invalid_types_return_validation_errors() {
        let missing_owner = minimal_types().replace("\"owner\": \"runtime\",", "");
        assert!(matches!(
            parse_types(&missing_owner),
            Err(OntologyError::Validation(_))
        ));

        let bad_kind = minimal_types().replace("\"kind\": \"object\"", "\"kind\": \"widget\"");
        assert!(matches!(
            parse_types(&bad_kind),
            Err(OntologyError::Validation(_))
        ));

        let duplicate = minimal_types().replace(
            "\"required\": [\"hasTask\", \"hasOutcome\"]",
            "\"required\": [\"hasTask\", \"hasTask\"]",
        );
        assert!(matches!(
            parse_types(&duplicate),
            Err(OntologyError::Validation(_))
        ));

        assert!(matches!(
            parse_types("{}"),
            Err(OntologyError::Validation(_))
        ));
    }

    #[test]
    fn structurally_invalid_bindings_return_validation_errors() {
        let missing_source =
            minimal_bindings().replace("\"source\": \"journal#RunStarted.request\",", "");
        assert!(matches!(
            parse_bindings(&missing_source),
            Err(OntologyError::Validation(_))
        ));

        let bad_required =
            minimal_bindings().replace("\"required\": true", "\"required\": \"yes\"");
        assert!(matches!(
            parse_bindings(&bad_required),
            Err(OntologyError::Validation(_))
        ));

        assert!(matches!(
            parse_bindings("[]"),
            Err(OntologyError::Validation(_))
        ));
    }

    #[test]
    fn infallible_boundary_never_panics_and_exposes_embedded_json() {
        assert!(builtin_types().is_object());
        assert!(builtin_bindings().is_object());
        assert!(ensure_builtin_types_parse().is_ok());
        assert!(ensure_builtin_bindings_parse().is_ok());
    }
}
