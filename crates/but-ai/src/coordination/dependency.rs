//! Cross-repo dependency DAG with topological sort.
//!
//! Tracks which PRs depend on which other PRs and computes a valid
//! execution order using Kahn's algorithm. Cycle detection is built in.

use crate::types::{AgentId, PrRef, PrStatus};
use std::collections::HashMap;

/// A node in the dependency DAG representing a tracked PR.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// The pull request this node represents.
    pub pr: PrRef,
    /// PRs that must merge before this one can proceed.
    pub depends_on: Vec<PrRef>,
    /// The agent responsible for this PR.
    pub agent: AgentId,
    /// Current status.
    pub status: PrStatus,
}

/// A dependency DAG tracking cross-repo PR dependencies.
///
/// Provides topological sorting (Kahn's algorithm), readiness queries,
/// and cycle detection.
pub struct DependencyGraph {
    nodes: Vec<DependencyNode>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add or update a node in the graph.
    pub fn upsert(&mut self, node: DependencyNode) {
        if let Some(existing) = self.nodes.iter_mut().find(|n| n.pr == node.pr) {
            *existing = node;
        } else {
            self.nodes.push(node);
        }
    }

    /// Remove a node from the graph.
    pub fn remove(&mut self, pr: &PrRef) {
        self.nodes.retain(|n| n.pr != *pr);
    }

    /// Get a node by PR reference.
    pub fn get(&self, pr: &PrRef) -> Option<&DependencyNode> {
        self.nodes.iter().find(|n| n.pr == *pr)
    }

