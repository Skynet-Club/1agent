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
}
