//! Continuity checking for memory entries.
//!
//! Given a new `MemoryEntry` and a set of existing entries, the continuity
//! checker flags contradictions: entries that share subject headings or
//! call number prefixes but contain conflicting content.

use crate::types::{EntryId, MemoryEntry};

/// Severity of a continuity observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationSeverity {
    /// Informational: worth noting but not blocking.
    Info,
    /// Warning: inconsistency that should be reviewed.
    Warning,
    /// Contradiction: conflicting information that must be resolved.
    Contradiction,
}

/// A single observation from a continuity check.
#[derive(Debug, Clone)]
pub struct Observation {
    /// Severity of the observation.
    pub severity: ObservationSeverity,
    /// Description of what was found.
    pub description: String,
    /// The existing entry that conflicts with the new entry.
    pub conflicting_entry: Option<EntryId>,
}

/// Report from a continuity check.
#[derive(Debug, Clone)]
pub struct ContinuityReport {
    /// The entry that was checked.
    pub checked_entry: EntryId,
    /// All observations found.
    pub observations: Vec<Observation>,
    /// Whether the entry passes continuity (no contradictions).
    pub passes: bool,
}

impl ContinuityReport {
    /// Count observations by severity.
    pub fn count_by_severity(&self, severity: ObservationSeverity) -> usize {
        self.observations
            .iter()
            .filter(|o| o.severity == severity)
            .count()
    }
}

/// The continuity checker. Reviews new entries for consistency with
/// existing memory.
pub struct ContinuityChecker;

impl ContinuityChecker {
    /// Check a new entry for continuity against existing entries.
    ///
    /// Checks for:
    /// 1. Subject heading overlap with potentially conflicting content
    /// 2. Call number prefix overlap with differing content
    /// 3. Duplicate content detection
    pub fn check(
        new_entry: &MemoryEntry,
        existing: &[&MemoryEntry],
    ) -> ContinuityReport {
        let mut observations = Vec::new();

        for existing_entry in existing {
            if existing_entry.id == new_entry.id {
                continue;
            }

            // Check for subject heading overlap.
            let shared_subjects = shared_subject_count(new_entry, existing_entry);

            if shared_subjects > 0 {
                // Entries share subjects -- check for content conflict.
                let similarity = content_similarity(&new_entry.content, &existing_entry.content);

                if similarity < 0.1 && shared_subjects >= 2 {
                    // Very different content with multiple shared subjects = contradiction.
                    observations.push(Observation {
                        severity: ObservationSeverity::Contradiction,
                        description: format!(
                            "Entry '{}' shares {} subject headings with '{}' but content \
                             is highly divergent (similarity: {:.2}). Possible contradiction.",
                            new_entry.id, shared_subjects, existing_entry.id, similarity,
                        ),
                        conflicting_entry: Some(existing_entry.id.clone()),
                    });
                } else if similarity > 0.8 {
                    // Very similar content = potential duplicate.
                    observations.push(Observation {
                        severity: ObservationSeverity::Warning,
                        description: format!(
                            "Entry '{}' has highly similar content to '{}' \
                             (similarity: {:.2}). Possible duplicate.",
                            new_entry.id, existing_entry.id, similarity,
                        ),
                        conflicting_entry: Some(existing_entry.id.clone()),
                    });
                }
            }

            // Check for call number prefix overlap.
            let shared_depth = new_entry
                .classification
                .call_number
                .shared_depth(&existing_entry.classification.call_number);

            if shared_depth >= 2 && shared_subjects == 0 {
                // Same knowledge area but no shared subjects -- informational.
                observations.push(Observation {
                    severity: ObservationSeverity::Info,
                    description: format!(
                        "Entry '{}' shares call number prefix (depth {}) with '{}' \
                         but no common subject headings. Related but distinct.",
                        new_entry.id, shared_depth, existing_entry.id,
                    ),
                    conflicting_entry: Some(existing_entry.id.clone()),
                });
            }
        }

        let passes = !observations
            .iter()
            .any(|o| o.severity == ObservationSeverity::Contradiction);

        ContinuityReport {
            checked_entry: new_entry.id.clone(),
            observations,
            passes,
        }
    }
}

/// Count subject headings shared between two entries (case-insensitive).
fn shared_subject_count(a: &MemoryEntry, b: &MemoryEntry) -> usize {
    a.classification
        .subject_headings
        .iter()
        .filter(|sh| {
            b.classification
                .subject_headings
                .iter()
                .any(|bsh| bsh.eq_ignore_ascii_case(sh))
        })
        .count()
}

/// Compute a simple Jaccard similarity between two content strings.
///
/// Splits on whitespace, lowercases, and computes |intersection| / |union|.
fn content_similarity(a: &str, b: &str) -> f64 {
    let a_words: std::collections::HashSet<String> =
        a.split_whitespace().map(|w| w.to_lowercase()).collect();
    let b_words: std::collections::HashSet<String> =
        b.split_whitespace().map(|w| w.to_lowercase()).collect();

    if a_words.is_empty() && b_words.is_empty() {
        return 1.0;
    }

    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();

    if union == 0 {
        return 0.0;
    }

    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry(id: &str, content: &str, subjects: &[&str], call_number: &str) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: content.into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: subjects.iter().map(|s| s.to_string()).collect(),
                call_number: CallNumber::parse(call_number),
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
    fn detects_contradiction() {
        let new = make_entry(
            "new",
            "use bcrypt for password hashing",
            &["authentication", "security"],
            "SEC.AUTH",
        );
        let existing = make_entry(
            "old",
            "completely unrelated database migration content",
            &["authentication", "security"],
            "SEC.AUTH",
        );
        let report = ContinuityChecker::check(&new, &[&existing]);
        assert!(!report.passes);
        assert!(report.count_by_severity(ObservationSeverity::Contradiction) > 0);
    }

    #[test]
    fn detects_duplicate() {
        let new = make_entry(
            "new",
            "use bcrypt for password hashing in auth module",
            &["authentication"],
            "SEC.AUTH",
        );
        let existing = make_entry(
            "old",
            "use bcrypt for password hashing in auth module",
            &["authentication"],
            "SEC.AUTH",
        );
        let report = ContinuityChecker::check(&new, &[&existing]);
        assert!(report.count_by_severity(ObservationSeverity::Warning) > 0);
    }

    #[test]
    fn passes_when_no_conflicts() {
        let new = make_entry("new", "database migrations", &["database"], "DB.MIGRATE");
        let existing = make_entry("old", "auth middleware", &["authentication"], "SEC.AUTH");
        let report = ContinuityChecker::check(&new, &[&existing]);
        assert!(report.passes);
    }
}
