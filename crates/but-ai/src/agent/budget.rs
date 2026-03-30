//! Token budget management with survival-based completion estimation.
//!
//! Determines the operational mode based on remaining budget and provides
//! completion probability estimates using the survival analysis module.
//!
//! Budget modes:
//! - **Full** (>80% remaining): all passes, full validation, coordination.
//! - **Abbreviated** (50-80%): skip polish pass, reduced validation.
//! - **MinimumOutput** (20-50%): rough pass only, skip validation.
//! - **EmergencyHalt** (<20%): stop work, catalog + coordinate only.

use crate::types::{TaskPhase, TokenBudget};

use super::BudgetMode;

/// Threshold: above this remaining fraction, full protocol is used.
const FULL_THRESHOLD: f64 = 0.80;

/// Threshold: above this remaining fraction, abbreviated protocol is used.
const ABBREVIATED_THRESHOLD: f64 = 0.50;

/// Threshold: above this remaining fraction, minimum output is produced.
const MINIMUM_THRESHOLD: f64 = 0.20;

/// Determine the budget mode from the current token budget state.
///
/// The mode is based on the fraction of budget **remaining** (not consumed).
pub fn budget_mode(budget: &TokenBudget) -> BudgetMode {
    let remaining_fraction = 1.0 - budget.utilization();

    if remaining_fraction >= FULL_THRESHOLD {
        BudgetMode::Full
    } else if remaining_fraction >= ABBREVIATED_THRESHOLD {
        BudgetMode::Abbreviated
    } else if remaining_fraction >= MINIMUM_THRESHOLD {
        BudgetMode::MinimumOutput
    } else {
        BudgetMode::EmergencyHalt
    }
}

/// Estimate the probability of completing a given phase within the remaining budget.
///
/// Uses a simple model: probability = min(1.0, available_tokens / estimated_tokens).
/// The estimate accounts for phase-specific token costs and reserves.
pub fn estimate_completion_probability(budget: &TokenBudget, phase: TaskPhase) -> f64 {
    let available = budget.available_for_work();
    let estimated_cost = phase_token_estimate(phase);

    if estimated_cost == 0 {
        return 1.0;
    }

    (available as f64 / estimated_cost as f64).min(1.0)
}

/// Estimate token cost for each phase.
///
/// These are rough estimates based on typical agent workloads.
/// Real implementations would learn from historical data via the
/// survival module's hazard rate functions.
fn phase_token_estimate(phase: TaskPhase) -> u64 {
    match phase {
        TaskPhase::Classify => 500,
        TaskPhase::Plan => 1_500,
        TaskPhase::Implement => 15_000,
        TaskPhase::Validate => 2_000,
        TaskPhase::Catalog => 1_000,
        TaskPhase::Coordinate => 1_500,
    }
}

/// Check whether a specific phase should proceed given the current budget.
///
/// Essential phases (Catalog, Coordinate) always proceed because their
/// tokens are reserved. Non-essential phases are gated by budget mode.
pub fn should_proceed(budget: &TokenBudget, phase: TaskPhase) -> bool {
    match phase {
        // Reserved phases: always proceed (tokens are set aside).
        TaskPhase::Catalog | TaskPhase::Coordinate => true,

        // Implementation requires at least MinimumOutput mode.
        TaskPhase::Implement => {
            !matches!(budget_mode(budget), BudgetMode::EmergencyHalt)
        }

        // Validation requires at least Abbreviated mode.
        TaskPhase::Validate => {
            matches!(
                budget_mode(budget),
                BudgetMode::Full | BudgetMode::Abbreviated
            )
        }

        // Classification and planning require at least MinimumOutput.
        TaskPhase::Classify | TaskPhase::Plan => {
            !matches!(budget_mode(budget), BudgetMode::EmergencyHalt)
        }
    }
}

/// Summary of budget state for logging/reporting.
pub fn budget_summary(budget: &TokenBudget) -> String {
    let mode = budget_mode(budget);
    let mode_label = match mode {
        BudgetMode::Full => "full",
        BudgetMode::Abbreviated => "abbreviated",
        BudgetMode::MinimumOutput => "minimum-output",
        BudgetMode::EmergencyHalt => "emergency-halt",
    };

    format!(
        "Budget: {}/{} tokens ({}% used, mode={}, work_available={})",
        budget.used,
        budget.total,
        (budget.utilization() * 100.0) as u32,
        mode_label,
        budget.available_for_work(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_mode_with_fresh_budget() {
        let budget = TokenBudget::new(32_000);
        assert_eq!(budget_mode(&budget), BudgetMode::Full);
    }

    #[test]
    fn abbreviated_mode() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 3_000; // 30% used, 70% remaining
        assert_eq!(budget_mode(&budget), BudgetMode::Abbreviated);
    }

    #[test]
    fn minimum_output_mode() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 6_500; // 65% used, 35% remaining
        assert_eq!(budget_mode(&budget), BudgetMode::MinimumOutput);
    }

    #[test]
    fn emergency_halt_mode() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 8_500; // 85% used, 15% remaining
        assert_eq!(budget_mode(&budget), BudgetMode::EmergencyHalt);
    }

    #[test]
    fn completion_probability_full_budget() {
        let budget = TokenBudget::new(32_000);
        let prob = estimate_completion_probability(&budget, TaskPhase::Implement);
        // 32000 - 3500 reserves = 28500 available, 15000 needed -> capped at 1.0
        assert!((prob - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn completion_probability_tight_budget() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 5_000;
        // 5000 remaining - 3500 reserves = 1500 available, 15000 needed
        let prob = estimate_completion_probability(&budget, TaskPhase::Implement);
        assert!(prob < 0.5);
    }

    #[test]
    fn catalog_always_proceeds() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 9_500; // Emergency halt
        assert!(should_proceed(&budget, TaskPhase::Catalog));
        assert!(should_proceed(&budget, TaskPhase::Coordinate));
    }

    #[test]
    fn implement_blocked_in_emergency() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 8_500;
        assert!(!should_proceed(&budget, TaskPhase::Implement));
    }

    #[test]
    fn validate_requires_abbreviated() {
        let mut budget = TokenBudget::new(10_000);
        budget.used = 6_500; // MinimumOutput mode
        assert!(!should_proceed(&budget, TaskPhase::Validate));

        let mut budget = TokenBudget::new(10_000);
        budget.used = 3_000; // Abbreviated mode
        assert!(should_proceed(&budget, TaskPhase::Validate));
    }

    #[test]
    fn budget_summary_format() {
        let budget = TokenBudget::new(32_000);
        let summary = budget_summary(&budget);
        assert!(summary.contains("mode=full"));
        assert!(summary.contains("0% used"));
    }
}
