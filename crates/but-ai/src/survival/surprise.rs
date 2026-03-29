//! Bayesian surprise detection for memory relevance updates.
//!
//! The surprise index measures how much an observed access pattern
//! deviates from what the fitted survival distribution predicted.
//! It is based on the Kullback-Leibler divergence between the
//! predicted and observed access interval distributions.
//!
//! When surprise exceeds a threshold, the memory's survival distribution
//! is flagged for re-fitting, and a cohort review is triggered to check
//! whether the surprise indicates a systematic shift affecting related
//! memories.

use crate::types::{AccessRecord, SurvivalDistribution};

/// Compute the surprise index for a memory entry.
///
/// The surprise index is the KL divergence D_KL(observed || predicted),
/// discretized into interval bins. Higher values indicate greater
/// deviation from the model's predictions.
///
/// Returns a value in [0, +inf). Practical interpretation:
/// - < 0.1: model fits well, no surprise.
/// - 0.1 - 0.5: mild surprise, worth monitoring.
/// - > 0.5: significant surprise, triggers cohort review.
pub fn compute_surprise_index(
    access_history: &[AccessRecord],
    created_at: &str,
    distribution: &SurvivalDistribution,
) -> f64 {
    let intervals = super::fitting::compute_intervals(access_history, created_at);
    if intervals.len() < 2 {
        return 0.0; // Insufficient data for surprise estimation.
    }

    // Compute observed interval distribution (empirical).
    let observed = empirical_distribution(&intervals);

    // Compute predicted interval distribution from the fitted model.
    let predicted = predicted_distribution(distribution, &observed.bin_edges);

    // KL divergence: sum_i p_obs(i) * ln(p_obs(i) / p_pred(i))
    kl_divergence(&observed.probabilities, &predicted)
}

/// Determine whether a cohort review should be triggered.
///
/// A cohort review examines all memories created in the same period
/// as the surprising memory, because cohort effects (a batch of
/// memories becoming simultaneously stale) are more common than
/// individual failures.
pub fn should_trigger_cohort_review(surprise_index: f64, threshold: f64) -> bool {
    surprise_index > threshold
}

/// Compute the Bayesian posterior update factor for a memory's
/// survival parameters given new access evidence.
///
/// If a memory is accessed when its survival probability is low,
/// the posterior update is large (the observation is surprising
/// and the memory should be "resuscitated"). If accessed when
/// survival is high, the update is small (expected behavior).
///
/// Returns a multiplicative factor for the survival probability:
/// > 1.0 means the memory is more relevant than predicted.
/// < 1.0 means the memory is less relevant (unlikely in practice,
/// since we only observe accesses, not non-accesses).
pub fn bayesian_update_factor(survival_probability_at_access: f64) -> f64 {
    // The update is based on the likelihood ratio.
    // Observing an access when S(t) is low is more informative.
    // Factor = 1 / S(t), clamped to a reasonable range.
    if survival_probability_at_access <= 1e-15 {
        return 10.0; // Maximum resuscitation factor.
    }
    (1.0 / survival_probability_at_access).min(10.0)
}

/// Compute the aggregate surprise index for a cohort of memories.
///
/// The cohort surprise is the mean of individual surprise indices,
/// weighted by the inverse of each memory's goodness-of-fit
/// (poorly-fitted memories contribute more to cohort surprise
/// because they are more likely to be genuinely misspecified).
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Empirical distribution of inter-access intervals.
struct EmpiricalDistribution {
    /// Bin edges (n_bins + 1 values).
    bin_edges: Vec<f64>,
    /// Probability mass in each bin (sums to 1).
    probabilities: Vec<f64>,
}

