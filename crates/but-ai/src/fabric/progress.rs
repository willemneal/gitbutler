//! Progress reporting for weaving operations.
//!
//! Tracks agent task progress including tokens used, files touched,
//! completion percentage, and budget utilization.

use crate::types::{AgentId, LoomConfig, LoomPosition, WeavePattern, WeavePhase, WeaveProgress};

/// Reports progress during a weaving operation.
///
/// Tracks cumulative state across shuttle passes and provides
/// budget-aware completion estimates.
pub struct ProgressReporter {
    agent: AgentId,
    position: LoomPosition,
    token_budget: u64,
    tokens_used: u64,
    files_touched: Vec<String>,
    current_phase: Option<WeavePhase>,
    picks_completed: u32,
    picks_estimated: u32,
}

impl ProgressReporter {
    pub fn new(agent: AgentId, position: LoomPosition, token_budget: u64) -> Self {
        Self {
            agent,
            position,
            token_budget,
            tokens_used: 0,
            files_touched: Vec::new(),
            current_phase: None,
            picks_completed: 0,
            picks_estimated: 0,
        }
    }

    /// Create from a loom configuration, using its token budget.
    pub fn from_config(agent: AgentId, position: LoomPosition, config: &LoomConfig) -> Self {
        Self::new(agent, position, config.token_budget)
    }

    /// Record tokens consumed by the current operation.
    pub fn record_tokens(&mut self, tokens: u64) {
        self.tokens_used += tokens;
    }

    /// Record a file that was touched (created, modified, or deleted).
    pub fn record_file_touched(&mut self, path: impl Into<String>) {
        let path = path.into();
        if !self.files_touched.contains(&path) {
            self.files_touched.push(path);
        }
    }

    /// Update the current phase and pick estimates.
    pub fn advance_phase(
        &mut self,
        phase: WeavePhase,
        picks_completed: u32,
        picks_estimated: u32,
    ) {
        self.current_phase = Some(phase);
        self.picks_completed = picks_completed;
        self.picks_estimated = picks_estimated;
    }

    /// Completion percentage based on picks (0.0 to 1.0).
    pub fn completion(&self) -> f64 {
        if self.picks_estimated == 0 {
            return 0.0;
        }
        (self.picks_completed as f64 / self.picks_estimated as f64).min(1.0)
    }

    /// Budget utilization as a fraction (0.0 to 1.0).
    pub fn budget_utilization(&self) -> f64 {
        if self.token_budget == 0 {
            return 1.0;
        }
        self.tokens_used as f64 / self.token_budget as f64
    }

    /// Whether the operation has exceeded its token budget.
    pub fn is_over_budget(&self) -> bool {
        self.tokens_used > self.token_budget
    }

    /// Whether the operation is approaching the halt threshold.
    pub fn should_halt(&self, halt_threshold: f64) -> bool {
        self.budget_utilization() >= halt_threshold
    }

    /// Number of unique files touched.
    pub fn files_touched_count(&self) -> usize {
        self.files_touched.len()
    }

    /// List of files touched.
    pub fn files_touched(&self) -> &[String] {
        &self.files_touched
    }

    /// Current tokens used.
    pub fn tokens_used(&self) -> u64 {
        self.tokens_used
    }

    /// Create a progress report for the current state.
    pub fn report(
        &self,
        phase: WeavePhase,
        pick_number: u32,
        picks_estimated: u32,
        tokens_used: u64,
        pattern: WeavePattern,
        integrity: f64,
    ) -> WeaveProgress {
        WeaveProgress {
            phase,
            agent: self.agent.clone(),
            loom_position: self.position,
            pick_number,
            picks_estimated,
            tokens_used,
            tokens_budget: self.token_budget,
            weave_pattern: pattern,
            fabric_integrity: integrity,
        }
    }

    /// Create a progress report from the reporter's internal state.
    pub fn snapshot(&self, pattern: WeavePattern, integrity: f64) -> anyhow::Result<WeaveProgress> {
        let phase = self
            .current_phase
            .ok_or_else(|| anyhow::anyhow!("No phase set -- call advance_phase first"))?;

        Ok(WeaveProgress {
            phase,
            agent: self.agent.clone(),
            loom_position: self.position,
            pick_number: self.picks_completed,
            picks_estimated: self.picks_estimated,
            tokens_used: self.tokens_used,
            tokens_budget: self.token_budget,
            weave_pattern: pattern,
            fabric_integrity: integrity,
        })
    }

    /// Serialize a progress report to JSON.
    pub fn to_json(progress: &WeaveProgress) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(progress)?)
    }

    /// Produce a human-readable summary line.
    pub fn summary(&self) -> String {
        let pct = (self.completion() * 100.0) as u32;
        let budget_pct = (self.budget_utilization() * 100.0) as u32;
        format!(
            "[{}] phase={} picks={}/{} tokens={}/{} ({}%) files={} completion={}%",
            self.agent.0,
            self.current_phase
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "none".to_string()),
            self.picks_completed,
            self.picks_estimated,
            self.tokens_used,
            self.token_budget,
            budget_pct,
            self.files_touched.len(),
            pct,
        )
    }
}
