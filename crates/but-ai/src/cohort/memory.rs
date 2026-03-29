//! Cohort-based memory storage and lifecycle management.
//!
//! The memory store is the actuarial table: every entry has a survival
//! function, a lifecycle state, and a complete access history. Entries
//! transition from alive to moribund to deceased as their survival
//! probability declines.
//!
//! Memory retrieval is survival-weighted: high-S(t) memories are
//! prioritized, and compaction tiers determine how much of each
//! memory is retained in the context window.

use crate::survival::{fitting, hazard, surprise};
use crate::types::{
    AccessRecord, ActuarialConfig, CompactionTier, LifeTable, LifecycleState, MemoryEntry,
    MemoryId, MemoryType, RelevanceScore, RelevanceWeights, StudyId,
};

/// The actuarial memory store.
///
/// Manages the full lifecycle of memory entries, from creation through
/// fitting, monitoring, moribund review, and archival.
#[derive(Debug, Clone)]
pub struct ActuarialMemoryStore {
    /// All memory entries, keyed by ID.
    entries: Vec<MemoryEntry>,
    /// Configuration for thresholds and defaults.
    config: ActuarialConfig,
    /// Relevance scoring weights.
    weights: RelevanceWeights,
    /// Tasks since last re-fit.
    tasks_since_refit: u32,
}

impl ActuarialMemoryStore {
    /// Create a new empty memory store.
    pub fn new(config: ActuarialConfig) -> Self {
        Self {
            entries: Vec::new(),
            config,
            weights: RelevanceWeights::default(),
            tasks_since_refit: 0,
        }
    }

    /// Create a store with custom relevance weights.
    pub fn with_weights(config: ActuarialConfig, weights: RelevanceWeights) -> Self {
        Self {
            entries: Vec::new(),
            config,
            weights,
            tasks_since_refit: 0,
        }
    }

    /// Add a new memory entry with a default survival distribution.
    pub fn admit(&mut self, id: MemoryId, memory_type: MemoryType, content: String, created_at: String) {
        let default_dist = fitting::fit_distribution(&[], memory_type, &fitting::FittingConfig::default());

        let entry = MemoryEntry {
            id,
            memory_type,
            content,
            created_at: created_at.clone(),
            last_accessed: created_at,
            access_history: Vec::new(),
            survival_distribution: crate::types::SurvivalDistribution {
                fitted_at: default_dist.fitted_at,
                ..default_dist
            },
            current_survival_probability: 1.0,
            current_hazard_rate: 0.0,
            surprise_index: 0.0,
            embedding_vector: None,
            source_commit: None,
            practitioner_summary: String::new(),
            lifecycle_state: LifecycleState::Alive,
        };

        self.entries.push(entry);

        // Enforce max alive entries.
        self.enforce_capacity();
    }

