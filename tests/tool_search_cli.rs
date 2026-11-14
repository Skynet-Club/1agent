use std::process::Command;

fn search(query: &str) -> serde_json::Value {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["tools", "--query", query, "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn tool_search_ranks_name_and_description_matches() {
    let read = search("read");
    let hits = read.as_array().unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0]["spec"]["name"], "read_file");
    assert!(hits.iter().any(|hit| hit["spec"]["name"] == "browser_read"));
    assert!(hits.iter().all(|hit| hit["spec"]["name"] != "write_file"));

    let shell = search("shell");
    assert_eq!(shell[0]["spec"]["name"], "terminal_exec");
    assert!(search("zzz").as_array().unwrap().is_empty());
}

#[test]
fn tool_search_human_output_contains_score() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["tools", "--query", "read"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("read_file"));
    assert!(text.contains("score="));
}
