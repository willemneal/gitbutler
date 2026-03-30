//! Contradiction detection for memory entries.
//!
//! Compares a new entry's content against existing entries sharing the
//! same call number prefix or subject headings. Detected contradictions
//! are returned as `Tension` objects for tracking and resolution.

use crate::types::{MemoryEntry, Tension, TensionId, TensionSeverity};

/// Detect contradictions between a new entry and existing entries.
///
/// Scans entries sharing the same call number prefix (depth >= 2) or
/// overlapping subject headings. When content diverges significantly,
/// a `Tension` is created.
pub fn detect_contradictions(
    new_entry: &MemoryEntry,
    existing: &[&MemoryEntry],
) -> Vec<Tension> {
    let mut tensions = Vec::new();
    let mut tension_counter = 0u32;

    for existing_entry in existing {
        if existing_entry.id == new_entry.id {
            continue;
        }

        let shared_depth = new_entry
            .classification
            .call_number
            .shared_depth(&existing_entry.classification.call_number);

        let shared_subjects = new_entry
            .classification
            .subject_headings
            .iter()
            .filter(|sh| {
                existing_entry
                    .classification
                    .subject_headings
                    .iter()
                    .any(|esh| esh.eq_ignore_ascii_case(sh))
            })
            .count();

        // Only compare entries in the same knowledge area.
        if shared_depth < 2 && shared_subjects == 0 {
            continue;
        }

        // Check for content contradiction using keyword divergence.
        if let Some(contradiction) =
            check_content_contradiction(new_entry, existing_entry, shared_subjects, shared_depth)
        {
            tension_counter += 1;
            let severity = if shared_subjects >= 2 && shared_depth >= 2 {
                TensionSeverity::High
            } else if shared_subjects >= 1 || shared_depth >= 2 {
                TensionSeverity::Moderate
            } else {
                TensionSeverity::Low
            };

            tensions.push(Tension {
                id: TensionId(format!(
                    "contradiction-{}-{}",
                    new_entry.id, tension_counter
                )),
                description: contradiction,
                severity,
                introduced_in: new_entry.id.clone(),
                resolved_in: None,
            });
        }
    }

    tensions
}

/// Check two entries for content contradiction.
///
/// Returns a description of the contradiction if found, or None if
/// the entries are consistent.
fn check_content_contradiction(
    new: &MemoryEntry,
    existing: &MemoryEntry,
    shared_subjects: usize,
    shared_depth: usize,
) -> Option<String> {
    let new_keywords = extract_content_keywords(&new.content);
    let existing_keywords = extract_content_keywords(&existing.content);

    if new_keywords.is_empty() || existing_keywords.is_empty() {
        return None;
    }

    // Compute keyword overlap.
    let overlap: usize = new_keywords
        .iter()
        .filter(|k| existing_keywords.contains(k))
        .count();

    let total = new_keywords.len().max(existing_keywords.len());
    let overlap_ratio = overlap as f64 / total as f64;

    // Low overlap with shared classification = potential contradiction.
    if overlap_ratio < 0.15 && (shared_subjects >= 2 || shared_depth >= 3) {
        return Some(format!(
            "Entry '{}' contradicts '{}': shared classification \
             (subjects={}, call_number_depth={}) but only {:.0}% keyword overlap. \
             Content may be conflicting.",
            new.id,
            existing.id,
            shared_subjects,
            shared_depth,
            overlap_ratio * 100.0,
        ));
    }

    // Check for negation patterns (simple heuristic).
    let negation_words = ["not", "never", "no", "don't", "shouldn't", "avoid", "deprecated"];
    let new_lower = new.content.to_lowercase();
    let existing_lower = existing.content.to_lowercase();

    let new_has_negation = negation_words.iter().any(|w| new_lower.contains(w));
    let existing_has_negation = negation_words.iter().any(|w| existing_lower.contains(w));

    if new_has_negation != existing_has_negation && overlap_ratio > 0.3 {
        return Some(format!(
            "Entry '{}' may contradict '{}': similar keywords but one \
             contains negation patterns. Verify semantic consistency.",
            new.id, existing.id,
        ));
    }

    None
}

/// Extract content keywords for comparison. Lowercased, deduped, filtered.
fn extract_content_keywords(content: &str) -> Vec<String> {
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "have", "has", "had", "do",
        "does", "did", "will", "would", "could", "should", "to", "of", "in", "for", "on", "with",
        "at", "by", "from", "as", "and", "but", "or", "not", "this", "that", "it", "its",
    ];

    let mut seen = std::collections::HashSet::new();
    content
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter_map(|w| {
            let lower = w.to_lowercase();
            if lower.len() < 3 || stop_words.contains(&lower.as_str()) || !seen.insert(lower.clone()) {
                None
            } else {
                Some(lower)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry(id: &str, content: &str, subjects: &[&str], cn: &str) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: content.into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: subjects.iter().map(|s| s.to_string()).collect(),
                call_number: CallNumber::parse(cn),
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 0.9,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.95,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        }
    }

    #[test]
    fn detects_content_contradiction() {
        let new = make_entry(
            "new",
            "use bcrypt for password hashing with salt rounds",
            &["authentication", "security"],
            "SEC.AUTH.PASSWORDS",
        );
        let existing = make_entry(
            "old",
            "database schema migration for user table columns",
            &["authentication", "security"],
            "SEC.AUTH.PASSWORDS",
        );
        let tensions = detect_contradictions(&new, &[&existing]);
        assert!(!tensions.is_empty());
        assert!(matches!(tensions[0].severity, TensionSeverity::High));
    }

    #[test]
    fn no_contradiction_for_unrelated() {
        let new = make_entry("new", "database migrations", &["database"], "DB.MIGRATE");
        let existing = make_entry("old", "auth middleware", &["authentication"], "SEC.AUTH");
        let tensions = detect_contradictions(&new, &[&existing]);
        assert!(tensions.is_empty());
    }

    #[test]
    fn detects_negation_contradiction() {
        let new = make_entry(
            "new",
            "never use MD5 for password hashing security",
            &["security", "hashing"],
            "SEC.CRYPTO",
        );
        let existing = make_entry(
            "old",
            "use MD5 for password hashing security",
            &["security", "hashing"],
            "SEC.CRYPTO",
        );
        let tensions = detect_contradictions(&new, &[&existing]);
        assert!(!tensions.is_empty());
    }
}
