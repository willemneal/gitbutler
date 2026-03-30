//! Hazard rate evaluation and lifecycle state classification.
//!
//! The hazard function h(t) = f(t)/S(t) gives the instantaneous rate of
//! irrelevance at time t. This module classifies memory entries into
//! lifecycle states based on their survival probability.

use crate::types::{MemoryEntry, MemoryState, SurvivalDistribution, SurvivalMetadata};

/// Thresholds for lifecycle state transitions.
pub const ALIVE_THRESHOLD: f64 = 0.25;
pub const DECEASED_THRESHOLD: f64 = 0.10;

/// Determine the lifecycle state from a survival probability.
///
/// - S(t) >= 0.25 -> Alive
/// - 0.10 <= S(t) < 0.25 -> Moribund
/// - S(t) < 0.10 -> Deceased
pub fn classify_state(survival_probability: f64) -> MemoryState {
    if survival_probability >= ALIVE_THRESHOLD {
        MemoryState::Alive
    } else if survival_probability >= DECEASED_THRESHOLD {
        MemoryState::Moribund
    } else {
        MemoryState::Deceased
    }
}

/// Hazard classification labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardClass {
    Negligible,
    Low,
    Moderate,
    Elevated,
    Critical,
}

impl std::fmt::Display for HazardClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Negligible => write!(f, "negligible"),
            Self::Low => write!(f, "low"),
            Self::Moderate => write!(f, "moderate"),
            Self::Elevated => write!(f, "elevated"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Classify a hazard rate value into a risk grade.
pub fn classify_hazard(h: f64) -> HazardClass {
    if h < 0.005 {
        HazardClass::Negligible
    } else if h < 0.02 {
        HazardClass::Low
    } else if h < 0.05 {
        HazardClass::Moderate
    } else if h < 0.10 {
        HazardClass::Elevated
    } else {
        HazardClass::Critical
    }
}

/// Evaluate survival statistics for a distribution at a given time.
///
/// Returns updated `SurvivalMetadata` with current probability and hazard rate.
/// The `surprise_index` and `goodness_of_fit` fields are carried forward.
pub fn evaluate_at(
    dist: &SurvivalDistribution,
    days_since_creation: f64,
    existing: &SurvivalMetadata,
) -> SurvivalMetadata {
    SurvivalMetadata {
        distribution: dist.clone(),
        current_probability: dist.survival_probability(days_since_creation),
        hazard_rate: dist.hazard_rate(days_since_creation),
        surprise_index: existing.surprise_index,
        goodness_of_fit: existing.goodness_of_fit,
    }
}

/// Compute the median remaining lifetime given survival up to `days_elapsed`.
///
/// Finds t* such that S(t* + days_elapsed) / S(days_elapsed) = 0.5
/// using bisection.
pub fn median_remaining_lifetime(
    dist: &SurvivalDistribution,
    days_elapsed: f64,
) -> f64 {
    let current = dist.survival_probability(days_elapsed);
    if current <= 1e-15 {
        return 0.0;
    }

    let target = 0.5 * current;

    let mut lo = days_elapsed;
    let mut hi = days_elapsed + 1.0;
    while dist.survival_probability(hi) > target && hi < days_elapsed + 1e6 {
        hi *= 2.0;
    }

    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        if dist.survival_probability(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    (lo + hi) / 2.0 - days_elapsed
}

/// Produce a one-line mortality summary for a memory entry.
pub fn mortality_summary(entry: &MemoryEntry) -> String {
    let hazard_class = classify_hazard(entry.survival.hazard_rate);
    format!(
        "Memory {} [{}]: S(t)={:.3}, h(t)={:.4} [{}], state={:?}",
        entry.id,
        entry.classification.call_number,
        entry.survival.current_probability,
        entry.survival.hazard_rate,
        hazard_class,
        entry.state,
    )
}

/// Batch mortality summary for multiple entries.
pub fn batch_mortality_summary(entries: &[&MemoryEntry]) -> Vec<String> {
    entries.iter().map(|e| mortality_summary(e)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SurvivalDistribution;

    #[test]
    fn lifecycle_state_thresholds() {
        assert_eq!(classify_state(0.50), MemoryState::Alive);
        assert_eq!(classify_state(0.25), MemoryState::Alive);
        assert_eq!(classify_state(0.15), MemoryState::Moribund);
        assert_eq!(classify_state(0.10), MemoryState::Moribund);
        assert_eq!(classify_state(0.05), MemoryState::Deceased);
    }

    #[test]
    fn hazard_classification() {
        assert_eq!(classify_hazard(0.001), HazardClass::Negligible);
        assert_eq!(classify_hazard(0.01), HazardClass::Low);
        assert_eq!(classify_hazard(0.03), HazardClass::Moderate);
        assert_eq!(classify_hazard(0.07), HazardClass::Elevated);
        assert_eq!(classify_hazard(0.15), HazardClass::Critical);
    }

    #[test]
    fn median_remaining_for_exponential() {
        let dist = SurvivalDistribution::Exponential { lambda: 0.1 };
        // Exponential is memoryless: median remaining = ln(2)/lambda always.
        let remaining = median_remaining_lifetime(&dist, 10.0);
        let expected = (2.0_f64).ln() / 0.1;
        assert!((remaining - expected).abs() < 0.5);
    }
}
