//! Bayesian surprise detection for memory relevance updates.
//!
//! The surprise index measures how much an observed access pattern deviates
//! from what the fitted survival distribution predicted. It is based on the
//! KL divergence between predicted and observed access interval distributions.
//!
//! When surprise exceeds a threshold, the memory is flagged for re-fitting
//! and a cohort review checks for systematic shifts.

use crate::types::SurvivalDistribution;

/// Compute the surprise index for a memory entry.
///
/// The surprise index is D_KL(observed || predicted), discretized into bins.
///
/// Interpretation:
/// - < 0.1: model fits well, no surprise.
/// - 0.1 - 0.5: mild surprise, worth monitoring.
/// - > 0.5: significant surprise, triggers cohort review.
pub fn compute_surprise_index(
    intervals: &[f64],
    distribution: &SurvivalDistribution,
) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    let observed = empirical_distribution(intervals);
    let predicted = predicted_distribution(distribution, &observed.bin_edges);
    kl_divergence(&observed.probabilities, &predicted)
}

/// Whether a cohort review should be triggered.
pub fn should_trigger_cohort_review(surprise_index: f64, threshold: f64) -> bool {
    surprise_index > threshold
}

/// Bayesian posterior update factor for a memory's survival parameters.
///
/// If a memory is accessed when S(t) is low, the observation is surprising
/// and the memory should be "resuscitated". Factor > 1.0 means more relevant
/// than predicted.
pub fn bayesian_update_factor(survival_probability_at_access: f64) -> f64 {
    if survival_probability_at_access <= 1e-15 {
        return 10.0;
    }
    (1.0 / survival_probability_at_access).min(10.0)
}

/// Aggregate surprise index for a cohort of memories.
///
/// Weighted by inverse goodness-of-fit: poorly-fitted memories contribute
/// more because they are more likely to be genuinely misspecified.
pub fn cohort_surprise(
    individual_surprises: &[(f64, f64)], // (surprise_index, goodness_of_fit)
) -> f64 {
    if individual_surprises.is_empty() {
        return 0.0;
    }

    let total_weight: f64 = individual_surprises
        .iter()
        .map(|(_, gof)| 1.0 / gof.max(0.01))
        .sum();

    if total_weight <= 1e-15 {
        return 0.0;
    }

    let weighted_sum: f64 = individual_surprises
        .iter()
        .map(|(surprise, gof)| surprise * (1.0 / gof.max(0.01)))
        .sum();

    weighted_sum / total_weight
}

/// Detect whether a set of surprise values exhibits a cohort effect.
///
/// A cohort effect is when multiple memories from the same creation period
/// simultaneously show elevated surprise -- a systematic shift rather than
/// individual failures.
pub fn detect_cohort_effect(
    surprise_values: &[f64],
    surprise_threshold: f64,
    proportion_threshold: f64,
) -> bool {
    if surprise_values.is_empty() {
        return false;
    }

    let high_surprise_count = surprise_values
        .iter()
        .filter(|&&s| s > surprise_threshold)
        .count();

    let proportion = high_surprise_count as f64 / surprise_values.len() as f64;
    proportion > proportion_threshold
}

/// Expected surprise under the null hypothesis (model is correct).
///
/// Approximately (k-1)/(2n) where k is the number of bins.
pub fn expected_surprise_under_null(n_samples: usize, n_bins: usize) -> f64 {
    if n_samples == 0 {
        return 0.0;
    }
    (n_bins.saturating_sub(1)) as f64 / (2.0 * n_samples as f64)
}

// ---------------------------------------------------------------------------
// Internal
// ---------------------------------------------------------------------------

struct EmpiricalDistribution {
    bin_edges: Vec<f64>,
    probabilities: Vec<f64>,
}

/// Construct an empirical distribution using Sturges' rule for bin count.
fn empirical_distribution(intervals: &[f64]) -> EmpiricalDistribution {
    let n = intervals.len();
    let n_bins = ((1.0 + (n as f64).log2()).ceil() as usize).max(2);

    let min_val = intervals.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = intervals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let range = (max_val - min_val).max(1e-6);
    let bin_width = range / n_bins as f64;

    let mut bin_edges = Vec::with_capacity(n_bins + 1);
    for i in 0..=n_bins {
        bin_edges.push(min_val + i as f64 * bin_width);
    }
    if let Some(last) = bin_edges.last_mut() {
        *last = max_val + 1e-10;
    }

    let mut counts = vec![0usize; n_bins];
    for &interval in intervals {
        let bin = ((interval - min_val) / bin_width).floor() as usize;
        let bin = bin.min(n_bins - 1);
        counts[bin] += 1;
    }

    // Laplace smoothing.
    let total = n as f64 + n_bins as f64;
    let probabilities: Vec<f64> = counts
        .iter()
        .map(|&c| (c as f64 + 1.0) / total)
        .collect();

    EmpiricalDistribution {
        bin_edges,
        probabilities,
    }
}

/// Predicted bin probabilities from the survival distribution.
fn predicted_distribution(dist: &SurvivalDistribution, bin_edges: &[f64]) -> Vec<f64> {
    let n_bins = bin_edges.len() - 1;
    let mut probabilities = Vec::with_capacity(n_bins);

    for i in 0..n_bins {
        let p = (dist.survival_probability(bin_edges[i])
            - dist.survival_probability(bin_edges[i + 1]))
        .max(1e-10);
        probabilities.push(p);
    }

    let total: f64 = probabilities.iter().sum();
    if total > 1e-15 {
        for p in &mut probabilities {
            *p /= total;
        }
    }

    probabilities
}

/// KL divergence D_KL(P || Q).
fn kl_divergence(p: &[f64], q: &[f64]) -> f64 {
    assert_eq!(p.len(), q.len());
    p.iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| {
            if pi < 1e-15 {
                return 0.0;
            }
            let qi_safe = qi.max(1e-15);
            pi * (pi / qi_safe).ln()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kl_divergence_of_identical_is_zero() {
        let p = vec![0.25, 0.25, 0.25, 0.25];
        assert!(kl_divergence(&p, &p).abs() < 1e-10);
    }

    #[test]
    fn kl_divergence_is_non_negative() {
        let p = vec![0.5, 0.3, 0.2];
        let q = vec![0.33, 0.34, 0.33];
        assert!(kl_divergence(&p, &q) >= -1e-10);
    }

    #[test]
    fn bayesian_update_low_survival_is_high() {
        assert!(bayesian_update_factor(0.1) > 5.0);
    }

    #[test]
    fn bayesian_update_high_survival_is_low() {
        assert!(bayesian_update_factor(0.9) < 2.0);
    }

    #[test]
    fn cohort_effect_detected() {
        let surprises = vec![0.8, 0.6, 0.7, 0.9, 0.1];
        assert!(detect_cohort_effect(&surprises, 0.5, 0.5));
    }

    #[test]
    fn no_cohort_effect_when_minority() {
        let surprises = vec![0.1, 0.2, 0.1, 0.8, 0.1];
        assert!(!detect_cohort_effect(&surprises, 0.5, 0.5));
    }
}