/// Construct an empirical distribution from observed intervals.
///
/// Uses Sturges' rule for bin count: k = ceil(1 + log2(n)).
fn empirical_distribution(intervals: &[f64]) -> EmpiricalDistribution {
    let n = intervals.len();
    let n_bins = ((1.0 + (n as f64).log2()).ceil() as usize).max(2);

    let min_val = intervals.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = intervals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Add a small epsilon to ensure all points fall within bins.
    let range = (max_val - min_val).max(1e-6);
    let bin_width = range / n_bins as f64;

    let mut bin_edges = Vec::with_capacity(n_bins + 1);
    for i in 0..=n_bins {
        bin_edges.push(min_val + i as f64 * bin_width);
    }
    // Ensure the last edge captures all points.
    if let Some(last) = bin_edges.last_mut() {
        *last = max_val + 1e-10;
    }

    let mut counts = vec![0usize; n_bins];
    for &interval in intervals {
        let bin = ((interval - min_val) / bin_width).floor() as usize;
        let bin = bin.min(n_bins - 1);
        counts[bin] += 1;
    }

    // Convert to probabilities with Laplace smoothing.
    let total = n as f64 + n_bins as f64; // Laplace: add 1 to each bin.
    let probabilities: Vec<f64> = counts
        .iter()
        .map(|&c| (c as f64 + 1.0) / total)
        .collect();

    EmpiricalDistribution {
        bin_edges,
        probabilities,
    }
}

/// Compute predicted bin probabilities from the survival distribution.
///
/// For each bin [a, b], the predicted probability is S(a) - S(b),
/// i.e. the probability of the event occurring in that interval.
fn predicted_distribution(dist: &SurvivalDistribution, bin_edges: &[f64]) -> Vec<f64> {
    use crate::survival::distributions::{self as dist_mod};

    let sf = match dist_mod::from_distribution(dist) {
        Ok(sf) => sf,
        Err(_) => return vec![1.0 / (bin_edges.len() - 1) as f64; bin_edges.len() - 1],
    };

    let n_bins = bin_edges.len() - 1;
    let mut probabilities = Vec::with_capacity(n_bins);

    for i in 0..n_bins {
        let p = (sf.survival(bin_edges[i]) - sf.survival(bin_edges[i + 1])).max(1e-10);
        probabilities.push(p);
    }

    // Normalize.
    let total: f64 = probabilities.iter().sum();
    if total > 1e-15 {
        for p in &mut probabilities {
            *p /= total;
        }
    }

    probabilities
}

/// KL divergence D_KL(P || Q) = sum_i P(i) * ln(P(i) / Q(i)).
///
/// Both vectors must be the same length and represent probability distributions.
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

/// Detect whether a set of surprise values exhibits a cohort effect.
///
/// A cohort effect is when multiple memories from the same creation
/// period simultaneously show elevated surprise -- indicating a
/// systematic shift rather than individual failures.
///
/// Returns true if the proportion of high-surprise memories exceeds
/// a threshold (default: 50% of the cohort).
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

/// Estimate the expected surprise for a well-fitted distribution.
///
/// Under the null hypothesis (model is correct), the expected KL divergence
/// between the empirical distribution (n samples) and the true distribution
/// is approximately (k-1)/(2n) where k is the number of bins.
///
/// This provides a calibration baseline: surprise values near or below
/// this level are consistent with the model being correct.
pub fn expected_surprise_under_null(n_samples: usize, n_bins: usize) -> f64 {
    if n_samples == 0 {
        return 0.0;
    }
    (n_bins.saturating_sub(1)) as f64 / (2.0 * n_samples as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kl_divergence_of_identical_distributions_is_zero() {
        let p = vec![0.25, 0.25, 0.25, 0.25];
        let d = kl_divergence(&p, &p);
        assert!(d.abs() < 1e-10);
    }

    #[test]
    fn kl_divergence_is_non_negative() {
        let p = vec![0.5, 0.3, 0.2];
        let q = vec![0.33, 0.34, 0.33];
        let d = kl_divergence(&p, &q);
        assert!(d >= -1e-10);
    }

    #[test]
    fn bayesian_update_for_low_survival_is_high() {
        let factor = bayesian_update_factor(0.1);
        assert!(factor > 5.0);
    }

    #[test]
    fn bayesian_update_for_high_survival_is_low() {
        let factor = bayesian_update_factor(0.9);
        assert!(factor < 2.0);
    }

    #[test]
    fn cohort_effect_detected_when_majority_surprised() {
        let surprises = vec![0.8, 0.6, 0.7, 0.9, 0.1];
        assert!(detect_cohort_effect(&surprises, 0.5, 0.5));
    }

    #[test]
    fn no_cohort_effect_when_minority_surprised() {
        let surprises = vec![0.1, 0.2, 0.1, 0.8, 0.1];
        assert!(!detect_cohort_effect(&surprises, 0.5, 0.5));
    }
}
