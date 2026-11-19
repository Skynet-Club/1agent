//! Open-world capability resolution.
//!
//! A task is compiled into a contract with a deliverable and capability
//! requirements, then resolved against declarative manifests. Task types and
//! tool names are not hardcoded: a new tool or skill only needs a manifest.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const MANIFESTS_JSON: &str = include_str!("../capabilities/manifests.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestFile {
    pub version: u32,
    #[serde(default)]
    pub deliverable_hints: Vec<DeliverableHint>,
    pub manifests: Vec<CapabilityManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeliverableHint {
    pub keywords: Vec<String>,
    pub media_type: String,
    #[serde(default)]
    pub schema: Option<String>,
    pub requirements: Vec<CapabilityRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityManifest {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub source: String,
    #[serde(default)]
    pub installed: bool,
    pub trust: String,
    pub license: String,
    pub capabilities: Vec<CapabilityDescriptor>,
    #[serde(default)]
    pub verification: Vec<String>,
    #[serde(default)]
    pub install: Option<InstallSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstallSpec {
    pub command: String,
    pub approval: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptor {
    pub verb: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeliverableSpec {
    pub media_type: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequirement {
    pub verb: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskContract {
    pub objective: String,
    #[serde(default)]
    pub deliverable: Option<DeliverableSpec>,
    #[serde(default)]
    pub requirements: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub ambiguities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub manifest_id: String,
    pub name: String,
    pub kind: String,
    pub source: String,
    pub installed: bool,
    pub trust: String,
    pub license: String,
    pub score: u16,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequirementResolution {
    pub requirement: CapabilityRequirement,
    #[serde(default)]
    pub selected: Option<Candidate>,
    #[serde(default)]
    pub alternatives: Vec<Candidate>,
    #[serde(default)]
    pub gap: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityResolution {
    pub contract: TaskContract,
    pub requirements: Vec<RequirementResolution>,
    #[serde(default)]
    pub gaps: Vec<String>,
}

pub fn manifests() -> Result<&'static ManifestFile, String> {
    static MANIFESTS: OnceLock<Result<ManifestFile, String>> = OnceLock::new();
    MANIFESTS
        .get_or_init(|| {
            let parsed: ManifestFile = serde_json::from_str(MANIFESTS_JSON)
                .map_err(|error| format!("invalid capability manifests: {error}"))?;
            validate_manifests(&parsed)?;
            Ok(parsed)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn validate_manifests(file: &ManifestFile) -> Result<(), String> {
    if file.version != 1 {
        return Err(format!("unsupported manifest version {}", file.version));
    }
    let mut ids = BTreeSet::new();
    for manifest in &file.manifests {
        if !ids.insert(manifest.id.as_str()) {
            return Err(format!("duplicate manifest id {}", manifest.id));
        }
        if manifest.name.trim().is_empty()
            || manifest.kind.trim().is_empty()
            || manifest.source.trim().is_empty()
            || manifest.license.trim().is_empty()
            || manifest.capabilities.is_empty()
        {
            return Err(format!("manifest {} is incomplete", manifest.id));
        }
        if !matches!(
            manifest.trust.as_str(),
            "core" | "verified" | "community" | "experimental"
        ) {
            return Err(format!("manifest {} has unknown trust", manifest.id));
        }
        for capability in &manifest.capabilities {
            if capability.verb.trim().is_empty() {
                return Err(format!("manifest {} has an empty verb", manifest.id));
            }
        }
        if let Some(install) = &manifest.install
            && (install.command.trim().is_empty() || install.approval != "human")
        {
            return Err(format!(
                "manifest {} install must have a command and human approval",
                manifest.id
            ));
        }
    }
    Ok(())
}

pub fn compile_contract(
    objective: &str,
    deliverable: Option<DeliverableSpec>,
    requirements: Option<Vec<CapabilityRequirement>>,
) -> Result<TaskContract, String> {
    let file = manifests()?;
    let mut contract = TaskContract {
        objective: objective.trim().to_string(),
        deliverable,
        requirements: Vec::new(),
        constraints: Vec::new(),
        acceptance: Vec::new(),
        ambiguities: Vec::new(),
    };
    if let Some(requirements) = requirements {
        contract.requirements = requirements;
    } else if contract.deliverable.is_none() {
        let normalized = objective.to_lowercase();
        for hint in &file.deliverable_hints {
            if hint
                .keywords
                .iter()
                .any(|keyword| normalized.contains(&keyword.to_lowercase()))
            {
                contract.deliverable = Some(DeliverableSpec {
                    media_type: hint.media_type.clone(),
                    schema: hint.schema.clone(),
                    required: Vec::new(),
                });
                contract.requirements = hint.requirements.clone();
                contract
                    .acceptance
                    .push("artifact exists and is previewable".into());
                break;
            }
        }
    }
    if contract.requirements.is_empty()
        && let Some(deliverable) = contract.deliverable.as_ref()
        && !deliverable.media_type.trim().is_empty()
    {
        contract.requirements.push(CapabilityRequirement {
            verb: "produce".into(),
            inputs: Vec::new(),
            outputs: vec![deliverable.media_type.clone()],
            constraints: Vec::new(),
            required: true,
        });
    }
    Ok(contract)
}

pub fn resolve_contract(
    contract: TaskContract,
    installed_tools: &[String],
) -> Result<CapabilityResolution, String> {
    let file = manifests()?;
    resolve_with_manifests(contract, &file.manifests, installed_tools)
}

pub fn resolve_task(
    objective: &str,
    deliverable: Option<DeliverableSpec>,
    requirements: Option<Vec<CapabilityRequirement>>,
    installed_tools: &[String],
) -> Result<CapabilityResolution, String> {
    let contract = compile_contract(objective, deliverable, requirements)?;
    resolve_contract(contract, installed_tools)
}

pub fn resolve_with_manifests(
    contract: TaskContract,
    manifests: &[CapabilityManifest],
    installed_tools: &[String],
) -> Result<CapabilityResolution, String> {
    let installed = installed_tools
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut requirements = Vec::new();
    let mut gaps = Vec::new();
    for requirement in &contract.requirements {
        let mut candidates = Vec::new();
        for manifest in manifests {
            for capability in &manifest.capabilities {
                let (score, reasons) = score(requirement, manifest, capability, &installed);
                if score > 0 {
                    candidates.push(Candidate {
                        manifest_id: manifest.id.clone(),
                        name: manifest.name.clone(),
                        kind: manifest.kind.clone(),
                        source: manifest.source.clone(),
                        installed: manifest.installed
                            || installed.contains(&manifest.name.to_ascii_lowercase()),
                        trust: manifest.trust.clone(),
                        license: manifest.license.clone(),
                        score,
                        reasons,
                    });
                }
            }
        }
        candidates.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.installed.cmp(&left.installed))
                .then_with(|| left.name.cmp(&right.name))
        });
        let selected = candidates
            .iter()
            .find(|candidate| candidate.installed)
            .cloned();
        let alternatives = candidates
            .iter()
            .filter(|candidate| {
                selected.as_ref().is_none_or(|selected| {
                    selected.manifest_id != candidate.manifest_id || selected.name != candidate.name
                })
            })
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        let gap = if requirement.required && selected.is_none() {
            Some(if candidates.is_empty() {
                format!(
                    "no capability manifest provides {} -> {:?}",
                    requirement.verb, requirement.outputs
                )
            } else {
                format!(
                    "no installed capability provides {} -> {:?}; install or approve a candidate",
                    requirement.verb, requirement.outputs
                )
            })
        } else {
            None
        };
        if let Some(gap) = &gap {
            gaps.push(gap.clone());
        }
        requirements.push(RequirementResolution {
            requirement: requirement.clone(),
            selected,
            alternatives,
            gap,
        });
    }
    Ok(CapabilityResolution {
        contract,
        requirements,
        gaps,
    })
}

fn verbs_match(required: &str, available: &str) -> bool {
    if required == available {
        return true;
    }
    fn family(verb: &str) -> Option<&'static str> {
        match verb {
            "compose" | "build" | "produce" | "render" | "generate" => Some("create"),
            "verify" | "validate" | "audit" => Some("verify"),
            "read" | "extract" | "fetch" => Some("read"),
            "search" | "find" => Some("search"),
            "preview" | "serve" | "open" => Some("preview"),
            _ => None,
        }
    }
    matches!((family(required), family(available)), (Some(a), Some(b)) if a == b)
}

fn score(
    requirement: &CapabilityRequirement,
    manifest: &CapabilityManifest,
    capability: &CapabilityDescriptor,
    installed: &BTreeSet<String>,
) -> (u16, Vec<String>) {
    if !verbs_match(&requirement.verb, &capability.verb) {
        return (0, Vec::new());
    }
    let mut score = 40u16;
    let mut reasons = vec![format!("verb={}", capability.verb)];
    let required_outputs = requirement.outputs.iter().collect::<BTreeSet<_>>();
    let available_outputs = capability.outputs.iter().collect::<BTreeSet<_>>();
    if !required_outputs.is_empty() && !required_outputs.is_subset(&available_outputs) {
        return (0, Vec::new());
    }
    if !required_outputs.is_empty() {
        score += 30;
        reasons.push("outputs covered".into());
    }
    let required_inputs = requirement.inputs.iter().collect::<BTreeSet<_>>();
    let available_inputs = capability.inputs.iter().collect::<BTreeSet<_>>();
    let input_hits = required_inputs.intersection(&available_inputs).count();
    if !required_inputs.is_empty() {
        if input_hits == 0 {
            return (0, Vec::new());
        }
        score += (20 * input_hits / required_inputs.len()) as u16;
        reasons.push(format!(
            "{input_hits}/{} inputs covered",
            required_inputs.len()
        ));
    }
    let manifest_installed =
        manifest.installed || installed.contains(&manifest.name.to_ascii_lowercase());
    if manifest_installed {
        score += 20;
        reasons.push("installed".into());
    }
    score += match manifest.trust.as_str() {
        "core" => 10,
        "verified" => 6,
        "community" => 0,
        _ => 0,
    };
    if !manifest.verification.is_empty() {
        score += 5;
        reasons.push("has verification".into());
    }
    (score, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(
        id: &str,
        name: &str,
        installed: bool,
        verb: &str,
        outputs: &[&str],
    ) -> CapabilityManifest {
        CapabilityManifest {
            id: id.into(),
            name: name.into(),
            kind: "tool".into(),
            source: "test".into(),
            installed,
            trust: "core".into(),
            license: "internal".into(),
            capabilities: vec![CapabilityDescriptor {
                verb: verb.into(),
                inputs: Vec::new(),
                outputs: outputs.iter().map(|value| value.to_string()).collect(),
                constraints: Vec::new(),
            }],
            verification: vec!["test receipt".into()],
            install: None,
        }
    }

    #[test]
    fn diagram_task_selects_manual_build_and_lists_archify() {
        let contract = compile_contract("画一张架构图", None, None).unwrap();
        assert_eq!(
            contract
                .deliverable
                .as_ref()
                .map(|value| value.media_type.as_str()),
            Some("image/svg+xml")
        );
        let resolution = resolve_contract(contract, &[]).unwrap();
        assert!(resolution.gaps.is_empty());
        assert_eq!(
            resolution.requirements[0]
                .selected
                .as_ref()
                .map(|candidate| candidate.name.as_str()),
            Some("manual-svg")
        );
        assert!(
            resolution.requirements[0]
                .alternatives
                .iter()
                .any(|candidate| candidate.name == "archify")
        );
    }

    #[test]
    fn a_new_manifest_can_resolve_a_new_capability_without_code() {
        let contract = TaskContract {
            objective: "生成 3D 模型".into(),
            deliverable: Some(DeliverableSpec {
                media_type: "model/gltf+json".into(),
                schema: Some("3d-model/v1".into()),
                required: Vec::new(),
            }),
            requirements: vec![CapabilityRequirement {
                verb: "render".into(),
                inputs: Vec::new(),
                outputs: vec!["model/gltf+json".into()],
                constraints: Vec::new(),
                required: true,
            }],
            constraints: Vec::new(),
            acceptance: vec!["artifact exists".into()],
            ambiguities: Vec::new(),
        };
        let manifests = vec![manifest(
            "skill.threejs",
            "threejs",
            true,
            "render",
            &["model/gltf+json"],
        )];
        let resolution = resolve_with_manifests(contract, &manifests, &[]).unwrap();
        assert!(resolution.gaps.is_empty());
        assert_eq!(
            resolution.requirements[0]
                .selected
                .as_ref()
                .map(|candidate| candidate.name.as_str()),
            Some("threejs")
        );
    }

    #[test]
    fn unknown_capability_is_a_named_gap_not_a_silent_fallback() {
        let contract = TaskContract {
            objective: "生成未知格式".into(),
            deliverable: None,
            requirements: vec![CapabilityRequirement {
                verb: "transmogrify".into(),
                inputs: vec!["thing".into()],
                outputs: vec!["other".into()],
                constraints: Vec::new(),
                required: true,
            }],
            constraints: Vec::new(),
            acceptance: Vec::new(),
            ambiguities: Vec::new(),
        };
        let resolution = resolve_with_manifests(contract, &[], &[]).unwrap();
        assert_eq!(resolution.gaps.len(), 1);
        assert!(resolution.gaps[0].contains("no capability manifest"));
    }

    #[test]
    fn installed_overrides_manifest_default() {
        let contract = TaskContract {
            objective: "read".into(),
            deliverable: None,
            requirements: vec![CapabilityRequirement {
                verb: "read".into(),
                inputs: Vec::new(),
                outputs: vec!["text".into()],
                constraints: Vec::new(),
                required: true,
            }],
            constraints: Vec::new(),
            acceptance: Vec::new(),
            ambiguities: Vec::new(),
        };
        let manifests = vec![manifest("tool.read", "read_file", false, "read", &["text"])];
        let resolution =
            resolve_with_manifests(contract, &manifests, &["read_file".into()]).unwrap();
        assert!(resolution.gaps.is_empty());
        assert!(
            resolution.requirements[0]
                .selected
                .as_ref()
                .is_some_and(|candidate| candidate.installed)
        );
    }

    #[test]
    fn builtin_action_manifests_select_the_typed_code_release_actions() {
        let resolution = resolve_task(
            "propose a code change",
            None,
            Some(vec![CapabilityRequirement {
                verb: "propose".into(),
                inputs: vec!["change_request".into()],
                outputs: vec!["change_proposal".into()],
                constraints: Vec::new(),
                required: true,
            }]),
            &["code.propose_change".into()],
        )
        .unwrap();
        assert!(resolution.gaps.is_empty());
        assert_eq!(
            resolution.requirements[0]
                .selected
                .as_ref()
                .map(|candidate| candidate.name.as_str()),
            Some("code.propose_change")
        );
    }
}
