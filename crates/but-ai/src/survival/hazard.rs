//! Hazard rate computation and mortality classification.
//!
//! The hazard function h(t) = f(t)/S(t) gives the instantaneous rate of
//! irrelevance at time t, given that the memory has survived until t.
//! This module computes hazard rates for memory entries and classifies
//! them into actuarial risk grades.

use crate::survival::distributions;
use crate::types::{
    CompactionTier, HazardClassification, HazardRate, LifecycleState, MemoryEntry,
    SurvivalDistribution,
};

/// Hazard classification thresholds (calibrated from actuarial practice).
const NEGLIGIBLE_THRESHOLD: f64 = 0.005;
const LOW_THRESHOLD: f64 = 0.02;
const MODERATE_THRESHOLD: f64 = 0.05;
const ELEVATED_THRESHOLD: f64 = 0.10;

/// Evaluate the survival function S(t) for a distribution at a given time.
pub fn evaluate_survival(dist: &SurvivalDistribution, days_since_creation: f64) -> anyhow::Result<f64> {
    let sf = distributions::from_distribution(dist)?;
    Ok(sf.survival(days_since_creation))
}

/// Evaluate the hazard function h(t) for a distribution at a given time.
pub fn evaluate_hazard(dist: &SurvivalDistribution, days_since_creation: f64) -> anyhow::Result<HazardRate> {
    let sf = distributions::from_distribution(dist)?;
    let value = sf.hazard(days_since_creation);
    let classification = classify_hazard(value);

    Ok(HazardRate {
        value,
        classification,
        evaluated_at_days: days_since_creation,
    })
}

/// Classify a hazard rate value into an actuarial risk grade.
pub fn classify_hazard(h: f64) -> HazardClassification {
    if h < NEGLIGIBLE_THRESHOLD {
        HazardClassification::Negligible
    } else if h < LOW_THRESHOLD {
        HazardClassification::Low
    } else if h < MODERATE_THRESHOLD {
        HazardClassification::Moderate
    } else if h < ELEVATED_THRESHOLD {
        HazardClassification::Elevated
    } else {
        HazardClassification::Critical
    }
}

/// Determine the lifecycle state of a memory entry based on its survival probability.
pub fn determine_lifecycle_state(
    survival_probability: f64,
    alive_threshold: f64,
    deceased_threshold: f64,
) -> LifecycleState {
    if survival_probability >= alive_threshold {
        LifecycleState::Alive
    } else if survival_probability >= deceased_threshold {
        LifecycleState::Moribund
    } else {
        LifecycleState::Deceased
    }
}

/// Determine the compaction tier for a memory based on survival probability.
///
/// - Full (S(t) > 0.75): complete content retained in context.
/// - Summary (0.25 < S(t) <= 0.75): practitioner summary only.
/// - Skeleton (S(t) <= 0.25): embedding + metadata only.
pub fn compaction_tier(survival_probability: f64) -> CompactionTier {
    if survival_probability > 0.75 {
        CompactionTier::Full
    } else if survival_probability > 0.25 {
        CompactionTier::Summary
    } else {
        CompactionTier::Skeleton
    }
}

/// Update a memory entry's survival statistics given the current time.
///
/// Recomputes `current_survival_probability`, `current_hazard_rate`,
/// and `lifecycle_state` based on the entry's fitted distribution
/// and elapsed time.
pub fn update_survival_statistics(
    entry: &mut MemoryEntry,
    days_since_creation: f64,
    alive_threshold: f64,
    deceased_threshold: f64,
) -> anyhow::Result<()> {
    let sf = distributions::from_distribution(&entry.survival_distribution)?;

    entry.current_survival_probability = sf.survival(days_since_creation);
    entry.current_hazard_rate = sf.hazard(days_since_creation);
    entry.lifecycle_state = determine_lifecycle_state(
        entry.current_survival_probability,
        alive_threshold,
        deceased_threshold,
    );

    Ok(())
}

/// Compute the hazard-adjusted recency score for a memory entry.
///
/// Combines time since last access with the current hazard rate.
/// A recently accessed memory with a low hazard rate scores highly.
/// A recently accessed memory with a high hazard rate scores moderately.
///
/// Formula: recency_score * (1 - hazard_penalty)
/// where recency_score = exp(-decay * days_since_access)
/// and hazard_penalty = min(1, hazard_rate / critical_threshold)
pub fn hazard_adjusted_recency(
    days_since_last_access: f64,
    hazard_rate: f64,
    recency_decay: f64,
) -> f64 {
    let recency_score = (-recency_decay * days_since_last_access).exp();
    let hazard_penalty = (hazard_rate / ELEVATED_THRESHOLD).min(1.0);
    recency_score * (1.0 - 0.5 * hazard_penalty)
}

