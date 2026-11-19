use serde_json::Value;

fn schema(name: &str) -> Value {
    let raw = match name {
        "task" => include_str!("../schemas/1ash/task.schema.json"),
        "run" => include_str!("../schemas/1ash/run.schema.json"),
        "trace" => include_str!("../schemas/1ash/trace.schema.json"),
        "report" => include_str!("../schemas/1ash/report.schema.json"),
        other => panic!("unknown schema {other}"),
    };
    serde_json::from_str(raw).unwrap()
}

fn example(name: &str) -> Value {
    let raw = match name {
        "task" => include_str!("../schemas/1ash/examples/task.example.json"),
        "run" => include_str!("../schemas/1ash/examples/run.example.json"),
        "report" => include_str!("../schemas/1ash/examples/report.example.json"),
        other => panic!("unknown example {other}"),
    };
    serde_json::from_str(raw).unwrap()
}

#[test]
fn schemas_are_versioned_and_examples_match_their_schema_constants() {
    for (name, schema_id, example_schema) in [
        ("task", "https://1ash.dev/schemas/task/v1", "1ash.task/v1"),
        ("run", "https://1ash.dev/schemas/run/v1", "1ash.run/v1"),
        (
            "report",
            "https://1ash.dev/schemas/report/v1",
            "1ash.report/v1",
        ),
    ] {
        let schema = schema(name);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["$id"], schema_id);
        assert_eq!(schema["type"], "object");
        let required = schema["required"].as_array().expect("required array");
        assert!(!required.is_empty(), "{name} has no required fields");
        assert_eq!(
            example(name)["schema"],
            example_schema,
            "{name} example schema mismatch"
        );
    }
    assert!(
        schema("trace")["properties"]["type"]["enum"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value == "tool_call"))
    );
}

#[test]
fn task_example_has_weighted_modules_and_baselines() {
    let task = example("task");
    let modules = task["success"]["modules"].as_array().unwrap();
    assert!(!modules.is_empty());
    let weight: f64 = modules
        .iter()
        .map(|module| module["weight"].as_f64().unwrap())
        .sum();
    assert!((weight - 1.0).abs() < 1e-9, "module weights must sum to 1");
    assert_eq!(task["baselines"]["noop"]["expected"], "fail");
    assert_eq!(task["environment"]["network"], "none");
}

#[test]
fn trace_example_parses_as_one_canonical_event() {
    let raw = include_str!("../schemas/1ash/examples/trace.example.jsonl");
    let event: Value = serde_json::from_str(raw.trim()).unwrap();
    assert_eq!(event["schema"], "1ash.trace/v1");
    assert_eq!(event["type"], "run_started");
    assert_eq!(event["seq"], 0);
}
