//! Unified 6-component scoring engine implementing `MemoryRetriever`.
//!
//! The scoring formula combines six dimensions with configurable weights:
//!
//! ```text
//! score = classification_match * w.call_number_proximity  (0.25 default)
//!       + motif_resonance      * w.motif_resonance        (0.20 default)
//!       + survival_probability * w.survival_probability    (0.20 default)
//!       + see_also_distance    * w.see_also_distance       (0.15 default)
//!       + tension_urgency      * w.tension_boost           (0.10 default)
//!       + freshness            * w.freshness               (0.10 default)
//! ```

use crate::memory::call_number::call_number_proximity;
use crate::memory::classification::extract_keywords;
use crate::memory::see_also::SeeAlsoGraph;
use crate::types::{
    CallNumber, EntryId, MemoryEntry, MemoryRetriever, MemoryStore, RelevanceWeights,
    ScoreBreakdown, ScoredMemory,
};

/// A retrieval engine that scores and ranks memory entries.
///
/// Holds references to the auxiliary data structures needed for
/// see-also scoring and call-number proximity.
pub struct RetrievalEngine<S: MemoryStore> {
    store: S,
    see_also: SeeAlsoGraph,
}

impl<S: MemoryStore> RetrievalEngine<S> {
    /// Create a new retrieval engine wrapping a store and see-also graph.
    pub fn new(store: S, see_also: SeeAlsoGraph) -> Self {
        Self { store, see_also }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a reference to the see-also graph.
    pub fn see_also(&self) -> &SeeAlsoGraph {
        &self.see_also
    }

    /// Get a mutable reference to the see-also graph.
    pub fn see_also_mut(&mut self) -> &mut SeeAlsoGraph {
        &mut self.see_also
    }
}

impl<S: MemoryStore> MemoryRetriever for RetrievalEngine<S> {
    fn retrieve(
        &self,
        query: &str,
        max_results: usize,
        weights: &RelevanceWeights,
    ) -> anyhow::Result<Vec<ScoredMemory>> {
        let all_ids = self.store.list(None)?;
        let query_keywords = extract_keywords(query);
        let query_call_number = infer_call_number(query);

        let mut scored: Vec<ScoredMemory> = Vec::new();

        for id in &all_ids {
            let entry = match self.store.load(id)? {
                Some(e) => e,
                None => continue,
            };

            let breakdown = score_entry(
                &entry,
                &query_keywords,
                query_call_number.as_ref(),
                &self.see_also,
                &all_ids,
            );

            let composite = breakdown.motif_resonance * weights.motif_resonance
                + breakdown.call_number_proximity * weights.call_number_proximity
                + breakdown.survival_probability * weights.survival_probability
                + breakdown.see_also_distance * weights.see_also_distance
                + breakdown.tension_boost * weights.tension_boost
                + breakdown.freshness * weights.freshness;

            scored.push(ScoredMemory {
                entry,
                score: composite.clamp(0.0, 1.0),
                breakdown,
            });
        }

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(max_results);

        Ok(scored)
    }
}

/// Score a single entry against query parameters across all 6 dimensions.
pub fn score_entry(
    entry: &MemoryEntry,
    query_keywords: &[String],
    query_call_number: Option<&CallNumber>,
    see_also: &SeeAlsoGraph,
    all_entry_ids: &[EntryId],
) -> ScoreBreakdown {
    // 1. Classification / call number proximity
    let cn_score = match query_call_number {
        Some(qcn) => call_number_proximity(&entry.classification.call_number, qcn),
        None => 0.0,
    };

    // 2. Motif resonance: keyword overlap between query and entry motifs + content
    let motif_score = motif_resonance(entry, query_keywords);

    // 3. Survival probability: direct from the entry's survival metadata
    let survival_score = entry.survival.current_probability;

    // 4. See-also distance: best score from any directly-matching entry to this one
    let see_also_score = see_also_score(entry, query_keywords, see_also, all_entry_ids);

    // 5. Tension urgency: boost for entries with high-severity unresolved tensions
    let tension_score = tension_urgency(entry);

    // 6. Freshness: use access count as a proxy (higher = more fresh/relevant)
    let freshness_score = freshness_score(entry);

    ScoreBreakdown {
        motif_resonance: motif_score,
        call_number_proximity: cn_score,
        see_also_distance: see_also_score,
        survival_probability: survival_score,
        freshness: freshness_score,
        tension_boost: tension_score,
    }
}

/// Compute motif resonance as keyword overlap between query and entry.
///
/// Checks both the entry's motif IDs and content-derived keywords.
fn motif_resonance(entry: &MemoryEntry, query_keywords: &[String]) -> f64 {
    if query_keywords.is_empty() {
        return 0.0;
    }

    let entry_keywords = extract_keywords(&entry.content);
    let motif_strings: Vec<String> = entry.motifs.iter().map(|m| m.0.to_lowercase()).collect();

    let mut matches = 0usize;
    for qk in query_keywords {
        let qk_lower = qk.to_lowercase();
        if entry_keywords.iter().any(|ek| *ek == qk_lower)
            || motif_strings.iter().any(|m| m.contains(&qk_lower))
            || entry
                .classification
                .subject_headings
                .iter()
                .any(|sh| sh.eq_ignore_ascii_case(&qk_lower))
        {
            matches += 1;
        }
    }

    matches as f64 / query_keywords.len() as f64
}

/// Compute see-also score: if any entry matching the query keywords is
/// linked via the see-also graph, use the distance score.
fn see_also_score(
    entry: &MemoryEntry,
    _query_keywords: &[String],
    see_also: &SeeAlsoGraph,
    all_entry_ids: &[EntryId],
) -> f64 {
    // Check the best see-also distance from any entry in the graph to this entry.
    let mut best = 0.0f64;
    for other_id in all_entry_ids {
        if *other_id == entry.id {
            continue;
        }
        let score = see_also.distance_score(other_id, &entry.id);
        if score > best {
            best = score;
        }
    }
    best
}

/// Compute tension urgency boost based on unresolved tension severity.
fn tension_urgency(entry: &MemoryEntry) -> f64 {
    if entry.tension_refs.is_empty() {
        return 0.0;
    }

    // Count by role: only "introduced" or "referenced" tensions are unresolved.
    let unresolved_count = entry
        .tension_refs
        .iter()
        .filter(|tr| {
            matches!(
                tr.role,
                crate::types::TensionRole::Introduced | crate::types::TensionRole::Referenced
            )
        })
        .count();

    if unresolved_count == 0 {
        return 0.0;
    }

    // Normalize: assume max 5 unresolved tensions yields full score.
    (unresolved_count as f64 / 5.0).min(1.0)
}

/// Compute a freshness score based on access count.
///
/// More accesses suggest the entry is still actively relevant.
/// Uses a logarithmic scale: 0 accesses = 0.0, 10+ = ~1.0.
fn freshness_score(entry: &MemoryEntry) -> f64 {
    if entry.access_count == 0 {
        return 0.1; // Base freshness for new entries.
    }
    (1.0 + entry.access_count as f64).ln() / (1.0 + 10.0_f64).ln()
}

/// Try to infer a call number from a query string.
///
/// If the query contains a path-like segment, convert it to a call number.
fn infer_call_number(query: &str) -> Option<CallNumber> {
    // Look for path-like patterns (containing /).
    for word in query.split_whitespace() {
        if word.contains('/') {
            return Some(crate::memory::call_number::call_number_from_path(word, 5));
        }
    }
    // Look for dot-separated uppercase segments (e.g. "ARCH.AUTH").
    for word in query.split_whitespace() {
        if word.contains('.') && word.chars().any(|c| c.is_uppercase()) {
            return Some(CallNumber::parse(word));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_entry(id: &str, content: &str, motifs: &[&str]) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: content.into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec!["testing".into()],
                call_number: CallNumber::parse("TEST.UNIT"),
                controlled_vocab: true,
            },
            see_also: Vec::new(),
            motifs: motifs.iter().map(|m| MotifId(m.to_string())).collect(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 0.8,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.95,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 5,
            source_commit: None,
        }
    }

    #[test]
    fn retrieval_returns_scored_results() {
        let store = InMemoryStore::new();
        let e1 = make_entry("e1", "authentication middleware JWT", &["auth"]);
        let e2 = make_entry("e2", "database migration scripts", &["db"]);
        store.store(&e1).unwrap();
        store.store(&e2).unwrap();

        let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
        let results = engine
            .retrieve("authentication", 10, &RelevanceWeights::default())
            .unwrap();

        assert_eq!(results.len(), 2);
        // e1 should score higher (content matches "authentication").
        assert!(results[0].entry.id == EntryId("e1".into()));
    }

    #[test]
    fn motif_resonance_scoring() {
        let entry = make_entry("e1", "auth middleware", &["authentication"]);
        let score = motif_resonance(&entry, &["authentication".into()]);
        assert!(score > 0.0);
    }

    #[test]
    fn tension_urgency_scoring() {
        let mut entry = make_entry("e1", "test", &[]);
        entry.tension_refs.push(TensionRef {
            tension_id: TensionId("t1".into()),
            role: TensionRole::Introduced,
        });
        let score = tension_urgency(&entry);
        assert!(score > 0.0);
    }
}
