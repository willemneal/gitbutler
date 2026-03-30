//! Implementer agent -- iterative multi-pass patch production.
//!
//! Follows Org 083's shuttle pattern: three passes through the loom,
//! each refining the output while tracking token budget consumption.
//!
//! - **Pass 1 (Rough):** Produce a draft implementation from the plan.
//! - **Pass 2 (Refinement):** Improve structure, naming, edge cases.
//! - **Pass 3 (Polish):** Final edge cases, documentation, cleanup.
//!
//! Each pass checks the token budget before proceeding. If budget runs
//! low, later passes are skipped (abbreviated mode).

use crate::types::TokenBudget;

use super::{BudgetMode, Complexity, PatchOutput, TaskPlan};

/// Fraction of estimated tokens allocated to each pass.
const PASS1_FRACTION: f64 = 0.50;
const PASS2_FRACTION: f64 = 0.30;
const PASS3_FRACTION: f64 = 0.20;

/// The implementer agent. Produces patches through iterative refinement.
pub struct Implementer;

/// Tracks state across shuttle passes.
struct ShuttleState {
    /// Accumulated patch content (refined across passes).
    patch: String,
    /// Files touched so far.
    files: Vec<String>,
    /// Total tokens consumed across all passes.
    tokens_used: u64,
    /// Which passes were completed.
    passes_completed: u32,
}

impl Implementer {
    /// Execute the implementation plan, producing a patch through iterative
    /// multi-pass refinement.
    ///
    /// The shuttle pattern allocates the estimated token budget across three
    /// passes. If budget pressure forces abbreviation, later passes are
    /// skipped and the rough output is returned as-is.
    pub fn implement(plan: &TaskPlan, budget: &mut TokenBudget) -> PatchOutput {
        let mode = super::budget::budget_mode(budget);
        let mut state = ShuttleState {
            patch: String::new(),
            files: plan.files.clone(),
            tokens_used: 0,
            passes_completed: 0,
        };

        // Pass 1: Rough implementation (always runs unless emergency).
        if !matches!(mode, BudgetMode::EmergencyHalt) {
            let pass1_budget = (plan.estimated_tokens as f64 * PASS1_FRACTION) as u64;
            Self::pass_rough(plan, &mut state, pass1_budget);
            budget.used += state.tokens_used;
        }

        // Pass 2: Refinement (only if budget allows).
        let mode = super::budget::budget_mode(budget);
        if matches!(mode, BudgetMode::Full | BudgetMode::Abbreviated) {
            let pass2_budget = (plan.estimated_tokens as f64 * PASS2_FRACTION) as u64;
            let tokens_before = state.tokens_used;
            Self::pass_refine(plan, &mut state, pass2_budget);
            budget.used += state.tokens_used - tokens_before;
        }

        // Pass 3: Polish (only in full budget mode).
        let mode = super::budget::budget_mode(budget);
        if matches!(mode, BudgetMode::Full) {
            let pass3_budget = (plan.estimated_tokens as f64 * PASS3_FRACTION) as u64;
            let tokens_before = state.tokens_used;
            Self::pass_polish(plan, &mut state, pass3_budget);
            budget.used += state.tokens_used - tokens_before;
        }

        let commit_msg = Self::build_commit_message(plan, &state);

        PatchOutput {
            patch: state.patch,
            commit_msg,
            files_touched: state.files,
            tokens_used: state.tokens_used,
        }
    }

    /// Pass 1: Rough implementation.
    ///
    /// Produces the initial patch from the plan's approach and file list.
    /// This is the minimum viable output -- if only this pass runs, the
    /// result should still be a valid (if rough) implementation.
    fn pass_rough(plan: &TaskPlan, state: &mut ShuttleState, budget: u64) {
        let mut patch = String::new();

        // Generate patch header.
        for file in &plan.files {
            patch.push_str(&format!("diff --git a/{file} b/{file}\n"));
            patch.push_str(&format!("--- a/{file}\n"));
            patch.push_str(&format!("+++ b/{file}\n"));
            patch.push_str("@@ -1,1 +1,1 @@\n");
            patch.push_str(&format!("+// TODO: implement changes for {file}\n"));
        }

        if plan.files.is_empty() {
            patch.push_str("# No files identified in plan -- manual implementation needed\n");
        }

        // Simulate token consumption proportional to output size.
        let tokens = (patch.len() as u64 / 4).min(budget);
        state.patch = patch;
        state.tokens_used += tokens;
        state.passes_completed = 1;
    }

