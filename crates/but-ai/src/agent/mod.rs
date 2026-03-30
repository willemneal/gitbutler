//! Agent orchestration -- the integration layer.
//!
//! Ties together all modules into a six-phase task lifecycle:
//!
//! 1. **Classify**: retrieve relevant memories, classify task context.
//! 2. **Plan**: architect agent designs approach and plan.
//! 3. **Implement**: implementer agent produces patches (multi-pass shuttle).
//! 4. **Validate**: validator agent checks for contradictions (conditional).
//! 5. **Catalog**: store the new work as memory for future retrieval.
//! 6. **Coordinate**: create PR, post coordination messages.

pub mod architect;
pub mod budget;
pub mod coordinator;
pub mod implementer;
pub mod phase_gate;
pub mod validator;

pub use architect::Architect;
pub use budget::{budget_mode, estimate_completion_probability};
pub use coordinator::Coordinator;
pub use implementer::Implementer;
pub use phase_gate::{is_tool_allowed, tools_for_phase};
pub use validator::Validator;

use crate::types::{
    AgentId, CallNumber, Classification, EntryId, MemoryEntry, MemoryRetriever,
    MemoryState, MemoryStore, SurvivalDistribution, SurvivalMetadata, TaskPhase, Tension,
    TokenBudget, PrRef,
};

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

// ---------------------------------------------------------------------------
// Agent-module types
// ---------------------------------------------------------------------------

/// A structured task plan produced by the architect agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskPlan {
    /// Description of the planned approach.
    pub approach: String,
    /// Files expected to be modified.
    pub files: Vec<String>,
    /// Estimated complexity level.
    pub complexity: Complexity,
    /// Estimated tokens needed for implementation.
    pub estimated_tokens: u64,
}

/// Task complexity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Complexity {
    Simple,
    Moderate,
    Complex,
}

/// Output from the implementer agent: a patch with metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatchOutput {
    /// The unified diff patch content.
    pub patch: String,
    /// The commit message (with metadata trailers).
    pub commit_msg: String,
    /// Files touched by this patch.
    pub files_touched: Vec<String>,
    /// Tokens consumed during implementation.
    pub tokens_used: u64,
}

/// Budget mode determining the operational protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMode {
    /// >80% remaining: all passes, full validation, coordination.
    Full,
    /// 50-80% remaining: skip polish pass, reduced validation.
    Abbreviated,
    /// 20-50% remaining: rough pass only, skip validation.
    MinimumOutput,
    /// <20% remaining: stop work, catalog + coordinate only.
    EmergencyHalt,
}

/// Result from the complete task execution pipeline.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// The plan produced by the architect.
    pub plan: TaskPlan,
    /// The patch produced by the implementer (if budget allowed).
    pub patch: Option<PatchOutput>,
    /// Validation result (if validation was run).
    pub validation: Option<ValidationResult>,
    /// Tokens remaining after task completion.
    pub budget_remaining: u64,
}

/// Result from the validator agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationResult {
    /// Whether the patch passed all checks.
    pub passed: bool,
    /// List of issues found.
    pub issues: Vec<String>,
    /// Tensions detected during validation.
    pub tensions_detected: Vec<Tension>,
}

/// Result from the coordinator agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoordinationResult {
    /// The PR created, if any.
    pub pr_created: Option<PrRef>,
    /// Number of coordination messages sent.
    pub messages_sent: u32,
}

/// Specification of a tool available in a given phase.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolSpec {
    /// Tool name (used for phase-gate lookup).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// The phase this tool specification belongs to.
    pub phase: TaskPhase,
}

// ---------------------------------------------------------------------------
// AgentOrchestrator
// ---------------------------------------------------------------------------

/// The top-level orchestrator that runs the six-phase task lifecycle.
///
/// Each phase checks the budget before proceeding. Essential phases
/// (Catalog, Coordinate) always run because their tokens are reserved.
pub struct AgentOrchestrator<'a> {
    agent_id: AgentId,
    retriever: &'a dyn MemoryRetriever,
    store: &'a dyn MemoryStore,
    budget: TokenBudget,
}

impl<'a> AgentOrchestrator<'a> {
    /// Create a new orchestrator for a task.
    pub fn new(
        agent_id: AgentId,
        retriever: &'a dyn MemoryRetriever,
        store: &'a dyn MemoryStore,
        budget: TokenBudget,
    ) -> Self {
        Self {
            agent_id,
            retriever,
            store,
            budget,
        }
    }

