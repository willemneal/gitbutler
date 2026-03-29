//! Cross-repo dependency tracking via the cut plan.
//!
//! The `CutPlan` is a dependency graph mapping which fabrics (PRs) from which
//! looms (repos) must be assembled in which order. The `ShuttleService`
//! orchestrates coordination by processing incoming shuttle messages and
//! dispatching status updates through a `LoomBridge`.

use super::bridge::{
    CoordinationPattern, FabricStatus, LoomBridge, ShuttleMessage, ShuttleMessageType,
};
use crate::types::{AgentId, MessageId, PrId, RepoRef, WeavePattern};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An entry in the cut plan -- one fabric that must be assembled.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CutPlanEntry {
    pub pr: PrId,
    pub status: FabricStatus,
    pub dependencies: Vec<PrId>,
    pub weave_pattern: WeavePattern,
    pub agent: AgentId,
}

/// The cut plan: a dependency graph mapping which fabrics (PRs) from which
/// looms (repos) must be assembled in which order.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CutPlan {
    pub entries: Vec<CutPlanEntry>,
}

impl CutPlan {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add or update an entry in the cut plan.
    pub fn upsert(&mut self, entry: CutPlanEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.pr == entry.pr) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Get the assembly order: entries sorted topologically by dependencies.
    ///
    /// Uses Kahn's algorithm for a proper topological sort. External
    /// dependencies (PRs not in the plan) are treated as already satisfied.
    pub fn assembly_order(&self) -> anyhow::Result<Vec<&CutPlanEntry>> {
        let len = self.entries.len();
        if len == 0 {
            return Ok(Vec::new());
        }

        // Build adjacency: for each entry index, which indices depend on it.
        let mut in_degree = vec![0usize; len];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); len];

        for (i, entry) in self.entries.iter().enumerate() {
            for dep in &entry.dependencies {
                if let Some(j) = self.entries.iter().position(|e| &e.pr == dep) {
                    dependents[j].push(i);
                    in_degree[i] += 1;
                }
                // External dependencies (not in plan) are treated as satisfied.
            }
        }

        // Kahn's algorithm.
        let mut queue: Vec<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, d)| **d == 0)
            .map(|(i, _)| i)
            .collect();
        let mut sorted = Vec::with_capacity(len);

        while let Some(idx) = queue.pop() {
            sorted.push(&self.entries[idx]);
            for &dep_idx in &dependents[idx] {
                in_degree[dep_idx] -= 1;
                if in_degree[dep_idx] == 0 {
                    queue.push(dep_idx);
                }
            }
        }

        if sorted.len() != len {
            anyhow::bail!(
                "Cycle detected in cut plan: sorted {} of {} entries",
                sorted.len(),
                len
            );
        }

        Ok(sorted)
    }

    /// Check if all dependencies for a given PR are complete (merged).
    pub fn dependencies_met(&self, pr: &PrId) -> bool {
        let entry = match self.entries.iter().find(|e| &e.pr == pr) {
            Some(e) => e,
            None => return false,
        };
        entry.dependencies.iter().all(|dep| {
            self.entries
                .iter()
                .any(|e| &e.pr == dep && matches!(e.status, FabricStatus::Merged))
        })
    }

    /// Get entries that are ready to proceed (all dependencies met, not yet merged/closed).
    pub fn ready_entries(&self) -> Vec<&CutPlanEntry> {
        self.entries
            .iter()
            .filter(|e| {
                !matches!(e.status, FabricStatus::Merged | FabricStatus::Closed)
                    && self.dependencies_met(&e.pr)
            })
            .collect()
    }

    /// Get an entry by PR reference.
    pub fn get(&self, pr: &PrId) -> Option<&CutPlanEntry> {
        self.entries.iter().find(|e| &e.pr == pr)
    }
}

/// Orchestrates cross-repo coordination using the LoomBridge.
pub struct ShuttleService {
    cut_plan: CutPlan,
}

impl ShuttleService {
    pub fn new() -> Self {
        Self {
            cut_plan: CutPlan::new(),
        }
    }

    /// Process incoming shuttle messages and update the cut plan.
    pub fn process_messages(
        &mut self,
        bridge: &dyn LoomBridge,
        since: &str,
    ) -> anyhow::Result<Vec<ShuttleMessage>> {
        let messages = bridge.receive_shuttles(since)?;
        for msg in &messages {
            self.update_cut_plan_from_message(msg);
        }
        Ok(messages)
    }

    /// Send a status update to a target repo.
    pub fn send_status(
        &self,
        bridge: &dyn LoomBridge,
        target: &RepoRef,
        message: &ShuttleMessage,
    ) -> anyhow::Result<MessageId> {
        bridge.send_shuttle(target, message)
    }

