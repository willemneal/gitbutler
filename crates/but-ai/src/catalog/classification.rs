//! Five simultaneous classification systems for catalog entries.
//!
//! Every memory is classified by subject, source, temporal position,
//! relational context, and hierarchical call number. The combination
//! makes the same memory findable through multiple paths — the way a
//! library book can be found by title, author, subject, or call number.

use crate::types::{
    AgentId, CallNumber, CatalogEntry, Classification, RetrievalWeights, ScoreBreakdown,
    ScoredEntry, SourceClassification, TaskId, TemporalClassification,
};

/// Classifies a new catalog entry using all five systems.
///
/// Given raw content and provenance, this produces a full
/// [`Classification`] with subject headings, call number, source,
/// and temporal metadata.
pub fn classify_entry(
    content: &str,
    call_number: CallNumber,
    subject_headings: Vec<String>,
    task: Option<TaskId>,
    agent: Option<AgentId>,
    branch: Option<String>,
    tool: Option<String>,
    now: &str,
    max_subjects: usize,
) -> Classification {
    // Enforce the maximum subject headings constraint.
    let subjects = if subject_headings.len() > max_subjects {
        tracing::warn!(
            count = subject_headings.len(),
            max = max_subjects,
            "truncating subject headings to maximum"
        );
        subject_headings.into_iter().take(max_subjects).collect()
    } else {
        subject_headings
    };

    // Extract additional keywords from content for enrichment.
    let mut enriched = subjects;
    let keywords = extract_keywords(content);
    for kw in keywords {
        if enriched.len() >= max_subjects {
            break;
        }
        if !enriched.iter().any(|s| s.eq_ignore_ascii_case(&kw)) {
            enriched.push(kw);
        }
    }
    enriched.truncate(max_subjects);

    Classification {
        subject_headings: enriched,
        call_number,
        source: SourceClassification {
            task,
            agent,
            branch,
            tool,
        },
        temporal: TemporalClassification {
            created: now.to_string(),
            last_accessed: now.to_string(),
            last_validated: now.to_string(),
        },
    }
}

/// Reclassify an existing entry with updated subject headings or call number.
pub fn reclassify_entry(
    entry: &mut CatalogEntry,
    new_subjects: Option<Vec<String>>,
    new_call_number: Option<CallNumber>,
    now: &str,
    max_subjects: usize,
) {
    if let Some(subjects) = new_subjects {
        let mut truncated = subjects;
        truncated.truncate(max_subjects);
        entry.classification.subject_headings = truncated;
    }
    if let Some(cn) = new_call_number {
        entry.classification.call_number = cn;
    }
    entry.classification.temporal.last_validated = now.to_string();
}

/// Score a catalog entry against a retrieval query.
///
/// The five scoring dimensions (subject match, call number proximity,
/// see-also distance, circulation frequency, freshness) are combined
/// with configurable weights.
pub fn score_entry(
    entry: &CatalogEntry,
    query_subjects: &[String],
    query_call_number: Option<&CallNumber>,
    see_also_hops: Option<u32>,
    max_circulation: u64,
    now_epoch: u64,
    entry_epoch: u64,
    max_age: u64,
    weights: &RetrievalWeights,
) -> ScoredEntry {
    // 1. Subject match: fraction of query subjects that appear in entry headings.
    let subject_score = if query_subjects.is_empty() {
        0.0
    } else {
        let matches = query_subjects
            .iter()
            .filter(|qs| {
                entry
                    .classification
                    .subject_headings
                    .iter()
                    .any(|sh| sh.eq_ignore_ascii_case(qs))
            })
            .count();
        matches as f64 / query_subjects.len() as f64
    };

    // 2. Call number proximity: shared depth / max depth.
    let cn_score = match query_call_number {
        Some(qcn) => {
            let shared = entry.classification.call_number.shared_depth(qcn);
            let max_depth = entry
                .classification
                .call_number
                .depth()
                .max(qcn.depth())
                .max(1);
            shared as f64 / max_depth as f64
        }
        None => 0.0,
    };

    // 3. See-also distance: inverse of hop count. Direct = 1.0, 2 hops = 0.5, etc.
    let see_also_score = match see_also_hops {
        Some(0) => 1.0,
        Some(hops) => 1.0 / (hops as f64 + 1.0),
        None => 0.0,
    };

    // 4. Circulation frequency: normalized against the most-circulated entry.
    let circ_score = if max_circulation == 0 {
        0.0
    } else {
        entry.circulation.total_checkouts as f64 / max_circulation as f64
    };

    // 5. Freshness: how recently the entry was validated, relative to max_age.
    let freshness_score = if max_age == 0 {
        1.0
    } else {
        let age = now_epoch.saturating_sub(entry_epoch);
        1.0 - (age as f64 / max_age as f64).min(1.0)
    };

    let composite = subject_score * weights.subject_match
        + cn_score * weights.call_number_proximity
        + see_also_score * weights.see_also_distance
        + circ_score * weights.circulation_frequency
        + freshness_score * weights.freshness;

    ScoredEntry {
        entry: entry.clone(),
        score: composite.clamp(0.0, 1.0),
        score_breakdown: ScoreBreakdown {
            subject_match: subject_score,
            call_number_proximity: cn_score,
            see_also_distance: see_also_score,
            circulation_frequency: circ_score,
            freshness: freshness_score,
        },
    }
}

