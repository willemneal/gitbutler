//! Survival analysis module.
//!
//! Implements the statistical core of the actuarial-table memory system:
//!
//! - **distributions**: Methods on `SurvivalDistribution` for S(t), f(t), h(t).
//! - **fitting**: MLE fitting from access history, AIC model selection.
//! - **hazard**: Hazard rate evaluation, lifecycle state classification.
//! - **surprise**: KL divergence surprise index, cohort analysis.

pub mod distributions;
pub mod fitting;
pub mod hazard;
pub mod surprise;

use crate::types::{MemoryEntry, SurvivalMetadata};

/// Convenience function: evaluate survival statistics for a memory entry
/// at a given timestamp (days since creation).
///
/// Updates the entry's survival metadata in place: current_probability,
/// hazard_rate, and returns the updated metadata.
pub fn evaluate(entry: &MemoryEntry, days_since_creation: f64) -> SurvivalMetadata {
    let dist = &entry.survival.distribution;
    SurvivalMetadata {
        distribution: dist.clone(),
        current_probability: dist.survival_probability(days_since_creation),
        hazard_rate: dist.hazard_rate(days_since_creation),
        surprise_index: entry.survival.surprise_index,
        goodness_of_fit: entry.survival.goodness_of_fit,
    }
}
