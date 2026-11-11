use std::process::Command;

#[test]
fn platform_cli_reports_capabilities_without_overclaiming() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin)
        .args(["platform", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let expected = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => "other",
    };
    assert_eq!(report["platform"], expected);
    assert_eq!(report["os"], std::env::consts::OS);
    assert_eq!(report["arch"], std::env::consts::ARCH);
    let capabilities = report["capabilities"].as_array().unwrap();
    assert!(!capabilities.is_empty());
    assert!(
        capabilities
            .iter()
            .any(|capability| capability["name"] == "runtime_home")
    );
    assert!(
        capabilities
            .iter()
            .any(|capability| capability["name"] == "computer_use")
    );
}

#[test]
fn platform_cli_human_output_lists_capabilities() {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    let output = Command::new(bin).arg("platform").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("1Agent Platform"));
    assert!(text.contains("runtime_home:"));
    assert!(text.contains("computer_use:"));
}