    /// Execute the full task lifecycle.
    ///
    /// Phases: Classify -> Plan -> Implement -> Validate -> Catalog -> Coordinate.
    /// Each phase checks budget before proceeding. Returns a `TaskResult`
    /// summarizing the outcome.
    pub fn execute_task(&mut self, task: &str) -> anyhow::Result<TaskResult> {
        tracing::info!(
            agent = %self.agent_id,
            task = %task,
            "Starting task execution: {}",
            budget::budget_summary(&self.budget),
        );

        // Phase 1: Classify (part of planning).
        // Budget check is implicit in the plan phase.

        // Phase 2: Plan.
        let plan = if budget::should_proceed(&self.budget, TaskPhase::Plan) {
            let p = Architect::plan(task, self.retriever)?;
            tracing::info!(
                complexity = ?p.complexity,
                files = p.files.len(),
                "Plan complete",
            );
            // Account for planning tokens.
            self.budget.used += 500;
            p
        } else {
            // Emergency: create a minimal plan.
            TaskPlan {
                approach: format!("Emergency plan for: {task}"),
                files: Vec::new(),
                complexity: Complexity::Simple,
                estimated_tokens: 0,
            }
        };

        // Phase 3: Implement.
        let patch = if budget::should_proceed(&self.budget, TaskPhase::Implement) {
            Some(Implementer::implement(&plan, &mut self.budget))
        } else {
            tracing::warn!("Skipping implementation due to budget constraints");
            None
        };

        // Phase 4: Validate (conditional).
        let validation = if let Some(ref p) = patch {
            if budget::should_proceed(&self.budget, TaskPhase::Validate)
                && Validator::should_validate(&self.budget, self.store)?
            {
                let result = Validator::validate(p, self.store)?;
                self.budget.used += 500; // Validation cost.
                Some(result)
            } else {
                None
            }
        } else {
            None
        };

        // Phase 5: Catalog -- store as memory for future retrieval.
        if let Some(ref p) = patch {
            self.catalog_patch(p, &plan)?;
        }

        // Phase 6: Coordinate is left to the caller, since it requires
        // a ForgeAdapter and DependencyGraph that are external concerns.

        Ok(TaskResult {
            plan,
            patch,
            validation,
            budget_remaining: self.budget.remaining(),
        })
    }

    /// Catalog a completed patch as a memory entry.
    fn catalog_patch(&self, patch: &PatchOutput, plan: &TaskPlan) -> anyhow::Result<()> {
        let subject_headings: Vec<String> = patch
            .files_touched
            .iter()
            .flat_map(|f| {
                f.split('/')
                    .filter(|s| !s.is_empty() && *s != "src" && *s != "crates")
                    .map(|s| s.replace(".rs", "").replace(".ts", ""))
                    .collect::<Vec<_>>()
            })
            .collect();

        let call_number = if let Some(first_file) = patch.files_touched.first() {
            crate::memory::call_number::call_number_from_path(first_file, 4)
        } else {
            CallNumber::parse("AGENT.PATCH")
        };

        let entry = MemoryEntry {
            id: EntryId(format!("task-{}", simple_hash(&patch.commit_msg))),
            agent: self.agent_id.clone(),
            content: format!(
                "{}\n\nFiles: {}\nComplexity: {:?}",
                plan.approach,
                patch.files_touched.join(", "),
                plan.complexity,
            ),
            created_at: String::new(),
            last_accessed: String::new(),
            classification: Classification {
                subject_headings,
                call_number,
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 1.0,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 1.0,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        };

        self.store.store(&entry)
    }
}

/// Simple hash for generating entry IDs.
fn simple_hash(s: &str) -> String {
    let mut hash: u64 = 0;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::retrieval::RetrievalEngine;
    use crate::memory::see_also::SeeAlsoGraph;
    use crate::memory::store::InMemoryStore;

    #[test]
    fn full_pipeline_executes() {
        let store = InMemoryStore::new();
        let engine = RetrievalEngine::new(InMemoryStore::new(), SeeAlsoGraph::new(5));
        let budget = TokenBudget::new(32_000);
        let mut orchestrator = AgentOrchestrator::new(
            AgentId("test-agent".into()),
            &engine,
            &store,
            budget,
        );

        let result = orchestrator.execute_task("add authentication middleware").unwrap();

        assert!(result.patch.is_some());
        assert!(result.budget_remaining > 0);
    }

    #[test]
    fn emergency_budget_skips_implementation() {
        let store = InMemoryStore::new();
        let engine = RetrievalEngine::new(InMemoryStore::new(), SeeAlsoGraph::new(5));
        let mut budget = TokenBudget::new(10_000);
        budget.used = 8_500; // Emergency halt
        let mut orchestrator = AgentOrchestrator::new(
            AgentId("test-agent".into()),
            &engine,
            &store,
            budget,
        );

        let result = orchestrator.execute_task("fix bug").unwrap();
        assert!(result.patch.is_none());
    }

    #[test]
    fn budget_modes_are_correct() {
        assert_eq!(budget_mode(&TokenBudget::new(32_000)), BudgetMode::Full);

        let mut b = TokenBudget::new(10_000);
        b.used = 3_000;
        assert_eq!(budget_mode(&b), BudgetMode::Abbreviated);

        b.used = 6_500;
        assert_eq!(budget_mode(&b), BudgetMode::MinimumOutput);

        b.used = 8_500;
        assert_eq!(budget_mode(&b), BudgetMode::EmergencyHalt);
    }
}
