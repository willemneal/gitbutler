//! Performance history tracking for agents.
//!
//! Tracks task completion statistics and provides running mean
//! calculations for confidence and patch survival.

use crate::types::PerformanceHistory;

/// Update a performance history after a task completion.
///
/// Uses a running mean so we don't need to store all historical values.
pub fn record_task_completion(
    history: &mut PerformanceHistory,
    confidence: f64,
    patch_survival_days: f64,
) {
    let n = history.tasks_completed as f64;
    history.mean_confidence = (history.mean_confidence * n + confidence) / (n + 1.0);
    history.mean_patch_survival_days =
        (history.mean_patch_survival_days * n + patch_survival_days) / (n + 1.0);
    history.tasks_completed += 1;
}

/// Compute the reliability score for an agent based on performance history.
///
/// A composite score in [0, 1] that combines:
/// - Task volume (more tasks = more reliable signal)
/// - Mean confidence (higher = better)
/// - Mean patch survival (longer = better)
///
/// The score is conservative for agents with few tasks (low confidence
/// in the estimate).
pub fn reliability_score(history: &PerformanceHistory) -> f64 {
    if history.tasks_completed == 0 {
        return 0.0;
    }

    // Volume factor: logarithmic growth, saturates around 50 tasks.
    let volume_factor = (1.0 + history.tasks_completed as f64).ln() / (1.0 + 50.0_f64).ln();
    let volume_factor = volume_factor.min(1.0);

    // Confidence factor: direct mapping.
    let confidence_factor = history.mean_confidence.clamp(0.0, 1.0);

    // Survival factor: normalized against a 90-day benchmark.
    let survival_factor = (history.mean_patch_survival_days / 90.0).min(1.0);

    // Weighted combination.
    0.3 * volume_factor + 0.4 * confidence_factor + 0.3 * survival_factor
}

/// Check if an agent has sufficient track record for autonomous operation.
///
/// Requires a minimum number of completed tasks and a minimum mean confidence.
pub fn has_sufficient_track_record(
    history: &PerformanceHistory,
    min_tasks: u64,
    min_confidence: f64,
) -> bool {
    history.tasks_completed >= min_tasks && history.mean_confidence >= min_confidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_single_task() {
        let mut h = PerformanceHistory::default();
        record_task_completion(&mut h, 0.9, 180.0);

        assert_eq!(h.tasks_completed, 1);
        assert!((h.mean_confidence - 0.9).abs() < 1e-10);
        assert!((h.mean_patch_survival_days - 180.0).abs() < 1e-10);
    }

    #[test]
    fn running_mean_correct_after_two_tasks() {
        let mut h = PerformanceHistory::default();
        record_task_completion(&mut h, 0.90, 180.0);
        record_task_completion(&mut h, 0.80, 120.0);

        assert_eq!(h.tasks_completed, 2);
        assert!((h.mean_confidence - 0.85).abs() < 1e-10);
        assert!((h.mean_patch_survival_days - 150.0).abs() < 1e-10);
    }

    #[test]
    fn reliability_zero_for_no_tasks() {
        let h = PerformanceHistory::default();
        assert!((reliability_score(&h)).abs() < 1e-10);
    }

    #[test]
    fn reliability_increases_with_tasks() {
        let mut h = PerformanceHistory::default();
        record_task_completion(&mut h, 0.9, 90.0);
        let score1 = reliability_score(&h);

        for _ in 0..10 {
            record_task_completion(&mut h, 0.9, 90.0);
        }
        let score2 = reliability_score(&h);

        assert!(score2 > score1);
    }

    #[test]
    fn track_record_check() {
        let mut h = PerformanceHistory::default();
        assert!(!has_sufficient_track_record(&h, 5, 0.8));

        for _ in 0..5 {
            record_task_completion(&mut h, 0.9, 90.0);
        }
        assert!(has_sufficient_track_record(&h, 5, 0.8));
        assert!(!has_sufficient_track_record(&h, 10, 0.8));
    }
}