    /// Record an access to a memory entry.
    ///
    /// Updates the access history, recalculates survival statistics,
    /// and potentially triggers resuscitation for moribund entries.
    pub fn record_access(
        &mut self,
        memory_id: &MemoryId,
        study_id: StudyId,
        timestamp: String,
        days_since_creation: f64,
    ) -> anyhow::Result<()> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == *memory_id)
            .ok_or_else(|| anyhow::anyhow!("Memory entry {} not found", memory_id.0))?;

        // Record the access.
        entry.access_history.push(AccessRecord {
            timestamp: timestamp.clone(),
            study: study_id,
        });
        entry.last_accessed = timestamp;

        // If moribund, this access constitutes resuscitation.
        if entry.lifecycle_state == LifecycleState::Moribund {
            tracing::info!(
                memory_id = %entry.id.0,
                "Resuscitating moribund memory via observed access"
            );
        }

        // Update survival statistics.
        hazard::update_survival_statistics(
            entry,
            days_since_creation,
            self.config.alive_threshold,
            self.config.deceased_threshold,
        )?;

        // Recompute surprise index.
        entry.surprise_index = surprise::compute_surprise_index(
            &entry.access_history,
            &entry.created_at,
            &entry.survival_distribution,
        );

        Ok(())
    }

    /// Run the periodic survival audit.
    ///
    /// Updates all entries' survival statistics, transitions lifecycle
    /// states, and triggers re-fitting if the refit interval has elapsed.
    pub fn audit(&mut self, current_days_since_epoch: f64) -> AuditReport {
        self.tasks_since_refit += 1;
        let should_refit = self.tasks_since_refit >= self.config.refit_interval;

        let mut transitions = Vec::new();
        let mut surprise_flags = Vec::new();

        for entry in &mut self.entries {
            let old_state = entry.lifecycle_state;

            // Approximate days since creation from the epoch offset.
            // In a real system this would use actual datetime parsing.
            let days_since_creation = current_days_since_epoch;

            // Update survival statistics.
            if let Err(e) = hazard::update_survival_statistics(
                entry,
                days_since_creation,
                self.config.alive_threshold,
                self.config.deceased_threshold,
            ) {
                tracing::warn!(memory_id = %entry.id.0, error = %e, "Failed to update survival statistics");
                continue;
            }

            // Record transitions.
            if entry.lifecycle_state != old_state {
                transitions.push(LifecycleTransition {
                    memory_id: entry.id.clone(),
                    from: old_state,
                    to: entry.lifecycle_state,
                    survival_probability: entry.current_survival_probability,
                });
            }

            // Check for surprise.
            if surprise::should_trigger_cohort_review(
                entry.surprise_index,
                self.config.surprise_threshold,
            ) {
                surprise_flags.push(entry.id.clone());
            }

            // Re-fit distribution if interval elapsed and sufficient data.
            if should_refit && entry.access_history.len() >= 3 {
                let intervals = fitting::compute_intervals(
                    &entry.access_history,
                    &entry.created_at,
                );
                let new_dist = fitting::fit_distribution(
                    &intervals,
                    entry.memory_type,
                    &fitting::FittingConfig::default(),
                );
                entry.survival_distribution = new_dist;
            }
        }

        if should_refit {
            self.tasks_since_refit = 0;
        }

        AuditReport {
            entries_audited: self.entries.len(),
            transitions,
            surprise_flags,
            refit_performed: should_refit,
        }
    }

    /// Retrieve memories relevant to a query, ranked by relevance score.
    ///
    /// Only alive and moribund memories are considered (deceased are archived).
    /// Results are sorted by composite relevance score, descending.
    pub fn retrieve(
        &self,
        query_embedding: Option<&[f64]>,
        max_results: usize,
    ) -> Vec<RetrievalResult> {
        let mut results: Vec<RetrievalResult> = self
            .entries
            .iter()
            .filter(|e| e.lifecycle_state != LifecycleState::Deceased)
            .map(|entry| {
                let score = compute_relevance(entry, query_embedding, &self.weights);
                let tier = hazard::compaction_tier(entry.current_survival_probability);
                RetrievalResult {
                    entry: entry.clone(),
                    score,
                    compaction_tier: tier,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .composite
                .partial_cmp(&a.score.composite)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(max_results);
        results
    }

    /// Produce aggregate life table statistics.
    pub fn life_table(&self, cohort_label: String) -> LifeTable {
        let alive_entries: Vec<&MemoryEntry> = self
            .entries
            .iter()
            .filter(|e| e.lifecycle_state == LifecycleState::Alive)
            .collect();

        let moribund_count = self
            .entries
            .iter()
            .filter(|e| e.lifecycle_state == LifecycleState::Moribund)
            .count() as u64;

        let deceased_count = self
            .entries
            .iter()
            .filter(|e| e.lifecycle_state == LifecycleState::Deceased)
            .count() as u64;

        let alive_count = alive_entries.len() as u64;

        let aggregate_hazard_rate = if alive_entries.is_empty() {
            0.0
        } else {
            alive_entries.iter().map(|e| e.current_hazard_rate).sum::<f64>()
                / alive_entries.len() as f64
        };

        let mean_survival_probability = if alive_entries.is_empty() {
            0.0
        } else {
            alive_entries
                .iter()
                .map(|e| e.current_survival_probability)
                .sum::<f64>()
                / alive_entries.len() as f64
        };

        LifeTable {
            total_entries: self.entries.len() as u64,
            alive_count,
            moribund_count,
            deceased_count,
            aggregate_hazard_rate,
            mean_survival_probability,
            cohort_label,
        }
    }

    /// Get all entries in a specific lifecycle state.
    pub fn entries_by_state(&self, state: LifecycleState) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.lifecycle_state == state)
            .collect()
    }

    /// Get a specific entry by ID.
    pub fn get(&self, id: &MemoryId) -> Option<&MemoryEntry> {
        self.entries.iter().find(|e| e.id == *id)
    }

    /// Get a mutable reference to a specific entry.
    pub fn get_mut(&mut self, id: &MemoryId) -> Option<&mut MemoryEntry> {
        self.entries.iter_mut().find(|e| e.id == *id)
    }

    /// Total number of entries across all states.
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    /// Compact the store by fully removing deceased entries beyond a retention limit.
    ///
    /// Deceased entries are kept as training data, but we cap the archive
    /// to prevent unbounded growth.
    pub fn compact_deceased(&mut self, max_deceased: usize) {
        let deceased: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.lifecycle_state == LifecycleState::Deceased)
            .map(|(i, _)| i)
            .collect();

        if deceased.len() > max_deceased {
            // Remove the oldest deceased entries (lowest survival probability first).
            let mut to_remove: Vec<usize> = deceased;
            // Sort by survival probability ascending (least useful first).
            to_remove.sort_by(|&a, &b| {
                self.entries[a]
                    .current_survival_probability
                    .partial_cmp(&self.entries[b].current_survival_probability)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let remove_count = to_remove.len() - max_deceased;
            let mut indices_to_remove: Vec<usize> = to_remove[..remove_count].to_vec();
            indices_to_remove.sort_unstable_by(|a, b| b.cmp(a)); // Remove from end first.

            for idx in indices_to_remove {
                self.entries.remove(idx);
            }
        }
    }

    /// Enforce maximum alive entries by transitioning excess to moribund.
    fn enforce_capacity(&mut self) {
        let alive_count = self
            .entries
            .iter()
            .filter(|e| e.lifecycle_state == LifecycleState::Alive)
            .count();

        if alive_count <= self.config.max_alive_entries {
            return;
        }

        // Find alive entries with the lowest survival probability.
        let mut alive_indices: Vec<(usize, f64)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.lifecycle_state == LifecycleState::Alive)
            .map(|(i, e)| (i, e.current_survival_probability))
            .collect();

        alive_indices.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let excess = alive_count - self.config.max_alive_entries;
        for &(idx, _) in alive_indices.iter().take(excess) {
            self.entries[idx].lifecycle_state = LifecycleState::Moribund;
        }
    }
}

