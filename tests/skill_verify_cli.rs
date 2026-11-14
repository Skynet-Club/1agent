use std::process::Command;

use first_principles_agent::skill_bundle::{
    export, export_with_key, generate_signing_key, signing_key_from_pkcs8_hex,
};
use first_principles_agent::{SkillManifest, SkillRisk, SkillService};

fn service(dir: &tempfile::TempDir) -> SkillService {
    let mut service = SkillService::open(dir.path().join("skills.jsonl")).unwrap();
    service
        .register(
            SkillManifest {
                name: "alpha".into(),
                description: "skill alpha".into(),
                version: "1.0.0".into(),
                platforms: vec![],
                scope: "project".into(),
                requires_tools: vec![],
                risk: SkillRisk::Low,
                body: "alpha body".into(),
                content_sha256: String::new(),
            },
            1,
        )
        .unwrap();
    service
}

fn verify(args: &[&str]) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_first-principles-agent");
    Command::new(bin)
        .args(["skill", "verify"])
        .args(args)
        .arg("--json")
        .output()
        .unwrap()
}

#[test]
fn skill_verify_accepts_unsigned_and_signed_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(&dir);
    let exported = export(&service, None).unwrap();
    let bundle = dir.path().join("bundle.json");
    std::fs::write(&bundle, &exported.bytes).unwrap();
    let output = verify(&[
        "--bundle",
        bundle.to_str().unwrap(),
        "--sha256",
        &exported.sha256,
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["skills"], 1);
    assert_eq!(report["signed"], false);

    let (private_hex, public_hex) = generate_signing_key().unwrap();
    let key = signing_key_from_pkcs8_hex(&private_hex).unwrap();
    let signed = export_with_key(&service, None, Some(&key)).unwrap();
    let signed_bundle = dir.path().join("signed.json");
    std::fs::write(&signed_bundle, &signed.bytes).unwrap();
    let public_key = dir.path().join("public.hex");
    std::fs::write(&public_key, public_hex).unwrap();
    let output = verify(&[
        "--bundle",
        signed_bundle.to_str().unwrap(),
        "--sha256",
        &signed.sha256,
        "--public-key",
        public_key.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["signed"], true);
}

#[test]
fn skill_verify_rejects_a_tampered_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(&dir);
    let exported = export(&service, None).unwrap();
    let mut bytes = exported.bytes.clone();
    let index = bytes
        .windows(b"alpha body".len())
        .position(|window| window == b"alpha body")
        .unwrap();
    bytes[index] = b'A';
    let bundle = dir.path().join("tampered.json");
    std::fs::write(&bundle, &bytes).unwrap();
    let output = verify(&[
        "--bundle",
        bundle.to_str().unwrap(),
        "--sha256",
        &exported.sha256,
    ]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(report["error"].as_str().unwrap().contains("sha256"));
}