    /// Synchronize fabric statuses by querying the bridge for each tracked PR.
    pub fn sync_fabric_statuses(&mut self, bridge: &dyn LoomBridge) -> anyhow::Result<()> {
        for entry in &mut self.cut_plan.entries {
            match bridge.track_fabric(&entry.pr) {
                Ok(status) => {
                    if entry.status != status {
                        tracing::info!(
                            pr = %format!("{}#{}", entry.pr.repo, entry.pr.number),
                            old_status = ?entry.status,
                            new_status = ?status,
                            "Fabric status changed"
                        );
                        entry.status = status;
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        pr = %format!("{}#{}", entry.pr.repo, entry.pr.number),
                        error = %e,
                        "Failed to track fabric status"
                    );
                }
            }
        }
        Ok(())
    }

    /// Attach coordination patterns to all tracked PRs that have dependencies.
    pub fn propagate_patterns(&self, bridge: &dyn LoomBridge) -> anyhow::Result<()> {
        for entry in &self.cut_plan.entries {
            if entry.dependencies.is_empty() {
                continue;
            }
            let dependents: Vec<PrId> = self
                .cut_plan
                .entries
                .iter()
                .filter(|e| e.dependencies.contains(&entry.pr))
                .map(|e| e.pr.clone())
                .collect();

            let pattern = CoordinationPattern {
                dependencies: entry.dependencies.clone(),
                dependents,
                weave_pattern: entry.weave_pattern,
                expected_completion: None,
            };
            bridge.attach_pattern(&entry.pr, &pattern)?;
        }
        Ok(())
    }

    /// Get the current cut plan.
    pub fn cut_plan(&self) -> &CutPlan {
        &self.cut_plan
    }

    /// Get a mutable reference to the cut plan for direct manipulation.
    pub fn cut_plan_mut(&mut self) -> &mut CutPlan {
        &mut self.cut_plan
    }

    fn update_cut_plan_from_message(&mut self, msg: &ShuttleMessage) {
        tracing::debug!(
            from = %msg.from_loom.repo,
            message_type = ?msg.message_type,
            "Processing shuttle message"
        );

        match msg.message_type {
            ShuttleMessageType::Status => {
                let pr = PrId {
                    repo: msg.from_loom.repo.clone(),
                    number: msg.thread.task_ref.parse::<u64>().unwrap_or(0),
                };
                let status = match msg.thread.status.as_str() {
                    "weaving" => FabricStatus::Weaving,
                    "inspecting" => FabricStatus::Inspecting,
                    "complete" => FabricStatus::Complete,
                    "merged" => FabricStatus::Merged,
                    "closed" => FabricStatus::Closed,
                    _ => FabricStatus::Weaving,
                };
                if let Some(entry) = self.cut_plan.entries.iter_mut().find(|e| e.pr == pr) {
                    entry.status = status;
                }
            }
            ShuttleMessageType::Dependency => {
                // Parse dependency references in "owner/repo#number" format.
                let dep_prs: Vec<PrId> = msg
                    .thread
                    .dependencies
                    .iter()
                    .filter_map(|d| {
                        let (repo_part, num_part) = d.rsplit_once('#')?;
                        let (owner, name) = repo_part.rsplit_once('/')?;
                        Some(PrId {
                            repo: RepoRef {
                                owner: owner.to_string(),
                                name: name.to_string(),
                            },
                            number: num_part.parse().ok()?,
                        })
                    })
                    .collect();

                let entry = CutPlanEntry {
                    pr: PrId {
                        repo: msg.from_loom.repo.clone(),
                        number: msg.thread.task_ref.parse::<u64>().unwrap_or(0),
                    },
                    status: FabricStatus::Weaving,
                    dependencies: dep_prs,
                    weave_pattern: msg.thread.pattern_complexity,
                    agent: msg.from_loom.agent.clone(),
                };
                self.cut_plan.upsert(entry);
            }
            ShuttleMessageType::Handoff => {
                let pr = PrId {
                    repo: msg.from_loom.repo.clone(),
                    number: msg.thread.task_ref.parse::<u64>().unwrap_or(0),
                };
                if let Some(entry) = self.cut_plan.entries.iter_mut().find(|e| e.pr == pr) {
                    entry.status = FabricStatus::Complete;
                    entry.agent = msg.to_loom.agent.clone();
                }
            }
            ShuttleMessageType::Task | ShuttleMessageType::Budget => {
                tracing::debug!(
                    message_type = ?msg.message_type,
                    task_ref = %msg.thread.task_ref,
                    "Informational shuttle message received"
                );
            }
        }
    }
}