/// Compute median remaining lifetime for a memory entry.
///
/// Given that the memory has survived until `days_elapsed`, what is the
/// conditional median remaining time?
///
/// For many distributions this requires numerical inversion:
/// find t* such that S(t* + days_elapsed) / S(days_elapsed) = 0.5
pub fn median_remaining_lifetime(
    dist: &SurvivalDistribution,
    days_elapsed: f64,
) -> anyhow::Result<f64> {
    let sf = distributions::from_distribution(dist)?;
    let current_survival = sf.survival(days_elapsed);

    if current_survival <= 1e-15 {
        return Ok(0.0);
    }

    let target = 0.5 * current_survival;

    // Bisection: find t such that S(t) = target.
    let mut lo = days_elapsed;
    let mut hi = days_elapsed + 1.0;
    while sf.survival(hi) > target && hi < days_elapsed + 1e6 {
        hi *= 2.0;
    }

    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if sf.survival(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    Ok((lo + hi) / 2.0 - days_elapsed)
}

/// Produce a mortality summary string for a memory entry.
///
/// This is the kind of summary Vassiliev would produce: precise,
/// quantitative, and appropriately uncertain.
pub fn mortality_summary(entry: &MemoryEntry) -> String {
    let classification = classify_hazard(entry.current_hazard_rate);
    let tier = compaction_tier(entry.current_survival_probability);

    format!(
        "Memory {} ({}): S(t)={:.3}, h(t)={:.4} [{}], state={}, compaction={}",
        entry.id.0,
        format_memory_type(entry.memory_type),
        entry.current_survival_probability,
        entry.current_hazard_rate,
        format_hazard_class(classification),
        format_lifecycle(entry.lifecycle_state),
        format_compaction(tier),
    )
}

fn format_memory_type(mt: crate::types::MemoryType) -> &'static str {
    match mt {
        crate::types::MemoryType::Architectural => "architectural",
        crate::types::MemoryType::BugFix => "bug/fix",
        crate::types::MemoryType::Convention => "convention",
        crate::types::MemoryType::Dependency => "dependency",
        crate::types::MemoryType::TaskContext => "task-context",
        crate::types::MemoryType::CrossRepo => "cross-repo",
    }
}

fn format_hazard_class(c: HazardClassification) -> &'static str {
    match c {
        HazardClassification::Negligible => "negligible",
        HazardClassification::Low => "low",
        HazardClassification::Moderate => "moderate",
        HazardClassification::Elevated => "elevated",
        HazardClassification::Critical => "CRITICAL",
    }
}

fn format_lifecycle(s: LifecycleState) -> &'static str {
    match s {
        LifecycleState::Alive => "alive",
        LifecycleState::Moribund => "moribund",
        LifecycleState::Deceased => "deceased",
    }
}

fn format_compaction(t: CompactionTier) -> &'static str {
    match t {
        CompactionTier::Full => "full",
        CompactionTier::Summary => "summary",
        CompactionTier::Skeleton => "skeleton",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DistributionFamily, DistributionParameters};

    fn test_weibull_dist() -> SurvivalDistribution {
        SurvivalDistribution {
            family: DistributionFamily::Weibull,
            parameters: DistributionParameters::Weibull {
                k: 1.8,
                lambda: 180.0,
            },
            fitted_at: String::new(),
            goodness_of_fit: 0.85,
        }
    }

    #[test]
    fn survival_at_zero_is_one() {
        let s = evaluate_survival(&test_weibull_dist(), 0.0).unwrap();
        assert!((s - 1.0).abs() < 1e-10);
    }

    #[test]
    fn survival_decreases_over_time() {
        let dist = test_weibull_dist();
        let s1 = evaluate_survival(&dist, 30.0).unwrap();
        let s2 = evaluate_survival(&dist, 90.0).unwrap();
        let s3 = evaluate_survival(&dist, 180.0).unwrap();
        assert!(s1 > s2);
        assert!(s2 > s3);
    }

    #[test]
    fn hazard_classification_thresholds() {
        assert_eq!(classify_hazard(0.001), HazardClassification::Negligible);
        assert_eq!(classify_hazard(0.01), HazardClassification::Low);
        assert_eq!(classify_hazard(0.03), HazardClassification::Moderate);
        assert_eq!(classify_hazard(0.07), HazardClassification::Elevated);
        assert_eq!(classify_hazard(0.15), HazardClassification::Critical);
    }

    #[test]
    fn lifecycle_state_determination() {
        assert_eq!(
            determine_lifecycle_state(0.50, 0.25, 0.10),
            LifecycleState::Alive
        );
        assert_eq!(
            determine_lifecycle_state(0.15, 0.25, 0.10),
            LifecycleState::Moribund
        );
        assert_eq!(
            determine_lifecycle_state(0.05, 0.25, 0.10),
            LifecycleState::Deceased
        );
    }
}
