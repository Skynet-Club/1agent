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
    assert_eq!(capabilities.len(), 39);
    let find = |name: &str| {
        capabilities
            .iter()
            .find(|capability| capability["name"] == name)
            .unwrap_or_else(|| panic!("missing tool {name}"))
    };
    for name in [
        "read_file",
        "preview_file",
        "session_search",
        "memory_recall",
        "user_model",
        "memory_why",
        "capability_search",
        "resolve_capability",
        "verify_artifact",
        "propose_capability_install",
        "acceptance_gate",
        "read_pdf",
        "read_document",
        "skill_search",
        "search_files",
        "fetch_url",
        "web_search",
        "browser_read",
        "skill_list",
        "skill_eval",
        "delegation_list",
    ] {
        let capability = find(name);
        assert_eq!(capability["status"], "implemented");
        assert_eq!(capability["requires_write"], false);
        assert_eq!(capability["parameters"]["type"], "object");
    }
    for name in [
        "edit_file",
        "write_file",
        "terminal_create",
        "terminal_write",
        "terminal_output",
        "terminal_exec",
        "terminal_destroy",
        "skill_register",
        "skill_activate",
        "skill_rollback",
        "delegation_register",
        "delegation_revoke",
        "spawn_agent",
        "browser",
    ] {
        let capability = find(name);
        assert_eq!(capability["status"], "implemented");
        assert_eq!(capability["requires_write"], true);
        assert_eq!(capability["parameters"]["type"], "object");
    }
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains(&std::env::current_dir().unwrap().display().to_string()));
}
