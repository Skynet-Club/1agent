//! Deterministic cascade-failure simulation and containment metrics.
//!
//! A failure starts at a root node and propagates along directed edges unless a
//! node is marked as a containment point. The report measures how far the
//! failure spread, how many nodes it reached and how much of it was contained.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeReport {
    pub root: String,
    pub affected: Vec<String>,
    pub contained: Vec<String>,
    pub propagated: Vec<String>,
    pub containment_rate_bps: u16,
    pub cascade_depth: usize,
    pub blast_radius: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadePolicy {
    pub min_containment_bps: u16,
    pub max_depth: usize,
    pub max_blast_radius: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeVerdict {
    pub passed: bool,
    pub failures: Vec<String>,
}

pub fn simulate(root: &str, edges: &[(String, String)], contained: &[String]) -> CascadeReport {
    let mut adjacency = BTreeMap::<String, Vec<String>>::new();
    for (parent, child) in edges {
        adjacency
            .entry(parent.clone())
            .or_default()
            .push(child.clone());
    }
    for children in adjacency.values_mut() {
        children.sort();
        children.dedup();
    }
    let containment = contained.iter().cloned().collect::<BTreeSet<_>>();
    let mut visited = BTreeSet::<String>::new();
    let mut queue = VecDeque::<(String, usize)>::new();
    visited.insert(root.to_string());
    queue.push_back((root.to_string(), 0));
    let mut max_depth = 0usize;
    while let Some((node, depth)) = queue.pop_front() {
        max_depth = max_depth.max(depth);
        if containment.contains(&node) {
            continue;
        }
        if let Some(children) = adjacency.get(&node) {
            for child in children {
                if visited.insert(child.clone()) {
                    queue.push_back((child.clone(), depth + 1));
                }
            }
        }
    }
    let affected = visited.into_iter().collect::<Vec<_>>();
    let contained = affected
        .iter()
        .filter(|node| containment.contains(*node))
        .cloned()
        .collect::<Vec<_>>();
    let propagated = affected
        .iter()
        .filter(|node| !containment.contains(*node))
        .cloned()
        .collect::<Vec<_>>();
    let containment_rate_bps = if affected.is_empty() {
        0
    } else {
        ((contained.len() as u64 * 10_000) / affected.len() as u64) as u16
    };
    CascadeReport {
        root: root.to_string(),
        blast_radius: affected.len(),
        cascade_depth: max_depth,
        affected,
        contained,
        propagated,
        containment_rate_bps,
    }
}

pub fn evaluate(report: &CascadeReport, policy: &CascadePolicy) -> CascadeVerdict {
    let mut failures = Vec::new();
    if report.containment_rate_bps < policy.min_containment_bps {
        failures.push(format!(
            "containment {} bps is below {} bps",
            report.containment_rate_bps, policy.min_containment_bps
        ));
    }
    if report.cascade_depth > policy.max_depth {
        failures.push(format!(
            "cascade depth {} exceeds {}",
            report.cascade_depth, policy.max_depth
        ));
    }
    if report.blast_radius > policy.max_blast_radius {
        failures.push(format!(
            "blast radius {} exceeds {}",
            report.blast_radius, policy.max_blast_radius
        ));
    }
    CascadeVerdict {
        passed: failures.is_empty(),
        failures,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeEdge {
    pub parent: String,
    pub child: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeScenario {
    pub id: String,
    pub root: String,
    pub edges: Vec<CascadeEdge>,
    #[serde(default)]
    pub contained: Vec<String>,
    pub policy: CascadePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeScenarioResult {
    pub id: String,
    pub report: CascadeReport,
    pub verdict: CascadeVerdict,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CascadeBenchReport {
    pub scenarios: u64,
    pub passed: u64,
    pub failed: u64,
    pub max_cascade_depth: usize,
    pub max_blast_radius: usize,
    pub min_containment_bps: Option<u16>,
    pub results: Vec<CascadeScenarioResult>,
}

#[derive(Debug, thiserror::Error)]
pub enum CascadeBenchError {
    #[error("cascade bench line {line} is invalid: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("cascade bench scenario is invalid: {0}")]
    Invalid(String),
    #[error("cascade bench has no scenarios")]
    Empty,
}

pub fn parse_cascade_scenarios(text: &str) -> Result<Vec<CascadeScenario>, CascadeBenchError> {
    let mut scenarios = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let scenario: CascadeScenario =
            serde_json::from_str(line).map_err(|source| CascadeBenchError::Json {
                line: line_number,
                source,
            })?;
        scenarios.push(scenario);
    }
    Ok(scenarios)
}

pub fn bench(scenarios: &[CascadeScenario]) -> Result<CascadeBenchReport, CascadeBenchError> {
    if scenarios.is_empty() {
        return Err(CascadeBenchError::Empty);
    }
    if scenarios.len() > 1_000 {
        return Err(CascadeBenchError::Invalid(
            "at most 1000 scenarios are allowed".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for scenario in scenarios {
        if scenario.id.trim().is_empty()
            || scenario.id.chars().count() > 64
            || !scenario.id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
        {
            return Err(CascadeBenchError::Invalid(
                "scenario id must be 1..=64 safe characters".into(),
            ));
        }
        if !ids.insert(scenario.id.as_str()) {
            return Err(CascadeBenchError::Invalid(format!(
                "duplicate scenario id {}",
                scenario.id
            )));
        }
        if scenario.root.trim().is_empty() || scenario.root.chars().count() > 128 {
            return Err(CascadeBenchError::Invalid(
                "root must be 1..=128 characters".into(),
            ));
        }
        if scenario.edges.len() > 1_000 {
            return Err(CascadeBenchError::Invalid(
                "a scenario has at most 1000 edges".into(),
            ));
        }
        for edge in &scenario.edges {
            if edge.parent.trim().is_empty()
                || edge.parent.chars().count() > 128
                || edge.child.trim().is_empty()
                || edge.child.chars().count() > 128
            {
                return Err(CascadeBenchError::Invalid(
                    "edge endpoints must be 1..=128 characters".into(),
                ));
            }
        }
        if scenario.policy.min_containment_bps > 10_000 {
            return Err(CascadeBenchError::Invalid(
                "min_containment_bps must be <= 10000".into(),
            ));
        }
    }

    let mut report = CascadeBenchReport {
        scenarios: scenarios.len() as u64,
        ..CascadeBenchReport::default()
    };
    for scenario in scenarios {
        let edges = scenario
            .edges
            .iter()
            .map(|edge| (edge.parent.clone(), edge.child.clone()))
            .collect::<Vec<_>>();
        let result = simulate(&scenario.root, &edges, &scenario.contained);
        let verdict = evaluate(&result, &scenario.policy);
        report.max_cascade_depth = report.max_cascade_depth.max(result.cascade_depth);
        report.max_blast_radius = report.max_blast_radius.max(result.blast_radius);
        report.min_containment_bps = Some(
            report
                .min_containment_bps
                .map_or(result.containment_rate_bps, |current| {
                    current.min(result.containment_rate_bps)
                }),
        );
        if verdict.passed {
            report.passed += 1;
        } else {
            report.failed += 1;
        }
        report.results.push(CascadeScenarioResult {
            id: scenario.id.clone(),
            report: result,
            verdict,
        });
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(parent: &str, child: &str) -> (String, String) {
        (parent.into(), child.into())
    }

    #[test]
    fn containment_stops_propagation() {
        let report = simulate(
            "a",
            &[edge("a", "b"), edge("b", "c"), edge("c", "d")],
            &["b".into()],
        );
        assert_eq!(report.affected, vec!["a", "b"]);
        assert_eq!(report.contained, vec!["b"]);
        assert_eq!(report.propagated, vec!["a"]);
        assert_eq!(report.containment_rate_bps, 5_000);
        assert_eq!(report.cascade_depth, 1);
        assert_eq!(report.blast_radius, 2);
    }

    #[test]
    fn no_containment_reaches_the_whole_chain() {
        let report = simulate("a", &[edge("a", "b"), edge("b", "c"), edge("c", "d")], &[]);
        assert_eq!(report.affected.len(), 4);
        assert_eq!(report.containment_rate_bps, 0);
        assert_eq!(report.cascade_depth, 3);
        assert_eq!(report.blast_radius, 4);
    }

    #[test]
    fn contained_root_limits_the_blast_radius() {
        let report = simulate("a", &[edge("a", "b")], &["a".into()]);
        assert_eq!(report.affected, vec!["a"]);
        assert_eq!(report.contained, vec!["a"]);
        assert_eq!(report.containment_rate_bps, 10_000);
        assert_eq!(report.cascade_depth, 0);
        assert_eq!(report.blast_radius, 1);
    }

    #[test]
    fn policy_fails_closed_on_depth_or_blast_radius() {
        let report = simulate("a", &[edge("a", "b"), edge("b", "c")], &[]);
        let pass = evaluate(
            &report,
            &CascadePolicy {
                min_containment_bps: 0,
                max_depth: 2,
                max_blast_radius: 3,
            },
        );
        assert!(pass.passed);
        let fail = evaluate(
            &report,
            &CascadePolicy {
                min_containment_bps: 5_000,
                max_depth: 1,
                max_blast_radius: 2,
            },
        );
        assert!(!fail.passed);
        assert_eq!(fail.failures.len(), 3);
    }

    fn scenario(id: &str, contained: &[&str], policy: CascadePolicy) -> CascadeScenario {
        CascadeScenario {
            id: id.into(),
            root: "a".into(),
            edges: vec![
                CascadeEdge {
                    parent: "a".into(),
                    child: "b".into(),
                },
                CascadeEdge {
                    parent: "b".into(),
                    child: "c".into(),
                },
                CascadeEdge {
                    parent: "c".into(),
                    child: "d".into(),
                },
            ],
            contained: contained.iter().map(|node| node.to_string()).collect(),
            policy,
        }
    }

    #[test]
    fn bench_passes_contained_and_fails_uncontained() {
        let report = bench(&[
            scenario(
                "contained",
                &["b"],
                CascadePolicy {
                    min_containment_bps: 5_000,
                    max_depth: 1,
                    max_blast_radius: 2,
                },
            ),
            scenario(
                "uncontained",
                &[],
                CascadePolicy {
                    min_containment_bps: 5_000,
                    max_depth: 2,
                    max_blast_radius: 4,
                },
            ),
        ])
        .unwrap();
        assert_eq!(report.scenarios, 2);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.max_cascade_depth, 3);
        assert_eq!(report.max_blast_radius, 4);
        assert_eq!(report.min_containment_bps, Some(0));
        assert_eq!(report.results[0].id, "contained");
        assert!(report.results[0].verdict.passed);
        assert_eq!(report.results[1].id, "uncontained");
        assert!(!report.results[1].verdict.passed);
    }

    #[test]
    fn bench_rejects_invalid_scenarios() {
        let policy = CascadePolicy {
            min_containment_bps: 0,
            max_depth: 10,
            max_blast_radius: 10,
        };
        assert!(matches!(bench(&[]), Err(CascadeBenchError::Empty)));
        assert!(matches!(
            bench(&[scenario("dup", &[], policy), scenario("dup", &[], policy)]),
            Err(CascadeBenchError::Invalid(_))
        ));
        let mut too_strict = scenario("strict", &[], policy);
        too_strict.policy.min_containment_bps = 10_001;
        assert!(matches!(
            bench(&[too_strict]),
            Err(CascadeBenchError::Invalid(_))
        ));
        let mut empty_root = scenario("root", &[], policy);
        empty_root.root.clear();
        assert!(matches!(
            bench(&[empty_root]),
            Err(CascadeBenchError::Invalid(_))
        ));
    }

    #[test]
    fn parse_reports_line_numbers() {
        let text = "{\"id\":\"ok\",\"root\":\"a\",\"edges\":[],\"policy\":{\"min_containment_bps\":0,\"max_depth\":1,\"max_blast_radius\":1}}\n{\"broken\":true}\n";
        match parse_cascade_scenarios(text).unwrap_err() {
            CascadeBenchError::Json { line, .. } => assert_eq!(line, 2),
            other => panic!("expected a JSON error, got {other:?}"),
        }
    }
}