/// Result of a memory retrieval operation.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub entry: MemoryEntry,
    pub score: RelevanceScore,
    pub compaction_tier: CompactionTier,
}

/// A lifecycle transition observed during audit.
#[derive(Debug, Clone)]
pub struct LifecycleTransition {
    pub memory_id: MemoryId,
    pub from: LifecycleState,
    pub to: LifecycleState,
    pub survival_probability: f64,
}

/// Report from a periodic audit.
#[derive(Debug, Clone)]
pub struct AuditReport {
    pub entries_audited: usize,
    pub transitions: Vec<LifecycleTransition>,
    pub surprise_flags: Vec<MemoryId>,
    pub refit_performed: bool,
}

/// Compute the relevance score for a memory entry.
///
/// Formula from the proposal:
///   score = 0.30 * embedding_similarity
///         + 0.25 * survival_probability
///         + 0.20 * hazard_adjusted_recency
///         + 0.15 * access_frequency
///         + 0.10 * goodness_of_fit
fn compute_relevance(
    entry: &MemoryEntry,
    query_embedding: Option<&[f64]>,
    weights: &RelevanceWeights,
) -> RelevanceScore {
    // Embedding similarity.
    let embedding_similarity = match (query_embedding, &entry.embedding_vector) {
        (Some(query), Some(entry_vec)) => cosine_similarity(query, entry_vec),
        _ => 0.5, // Neutral score when embeddings are unavailable.
    };

    // Survival probability (already in [0, 1]).
    let survival_probability = entry.current_survival_probability;

    // Hazard-adjusted recency.
    // Use a default recency decay of 0.05 per day.
    let hazard_adjusted = hazard::hazard_adjusted_recency(
        0.0, // Approximate: we don't have actual days since last access here.
        entry.current_hazard_rate,
        0.05,
    );

    // Access frequency (normalized).
    let access_frequency = (entry.access_history.len() as f64 / 10.0).min(1.0);

    // Goodness of fit.
    let goodness_of_fit = entry.survival_distribution.goodness_of_fit;

    // Weighted composite.
    let composite = weights.embedding_similarity * embedding_similarity
        + weights.survival_probability * survival_probability
        + weights.hazard_adjusted_recency * hazard_adjusted
        + weights.access_frequency * access_frequency
        + weights.goodness_of_fit * goodness_of_fit;

    RelevanceScore {
        composite,
        embedding_similarity,
        survival_probability,
        hazard_adjusted_recency: hazard_adjusted,
        access_frequency,
        goodness_of_fit,
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a < 1e-15 || norm_b < 1e-15 {
        return 0.0;
    }

    // Map from [-1, 1] to [0, 1].
    (dot / (norm_a * norm_b) + 1.0) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ActuarialConfig;

    #[test]
    fn admit_and_retrieve() {
        let config = ActuarialConfig::default();
        let mut store = ActuarialMemoryStore::new(config);

        store.admit(
            MemoryId("arch-001".into()),
            MemoryType::Architectural,
            "Auth uses middleware".into(),
            "2026-03-15T10:00:00Z".into(),
        );

        let results = store.retrieve(None, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entry.id.0, "arch-001");
    }

    #[test]
    fn cosine_similarity_of_identical_vectors_is_one() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cosine_similarity_of_orthogonal_vectors_is_half() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.5).abs() < 1e-10);
    }

    #[test]
    fn life_table_counts_correctly() {
        let config = ActuarialConfig::default();
        let mut store = ActuarialMemoryStore::new(config);

        store.admit(MemoryId("a".into()), MemoryType::Architectural, "a".into(), "2026-01-01T00:00:00Z".into());
        store.admit(MemoryId("b".into()), MemoryType::BugFix, "b".into(), "2026-01-01T00:00:00Z".into());

        let table = store.life_table("2026-Q1".into());
        assert_eq!(table.alive_count, 2);
        assert_eq!(table.total_entries, 2);
    }
}
