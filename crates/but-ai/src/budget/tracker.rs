//! Token budget tracking per agent per tide cycle.
//!
//! Raul's domain. Every LLM call has a cost measured in tokens, every task
//! has a budget, and the agent must complete its work within budget or
//! produce a valid partial result.
//!
//! When remaining tokens drop below 10% of total, the agent enters
//! **graceful degradation**: skip remaining steps, produce INDEX.patch
//! from work completed so far, write COMMIT.msg with "PARTIAL:" prefix.

use crate::types::{AgentId, TokenUsage};

/// Percentage of budget remaining that triggers graceful degradation.
const DEGRADATION_THRESHOLD: f64 = 0.10;

/// The phase of task execution (for budget checkpoint reporting).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// Reading task description and decomposing.
    Ingest,
    /// Querying memory for relevant context.
    Recall,
    /// Decomposing task into steps.
    Plan,
    /// Executing tool calls.
    Execute { step: u32, total_steps: u32 },
    /// Generating INDEX.patch.
    Generate,
    /// Writing COMMIT.msg.
    Message,
    /// Signing the commit.
    Sign,
    /// Producing the final report.
    Report,
    /// Graceful degradation -- budget nearly exhausted.
    Degraded,
}

impl std::fmt::Display for ExecutionPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ingest => write!(f, "ingest"),
            Self::Recall => write!(f, "recall"),
            Self::Plan => write!(f, "plan"),
            Self::Execute { step, total_steps } => {
                write!(f, "execute (step {} of {})", step, total_steps)
            }
            Self::Generate => write!(f, "generate"),
            Self::Message => write!(f, "message"),
            Self::Sign => write!(f, "sign"),
            Self::Report => write!(f, "report"),
            Self::Degraded => write!(f, "degraded"),
        }
    }
}

/// A checkpoint in the budget tracking log.
#[derive(Debug, Clone)]
pub struct BudgetCheckpoint {
    /// The execution phase at checkpoint time.
    pub phase: ExecutionPhase,
    /// Tokens used at this checkpoint.
    pub used: TokenUsage,
    /// Remaining budget at this checkpoint.
    pub remaining: u64,
    /// ISO-8601 timestamp of the checkpoint.
    pub timestamp: String,
    /// Description of what happened at this checkpoint.
    pub note: String,
}

/// Budget tracker for a single agent on a single task.
///
/// Tracks token usage across all LLM calls, logs checkpoints,
/// and signals when the agent should enter graceful degradation.
pub struct BudgetTracker {
    /// The agent being tracked.
    agent: AgentId,
    /// Total budget for this task.
    total: u64,
    /// Running usage counters.
    used: TokenUsage,
    /// Current execution phase.
    phase: ExecutionPhase,
    /// Checkpoint history.
    checkpoints: Vec<BudgetCheckpoint>,
    /// Whether we have entered degraded mode.
    degraded: bool,
}

impl BudgetTracker {
    /// Create a new budget tracker with the given total budget.
    pub fn new(agent: AgentId, total: u64) -> Self {
        Self {
            agent,
            total,
            used: TokenUsage::zero(),
            phase: ExecutionPhase::Ingest,
            checkpoints: Vec::new(),
            degraded: false,
        }
    }

    /// Record token usage from an LLM call.
    ///
    /// Returns `true` if the agent should enter graceful degradation.
    pub fn record(&mut self, input_tokens: u64, output_tokens: u64) -> bool {
        self.used.input += input_tokens;
        self.used.output += output_tokens;

        let should_degrade = self.should_degrade();
        if should_degrade && !self.degraded {
            self.degraded = true;
            tracing::warn!(
                agent = %self.agent,
                used = %self.used.total(),
                total = %self.total,
                remaining = %self.remaining(),
                "Budget threshold reached -- entering graceful degradation"
            );
            self.phase = ExecutionPhase::Degraded;
        }
        should_degrade
    }

    /// Set the current execution phase and optionally log a checkpoint.
    pub fn set_phase(&mut self, phase: ExecutionPhase, now: String, note: String) {
        self.phase = phase.clone();
        self.checkpoints.push(BudgetCheckpoint {
            phase,
            used: self.used.clone(),
            remaining: self.remaining(),
            timestamp: now,
            note,
        });
    }