/// Extract simple keywords from content for subject heading enrichment.
///
/// This is a lightweight keyword extraction — split on whitespace and
/// punctuation, filter short words, lowercase, deduplicate.
fn extract_keywords(content: &str) -> Vec<String> {
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
        "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at",
        "by", "from", "as", "into", "through", "during", "before", "after", "above", "below",
        "between", "out", "off", "over", "under", "again", "further", "then", "once", "and",
        "but", "or", "nor", "not", "so", "yet", "both", "either", "neither", "each", "every",
        "all", "any", "few", "more", "most", "other", "some", "such", "no", "only", "own",
        "same", "than", "too", "very", "just", "because", "if", "when", "while", "this", "that",
        "these", "those", "it", "its",
    ];

    let mut seen = std::collections::HashSet::new();
    let mut keywords = Vec::new();

    for word in content.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_') {
        let lower = word.to_lowercase();
        if lower.len() < 3 {
            continue;
        }
        if stop_words.contains(&lower.as_str()) {
            continue;
        }
        if seen.insert(lower.clone()) {
            keywords.push(lower);
        }
    }

    // Return only the first few most relevant keywords.
    keywords.truncate(10);
    keywords
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CirculationRecord, ItemId, SeeAlsoLink};

    fn make_entry(subjects: &[&str], call_number: &str, checkouts: u64) -> CatalogEntry {
        CatalogEntry {
            item_id: ItemId("test_001".into()),
            content: "test content".into(),
            classification: Classification {
                subject_headings: subjects.iter().map(|s| s.to_string()).collect(),
                call_number: CallNumber::parse(call_number),
                source: SourceClassification {
                    task: None,
                    agent: None,
                    branch: None,
                    tool: None,
                },
                temporal: TemporalClassification {
                    created: "2026-01-01T00:00:00Z".into(),
                    last_accessed: "2026-03-01T00:00:00Z".into(),
                    last_validated: "2026-03-01T00:00:00Z".into(),
                },
            },
            see_also: Vec::new(),
            circulation: CirculationRecord {
                total_checkouts: checkouts,
                last_checkout: None,
                checkout_contexts: Vec::new(),
            },
            ttl: "30d".into(),
            confidence: 0.9,
            deaccessioned: false,
        }
    }

    #[test]
    fn subject_match_scoring() {
        let entry = make_entry(&["authentication", "jwt"], "ARCH.AUTH", 5);
        let scored = score_entry(
            &entry,
            &["authentication".into()],
            None,
            None,
            10,
            100,
            50,
            200,
            &RetrievalWeights::default(),
        );
        assert!(scored.score_breakdown.subject_match > 0.9);
    }

    #[test]
    fn call_number_proximity_scoring() {
        let entry = make_entry(&[], "ARCH.AUTH.MIDDLEWARE", 0);
        let query_cn = CallNumber::parse("ARCH.AUTH.SESSION");
        let scored = score_entry(
            &entry,
            &[],
            Some(&query_cn),
            None,
            10,
            100,
            50,
            200,
            &RetrievalWeights::default(),
        );
        // Shared depth is 2 (ARCH.AUTH), max depth is 3 -> 2/3 ≈ 0.67
        assert!(scored.score_breakdown.call_number_proximity > 0.6);
        assert!(scored.score_breakdown.call_number_proximity < 0.7);
    }
}
