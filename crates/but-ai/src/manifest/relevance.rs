//! Consensus-weighted relevance scoring for manifest entries.
//!
//! When retrieving memory, the agent computes a relevance score using three signals:
//!
//! 1. **Semantic similarity** (40%): tag overlap between query tags and entry tags.
//!    In a full implementation this would use embedding cosine similarity.
//! 2. **Recency decay** (30%): `relevance_decay ^ hours_since_last_access`.
//! 3. **Consensus validation** (30%): how many other agents have cited this entry,
//!    normalized against the collective size.
//!
//! The final score: `0.4 * semantic + 0.3 * recency + 0.3 * consensus`.

use crate::types::{ManifestEntry, TidalConfig};

/// Weights for the three relevance scoring components.
const SEMANTIC_WEIGHT: f64 = 0.4;
const RECENCY_WEIGHT: f64 = 0.3;
const CONSENSUS_WEIGHT: f64 = 0.3;

/// A computed relevance score with its component breakdown.
#[derive(Debug, Clone)]
pub struct RelevanceScore {
    /// Semantic similarity component (0.0 to 1.0).
    pub semantic: f64,
    /// Recency decay component (0.0 to 1.0).
    pub recency: f64,
    /// Consensus validation component (0.0 to 1.0).
    pub consensus: f64,
}

impl RelevanceScore {
    /// Compute the weighted total score.
    pub fn total(&self) -> f64 {
        SEMANTIC_WEIGHT * self.semantic
            + RECENCY_WEIGHT * self.recency
            + CONSENSUS_WEIGHT * self.consensus
    }
}

/// Compute the semantic similarity between query tags and entry tags.
///
/// Uses Jaccard index: |intersection| / |union|.
/// Returns 0.0 if both tag sets are empty.
fn semantic_similarity(query_tags: &[String], entry_tags: &[String]) -> f64 {
    if query_tags.is_empty() && entry_tags.is_empty() {
        return 0.0;
    }

    let query_set: std::collections::HashSet<&str> =
        query_tags.iter().map(|s| s.as_str()).collect();
    let entry_set: std::collections::HashSet<&str> =
        entry_tags.iter().map(|s| s.as_str()).collect();

    let intersection = query_set.intersection(&entry_set).count();
    let union = query_set.union(&entry_set).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Compute the recency decay score.
///
/// `hours_since_access`: hours since the entry was last accessed.
/// `decay_factor`: the per-hour decay (typically 0.95).
///
/// Score = `decay_factor ^ hours_since_access`, clamped to [0.0, 1.0].
fn recency_score(hours_since_access: f64, decay_factor: f64) -> f64 {
    if hours_since_access <= 0.0 {
        return 1.0;
    }
    decay_factor.powf(hours_since_access).clamp(0.0, 1.0)
}

/// Compute the consensus validation score.
///
/// `citations`: number of distinct agents that have cited this entry.
/// `collective_size`: total number of agents in the collective.
///
/// Score = `citations / collective_size`, clamped to [0.0, 1.0].
fn consensus_score(citations: u64, collective_size: usize) -> f64 {
    if collective_size == 0 {
        return 0.0;
    }
    (citations as f64 / collective_size as f64).clamp(0.0, 1.0)
}

/// Score a single manifest entry against a query.
pub fn score_entry(
    entry: &ManifestEntry,
    query_tags: &[String],
    hours_since_access: f64,
    collective_size: usize,
) -> RelevanceScore {
    RelevanceScore {
        semantic: semantic_similarity(query_tags, &entry.tags),
        recency: recency_score(hours_since_access, entry.relevance_decay),
        consensus: consensus_score(entry.consensus_citations, collective_size),
    }
}

/// Retrieve the top-N relevant manifest entries from a set.
///
/// Entries scoring below `config.relevance_floor` are excluded regardless of rank.
/// Returns at most `config.max_memory_entries` entries, sorted by descending score.
pub fn retrieve_relevant(
    entries: &[ManifestEntry],
    query_tags: &[String],
    hours_since_access_fn: impl Fn(&ManifestEntry) -> f64,
    collective_size: usize,
    config: &TidalConfig,
) -> Vec<(ManifestEntry, RelevanceScore)> {
    let mut scored: Vec<(ManifestEntry, RelevanceScore)> = entries
        .iter()
        .map(|entry| {
            let hours = hours_since_access_fn(entry);
            let score = score_entry(entry, query_tags, hours, collective_size);
            (entry.clone(), score)
        })
        .filter(|(_, score)| score.total() >= config.relevance_floor)
        .collect();

    scored.sort_by(|a, b| {
        b.1.total()
            .partial_cmp(&a.1.total())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scored.truncate(config.max_memory_entries);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, EntryId, ManifestCategory, ManifestEntry};

    fn test_entry(tags: Vec<&str>, citations: u64) -> ManifestEntry {
        ManifestEntry {
            id: EntryId("test".to_string()),
            agent: AgentId("dara".to_string()),
            category: ManifestCategory::Pattern,
            created: "2026-03-28T14:00:00Z".to_string(),
            ttl: Some("720h".to_string()),
            expires: Some("2026-04-27T14:00:00Z".to_string()),
            tags: tags.into_iter().map(String::from).collect(),
            content: "test".to_string(),
            embedding_hash: None,
            relevance_decay: 0.95,
            access_count: 5,
            last_accessed: Some("2026-03-27T14:00:00Z".to_string()),
            tide_created: "high tide".to_string(),
            version: 1,
            consensus_citations: citations,
        }
    }

    #[test]
    fn semantic_similarity_perfect_match() {
        let tags = vec!["auth".to_string(), "refactor".to_string()];
        let score = semantic_similarity(&tags, &tags);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn semantic_similarity_no_overlap() {
        let a = vec!["auth".to_string()];
        let b = vec!["database".to_string()];
        let score = semantic_similarity(&a, &b);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn recency_score_just_accessed() {
        let score = recency_score(0.0, 0.95);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn recency_score_decays_over_time() {
        let recent = recency_score(1.0, 0.95);
        let old = recency_score(100.0, 0.95);
        assert!(recent > old);
    }

    #[test]
    fn consensus_score_all_agents_cited() {
        let score = consensus_score(5, 5);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn high_consensus_boosts_total_score() {
        let entry_low = test_entry(vec!["auth"], 0);
        let entry_high = test_entry(vec!["auth"], 5);
        let query = vec!["auth".to_string()];

        let score_low = score_entry(&entry_low, &query, 1.0, 5);
        let score_high = score_entry(&entry_high, &query, 1.0, 5);

        assert!(score_high.total() > score_low.total());
    }
}