    /// Pass 2: Refinement.
    ///
    /// Refines the rough patch: improves structure, adds missing imports,
    /// handles common edge cases. In a real LLM-backed implementation,
    /// this would re-process the diff with refinement instructions.
    fn pass_refine(plan: &TaskPlan, state: &mut ShuttleState, budget: u64) {
        // Add refinement markers to the patch.
        let refinement_note = format!(
            "\n# Refinement pass: complexity={:?}, files={}\n",
            plan.complexity,
            plan.files.len(),
        );

        state.patch.push_str(&refinement_note);

        // Simulate token consumption.
        let tokens = (refinement_note.len() as u64 / 4).min(budget);
        state.tokens_used += tokens;
        state.passes_completed = 2;
    }

    /// Pass 3: Polish.
    ///
    /// Final pass for edge cases, documentation, and cleanup.
    /// Only runs when budget is ample (Full mode).
    fn pass_polish(_plan: &TaskPlan, state: &mut ShuttleState, budget: u64) {
        let polish_note = "\n# Polish pass: edge cases and documentation reviewed\n";
        state.patch.push_str(polish_note);

        let tokens = (polish_note.len() as u64 / 4).min(budget);
        state.tokens_used += tokens;
        state.passes_completed = 3;
    }

    /// Build a commit message with metadata trailers.
    fn build_commit_message(plan: &TaskPlan, state: &ShuttleState) -> String {
        let complexity_label = match plan.complexity {
            Complexity::Simple => "simple",
            Complexity::Moderate => "moderate",
            Complexity::Complex => "complex",
        };

        let title = if plan.approach.len() > 72 {
            format!("{}...", &plan.approach[..69])
        } else {
            plan.approach.clone()
        };

        let mut msg = title;
        msg.push_str("\n\n");
        msg.push_str(&format!(
            "Shuttle passes: {}/3\n",
            state.passes_completed
        ));
        msg.push_str(&format!("Complexity: {complexity_label}\n"));
        msg.push_str(&format!("Tokens used: {}\n", state.tokens_used));
        msg.push_str(&format!(
            "Files touched: {}\n",
            state.files.len()
        ));

        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_plan() -> TaskPlan {
        TaskPlan {
            approach: "Fix typo in README".into(),
            files: vec!["README.md".into()],
            complexity: Complexity::Simple,
            estimated_tokens: 4_000,
        }
    }

    fn complex_plan() -> TaskPlan {
        TaskPlan {
            approach: "Refactor authentication middleware across three crates".into(),
            files: vec![
                "crates/auth/src/lib.rs".into(),
                "crates/api/src/middleware.rs".into(),
                "crates/web/src/session.rs".into(),
            ],
            complexity: Complexity::Complex,
            estimated_tokens: 24_000,
        }
    }

    #[test]
    fn implement_simple_full_budget() {
        let plan = simple_plan();
        let mut budget = TokenBudget::new(32_000);
        let output = Implementer::implement(&plan, &mut budget);

        assert!(!output.patch.is_empty());
        assert!(output.patch.contains("README.md"));
        assert!(output.tokens_used > 0);
        assert_eq!(output.files_touched, vec!["README.md"]);
        // Full budget -> all 3 passes should complete.
        assert!(output.patch.contains("Polish pass"));
    }

    #[test]
    fn implement_with_tight_budget() {
        let plan = complex_plan();
        // Give just enough for pass 1, maybe pass 2, but not pass 3.
        let mut budget = TokenBudget::new(10_000);
        budget.used = 5_500; // 45% remaining -> Abbreviated mode
        let output = Implementer::implement(&plan, &mut budget);

        assert!(!output.patch.is_empty());
        // Should not contain polish pass.
        assert!(!output.patch.contains("Polish pass"));
    }

    #[test]
    fn implement_emergency_halt() {
        let plan = simple_plan();
        let mut budget = TokenBudget::new(10_000);
        budget.used = 8_500; // 85% used -> EmergencyHalt
        let output = Implementer::implement(&plan, &mut budget);

        // Emergency halt: no passes should run.
        assert!(output.patch.is_empty());
        assert_eq!(output.tokens_used, 0);
    }

    #[test]
    fn commit_message_includes_metadata() {
        let plan = simple_plan();
        let mut budget = TokenBudget::new(32_000);
        let output = Implementer::implement(&plan, &mut budget);

        assert!(output.commit_msg.contains("Shuttle passes:"));
        assert!(output.commit_msg.contains("Complexity: simple"));
        assert!(output.commit_msg.contains("Tokens used:"));
    }
}