    /// Update the status of a tracked PR.
    pub fn set_status(&mut self, pr: &PrRef, status: PrStatus) {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.pr == *pr) {
            node.status = status;
        }
    }

    /// Compute a topological ordering of all nodes using Kahn's algorithm.
    ///
    /// External dependencies (PRs not in the graph) are treated as already
    /// satisfied. Returns an error if the graph contains a cycle.
    pub fn topological_sort(&self) -> anyhow::Result<Vec<&DependencyNode>> {
        let len = self.nodes.len();
        if len == 0 {
            return Ok(Vec::new());
        }

        // Map PR -> index for efficient lookup
        let index_of: HashMap<&PrRef, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (&n.pr, i))
            .collect();

        // Build in-degree counts and adjacency lists
        let mut in_degree = vec![0usize; len];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); len];

        for (i, node) in self.nodes.iter().enumerate() {
            for dep in &node.depends_on {
                if let Some(&j) = index_of.get(dep) {
                    dependents[j].push(i);
                    in_degree[i] += 1;
                }
                // External dependencies not in graph are treated as satisfied
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == 0)
            .map(|(i, _)| i)
            .collect();

        let mut sorted = Vec::with_capacity(len);

        while let Some(idx) = queue.pop() {
            sorted.push(&self.nodes[idx]);
            for &dep_idx in &dependents[idx] {
                in_degree[dep_idx] -= 1;
                if in_degree[dep_idx] == 0 {
                    queue.push(dep_idx);
                }
            }
        }

        if sorted.len() != len {
            anyhow::bail!(
                "Cycle detected in dependency graph: sorted {} of {} nodes",
                sorted.len(),
                len
            );
        }

        Ok(sorted)
    }

    /// Check whether all dependencies for a given PR are satisfied (merged).
    pub fn dependencies_met(&self, pr: &PrRef) -> bool {
        let node = match self.nodes.iter().find(|n| n.pr == *pr) {
            Some(n) => n,
            None => return false,
        };
        node.depends_on.iter().all(|dep| {
            // If the dependency is in our graph, it must be merged.
            // If it's external (not in graph), treat as satisfied.
            match self.nodes.iter().find(|n| n.pr == *dep) {
                Some(dep_node) => dep_node.status == PrStatus::Merged,
                None => true,
            }
        })
    }

    /// Get all nodes that are ready to proceed: dependencies met,
    /// not yet merged or closed.
    pub fn ready(&self) -> Vec<&DependencyNode> {
        self.nodes
            .iter()
            .filter(|n| {
                !matches!(n.status, PrStatus::Merged | PrStatus::Closed)
                    && self.dependencies_met(&n.pr)
            })
            .collect()
    }

    /// Get all nodes that are blocked (have unmet dependencies).
    pub fn blocked(&self) -> Vec<&DependencyNode> {
        self.nodes
            .iter()
            .filter(|n| {
                !matches!(n.status, PrStatus::Merged | PrStatus::Closed)
                    && !self.dependencies_met(&n.pr)
            })
            .collect()
    }

    /// Get all tracked nodes.
    pub fn nodes(&self) -> &[DependencyNode] {
        &self.nodes
    }

    /// Get the number of tracked nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ForgeType, RepoRef};

    fn pr(owner: &str, repo: &str, number: u64) -> PrRef {
        PrRef {
            repo: RepoRef {
                forge: ForgeType::GitHub,
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
            number,
        }
    }

    fn node(pr_ref: PrRef, deps: Vec<PrRef>) -> DependencyNode {
        DependencyNode {
            pr: pr_ref,
            depends_on: deps,
            agent: AgentId("test-agent".to_string()),
            status: PrStatus::Open,
        }
    }

    #[test]
    fn topological_sort_linear_chain() {
        let mut graph = DependencyGraph::new();

        let a = pr("org", "repo", 1);
        let b = pr("org", "repo", 2);
        let c = pr("org", "repo", 3);

        graph.upsert(node(c.clone(), vec![b.clone()]));
        graph.upsert(node(b.clone(), vec![a.clone()]));
        graph.upsert(node(a.clone(), vec![]));

        let sorted = graph.topological_sort().unwrap();
        let order: Vec<u64> = sorted.iter().map(|n| n.pr.number).collect();

        // a must come before b, b before c
        let pos_a = order.iter().position(|&n| n == 1).unwrap();
        let pos_b = order.iter().position(|&n| n == 2).unwrap();
        let pos_c = order.iter().position(|&n| n == 3).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn topological_sort_detects_cycle() {
        let mut graph = DependencyGraph::new();

        let a = pr("org", "repo", 1);
        let b = pr("org", "repo", 2);

        graph.upsert(node(a.clone(), vec![b.clone()]));
        graph.upsert(node(b.clone(), vec![a.clone()]));

        assert!(graph.topological_sort().is_err());
    }

    #[test]
    fn dependencies_met_checks_status() {
        let mut graph = DependencyGraph::new();

        let a = pr("org", "repo", 1);
        let b = pr("org", "repo", 2);

        graph.upsert(node(a.clone(), vec![]));
        graph.upsert(node(b.clone(), vec![a.clone()]));

        // b depends on a, which is Open -- not met
        assert!(!graph.dependencies_met(&b));

        // Mark a as merged
        graph.set_status(&a, PrStatus::Merged);
        assert!(graph.dependencies_met(&b));
    }

    #[test]
    fn ready_and_blocked() {
        let mut graph = DependencyGraph::new();

        let a = pr("org", "repo", 1);
        let b = pr("org", "repo", 2);
        let c = pr("org", "repo", 3);

        graph.upsert(node(a.clone(), vec![]));
        graph.upsert(node(b.clone(), vec![a.clone()]));
        graph.upsert(node(c.clone(), vec![a.clone()]));

        // Only a is ready (no deps)
        assert_eq!(graph.ready().len(), 1);
        assert_eq!(graph.ready()[0].pr, a);

        // b and c are blocked
        assert_eq!(graph.blocked().len(), 2);

        // Merge a -- now b and c are ready
        graph.set_status(&a, PrStatus::Merged);
        assert_eq!(graph.ready().len(), 2);
        assert!(graph.blocked().is_empty());
    }

    #[test]
    fn external_dependencies_treated_as_satisfied() {
        let mut graph = DependencyGraph::new();

        let external = pr("other-org", "other-repo", 99);
        let local = pr("org", "repo", 1);

        graph.upsert(node(local.clone(), vec![external]));

        // External dep not in graph -- treated as met
        assert!(graph.dependencies_met(&local));
        assert_eq!(graph.ready().len(), 1);
    }

    #[test]
    fn upsert_replaces_existing() {
        let mut graph = DependencyGraph::new();
        let a = pr("org", "repo", 1);

        graph.upsert(DependencyNode {
            pr: a.clone(),
            depends_on: vec![],
            agent: AgentId("old".to_string()),
            status: PrStatus::Open,
        });

        graph.upsert(DependencyNode {
            pr: a.clone(),
            depends_on: vec![],
            agent: AgentId("new".to_string()),
            status: PrStatus::Draft,
        });

        assert_eq!(graph.len(), 1);
        assert_eq!(graph.get(&a).unwrap().agent.0, "new");
        assert_eq!(graph.get(&a).unwrap().status, PrStatus::Draft);
    }
}
