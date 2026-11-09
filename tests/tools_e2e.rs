use std::process::Command;

#[test]
fn tools_command_uses_runtime_registry_and_redacts_paths() {
    let output = Command::new(env!("CARGO_BIN_EXE_first-principles-agent"))
        .args(["tools", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let capabilities: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let capabilities = capabilities.as_array().unwrap();
    assert_eq!(capabilities.len(), 2);
    assert_eq!(capabilities[0]["name"], "read_file");
    assert_eq!(capabilities[1]["name"], "search_files");
    for capability in capabilities {
        assert_eq!(capability["status"], "implemented");
        assert_eq!(capability["requires_write"], false);
        assert_eq!(capability["parameters"]["type"], "object");
    }
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains(&std::env::current_dir().unwrap().display().to_string()));
}
