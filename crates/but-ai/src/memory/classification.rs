//! Five classification systems for memory entries.
//!
//! Every memory is classified along five dimensions:
//! 1. **Subject headings** — topical descriptors from the controlled vocabulary
//! 2. **Call number** — hierarchical position in the knowledge tree
//! 3. **Source provenance** — where the memory came from (agent, commit, task)
//! 4. **Temporal** — when the memory was created/accessed/validated
//! 5. **Relational** — see-also links to related entries
//!
//! The combination makes the same memory findable through multiple paths.

use crate::types::{
    AgentId, CallNumber, Classification, MemoryEntry, MemoryState, SurvivalDistribution,
    SurvivalMetadata,
};

/// Classify a memory entry by auto-generating subject headings from content.
///
/// Extracts keywords from the entry's content and merges them with any
/// existing subject headings, up to `max_subjects`.
pub fn auto_classify(entry: &mut MemoryEntry, max_subjects: usize) {
    let keywords = extract_keywords(&entry.content);

    // Merge new keywords into existing headings, deduplicating.
    for kw in keywords {
        if entry.classification.subject_headings.len() >= max_subjects {
            break;
        }
        if !entry
            .classification
            .subject_headings
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&kw))
        {
            entry.classification.subject_headings.push(kw);
        }
    }

    entry
        .classification
        .subject_headings
        .truncate(max_subjects);
}

/// Assign a call number to a memory entry based on a file path.
///
/// Maps directory structure to call number segments:
/// `crates/but-ai/src/memory/store.rs` -> `CRATES.BUT-AI.MEMORY.STORE`
pub fn classify_by_path(entry: &mut MemoryEntry, path: &str, max_depth: usize) {
    entry.classification.call_number =
        crate::memory::call_number::call_number_from_path(path, max_depth);
}

/// Reclassify an entry with updated subject headings and/or call number.
pub fn reclassify(
    entry: &mut MemoryEntry,
    new_subjects: Option<Vec<String>>,
    new_call_number: Option<CallNumber>,
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
}

/// Create a fully classified memory entry with all five classification dimensions.
pub fn create_classified_entry(
    id: &str,
    agent: &str,
    content: String,
    call_number: CallNumber,
    subject_headings: Vec<String>,
    now: &str,
) -> MemoryEntry {
    MemoryEntry {
        id: crate::types::EntryId(id.into()),
        agent: AgentId(agent.into()),
        content,
        created_at: now.into(),
        last_accessed: now.into(),
        classification: Classification {
            subject_headings,
            call_number,
            controlled_vocab: false,
        },
        see_also: Vec::new(),
        motifs: Vec::new(),
        tension_refs: Vec::new(),
        survival: SurvivalMetadata {
            distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
            current_probability: 1.0,
            hazard_rate: 0.01,
            surprise_index: 0.0,
            goodness_of_fit: 1.0,
        },
        state: MemoryState::Alive,
        consensus_citations: 0,
        access_count: 0,
        source_commit: None,
    }
}

/// Extract simple keywords from content for subject heading enrichment.
///
/// Splits on non-alphanumeric chars, filters stop words and short tokens,
/// lowercases, and deduplicates.
pub fn extract_keywords(content: &str) -> Vec<String> {
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

    keywords.truncate(10);
    keywords
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry() -> MemoryEntry {
        create_classified_entry(
            "test_001",
            "test-agent",
            "authentication middleware with JWT token validation".into(),
            CallNumber::parse("ARCH.AUTH"),
            vec!["authentication".into()],
            "2026-03-29T00:00:00Z",
        )
    }

    #[test]
    fn auto_classify_enriches_headings() {
        let mut entry = make_entry();
        auto_classify(&mut entry, 5);
        assert!(entry.classification.subject_headings.len() > 1);
        // Should not duplicate existing "authentication"
        let auth_count = entry
            .classification
            .subject_headings
            .iter()
            .filter(|s| s.eq_ignore_ascii_case("authentication"))
            .count();
        assert_eq!(auth_count, 1);
    }

    #[test]
    fn classify_by_path_sets_call_number() {
        let mut entry = make_entry();
        classify_by_path(&mut entry, "crates/but-ai/src/memory/store.rs", 5);
        assert_eq!(
            entry.classification.call_number.to_string_repr(),
            "CRATES.BUT-AI.MEMORY.STORE"
        );
    }

    #[test]
    fn extract_keywords_filters_stop_words() {
        let kw = extract_keywords("the authentication middleware is used for token validation");
        assert!(!kw.contains(&"the".to_string()));
        assert!(!kw.contains(&"is".to_string()));
        assert!(kw.contains(&"authentication".to_string()));
    }
}