    /// Get the remaining token budget.
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.used.total())
    }

    /// Get the percentage of budget used (0.0 to 1.0).
    pub fn usage_fraction(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.used.total() as f64 / self.total as f64
    }

    /// Whether the agent should enter graceful degradation.
    pub fn should_degrade(&self) -> bool {
        let remaining_fraction = 1.0 - self.usage_fraction();
        remaining_fraction < DEGRADATION_THRESHOLD
    }

    /// Whether we are in degraded mode.
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Get the current usage.
    pub fn used(&self) -> &TokenUsage {
        &self.used
    }

    /// Get the total budget.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Get the current phase.
    pub fn current_phase(&self) -> &ExecutionPhase {
        &self.phase
    }

    /// Get all checkpoints.
    pub fn checkpoints(&self) -> &[BudgetCheckpoint] {
        &self.checkpoints
    }

    /// Produce a budget report snapshot (for protocol messages).
    pub fn snapshot(&self) -> BudgetSnapshot {
        BudgetSnapshot {
            agent: self.agent.clone(),
            total: self.total,
            used: self.used.clone(),
            remaining: self.remaining(),
            phase: format!("{}", self.phase),
            degraded: self.degraded,
            checkpoint_count: self.checkpoints.len(),
        }
    }
}

/// A serializable budget snapshot.
#[derive(Debug, Clone)]
pub struct BudgetSnapshot {
    pub agent: AgentId,
    pub total: u64,
    pub used: TokenUsage,
    pub remaining: u64,
    pub phase: String,
    pub degraded: bool,
    pub checkpoint_count: usize,
}

/// Budget allocation table for the collective.
///
/// Maps agent IDs to their per-task token budgets. Used by the collective
/// to ensure the total team budget is within limits.
pub struct BudgetAllocation {
    allocations: std::collections::HashMap<String, u64>,
    team_total: u64,
}

impl BudgetAllocation {
    /// Create a new budget allocation with equal budgets per agent.
    pub fn equal(agents: &[AgentId], per_agent: u64) -> Self {
        let mut allocations = std::collections::HashMap::new();
        for agent in agents {
            allocations.insert(agent.0.clone(), per_agent);
        }
        let team_total = per_agent * agents.len() as u64;
        Self {
            allocations,
            team_total,
        }
    }

    /// Create a custom allocation from a map.
    pub fn custom(allocations: std::collections::HashMap<String, u64>) -> Self {
        let team_total = allocations.values().sum();
        Self {
            allocations,
            team_total,
        }
    }

    /// Get the budget for a specific agent.
    pub fn budget_for(&self, agent: &AgentId) -> Option<u64> {
        self.allocations.get(&agent.0).copied()
    }

    /// Get the total team budget.
    pub fn team_total(&self) -> u64 {
        self.team_total
    }

    /// Create a tracker for a specific agent using their allocated budget.
    pub fn tracker_for(&self, agent: &AgentId) -> Option<BudgetTracker> {
        self.budget_for(agent)
            .map(|budget| BudgetTracker::new(agent.clone(), budget))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_tracking_basic() {
        let agent = AgentId("dara".to_string());
        let mut tracker = BudgetTracker::new(agent, 50_000);

        assert_eq!(tracker.remaining(), 50_000);
        assert!(!tracker.should_degrade());

        tracker.record(10_000, 5_000);
        assert_eq!(tracker.remaining(), 35_000);
        assert!(!tracker.is_degraded());
    }

    #[test]
    fn degradation_triggers_at_threshold() {
        let agent = AgentId("dara".to_string());
        let mut tracker = BudgetTracker::new(agent, 10_000);

        // Use 91% of budget
        let degraded = tracker.record(8_000, 1_100);
        assert!(degraded);
        assert!(tracker.is_degraded());
    }

    #[test]
    fn checkpoint_logging() {
        let agent = AgentId("dara".to_string());
        let mut tracker = BudgetTracker::new(agent, 50_000);

        tracker.set_phase(
            ExecutionPhase::Plan,
            "2026-03-28T14:00:00Z".to_string(),
            "Starting planning".to_string(),
        );
        tracker.record(1_000, 500);

        tracker.set_phase(
            ExecutionPhase::Execute {
                step: 1,
                total_steps: 5,
            },
            "2026-03-28T14:01:00Z".to_string(),
            "First tool call".to_string(),
        );

        assert_eq!(tracker.checkpoints().len(), 2);
    }

    #[test]
    fn budget_allocation_equal() {
        let agents = vec![
            AgentId("dara".to_string()),
            AgentId("ines".to_string()),
            AgentId("koel".to_string()),
        ];
        let allocation = BudgetAllocation::equal(&agents, 10_000);

        assert_eq!(allocation.budget_for(&agents[0]), Some(10_000));
        assert_eq!(allocation.team_total(), 30_000);
    }

    #[test]
    fn snapshot_captures_state() {
        let agent = AgentId("raul".to_string());
        let mut tracker = BudgetTracker::new(agent, 50_000);
        tracker.record(12_000, 3_000);

        let snap = tracker.snapshot();
        assert_eq!(snap.total, 50_000);
        assert_eq!(snap.used.total(), 15_000);
        assert_eq!(snap.remaining, 35_000);
        assert!(!snap.degraded);
    }
}
