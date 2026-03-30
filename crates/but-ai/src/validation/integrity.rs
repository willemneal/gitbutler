//! Memory store integrity checking.
//!
//! Validates internal consistency of the memory store:
//! - No orphaned see_also links (targets must exist)
//! - No entries stuck in wrong lifecycle state (S(t) vs actual state)
//! - No duplicate IDs across state partitions

use crate::memory::lifecycle;
use crate::types::{EntryId, MemoryEntry, MemoryStore};

/// Classification of integrity violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// A see_also link points to a non-existent entry.
    OrphanedSeeAlso,
    /// An entry's lifecycle state does not match its survival probability.
    StateMismatch,
    /// An entry ID appears in multiple state partitions.
    DuplicateId,
    /// Survival probability is outside [0, 1].
    InvalidSurvivalProbability,
    /// Entry has no subject headings and no call number segments.
    UnclassifiedEntry,
}

/// A single integrity violation.
#[derive(Debug, Clone)]
pub struct Violation {
    /// What kind of violation.
    pub kind: ViolationKind,
    /// The entry that has the violation.
    pub entry_id: EntryId,
    /// Description of the issue.
    pub description: String,
}

/// Report from an integrity check.
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    /// All violations found.
    pub violations: Vec<Violation>,
    /// Total entries checked.
    pub entries_checked: usize,
    /// Whether the store passes integrity (no violations).
    pub passes: bool,
}

impl IntegrityReport {
    /// Count violations by kind.
    pub fn count_by_kind(&self, kind: ViolationKind) -> usize {
        self.violations.iter().filter(|v| v.kind == kind).count()
    }
}

/// The integrity checker. Validates memory store consistency.
pub struct IntegrityChecker;

impl IntegrityChecker {
    /// Run a full integrity check on the memory store.
    pub fn check<S: MemoryStore>(store: &S) -> anyhow::Result<IntegrityReport> {
        let mut violations = Vec::new();

        // Load all entries across all states.
        let all_ids = store.list(None)?;
        let mut entries: Vec<MemoryEntry> = Vec::new();
        for id in &all_ids {
            if let Some(entry) = store.load(id)? {
                entries.push(entry);
            }
        }

        // Check for duplicate IDs (should not happen in a correct store).
        check_duplicate_ids(&entries, &mut violations);

        // Check each entry.
        let entry_id_set: std::collections::HashSet<&EntryId> =
            entries.iter().map(|e| &e.id).collect();

        for entry in &entries {
            check_orphaned_see_also(entry, &entry_id_set, &mut violations);
            check_state_mismatch(entry, &mut violations);
            check_survival_probability(entry, &mut violations);
            check_classification(entry, &mut violations);
        }

        let passes = violations.is_empty();

        Ok(IntegrityReport {
            violations,
            entries_checked: entries.len(),
            passes,
        })
    }
}

/// Check for duplicate entry IDs.
fn check_duplicate_ids(entries: &[MemoryEntry], violations: &mut Vec<Violation>) {
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        if !seen.insert(&entry.id) {
            violations.push(Violation {
                kind: ViolationKind::DuplicateId,
                entry_id: entry.id.clone(),
                description: format!("Entry '{}' appears multiple times in the store.", entry.id),
            });
        }
    }
}

/// Check that all see_also targets exist in the store.
fn check_orphaned_see_also(
    entry: &MemoryEntry,
    all_ids: &std::collections::HashSet<&EntryId>,
    violations: &mut Vec<Violation>,
) {
    for link in &entry.see_also {
        if !all_ids.contains(&link.target_id) {
            violations.push(Violation {
                kind: ViolationKind::OrphanedSeeAlso,
                entry_id: entry.id.clone(),
                description: format!(
                    "Entry '{}' has see_also link to '{}' which does not exist.",
                    entry.id, link.target_id,
                ),
            });
        }
    }
}

/// Check that an entry's state matches its survival probability.
fn check_state_mismatch(entry: &MemoryEntry, violations: &mut Vec<Violation>) {
    let expected = lifecycle::state_for_probability(entry.survival.current_probability);
    if entry.state != expected {
        violations.push(Violation {
            kind: ViolationKind::StateMismatch,
            entry_id: entry.id.clone(),
            description: format!(
                "Entry '{}' has state {:?} but survival probability {:.3} \
                 suggests it should be {:?}.",
                entry.id, entry.state, entry.survival.current_probability, expected,
            ),
        });
    }
}

/// Check that survival probability is within [0, 1].
fn check_survival_probability(entry: &MemoryEntry, violations: &mut Vec<Violation>) {
    let sp = entry.survival.current_probability;
    if !(0.0..=1.0).contains(&sp) {
        violations.push(Violation {
            kind: ViolationKind::InvalidSurvivalProbability,
            entry_id: entry.id.clone(),
            description: format!(
                "Entry '{}' has survival probability {:.3} outside [0, 1].",
                entry.id, sp,
            ),
        });
    }
}

/// Check that the entry has at least some classification.
fn check_classification(entry: &MemoryEntry, violations: &mut Vec<Violation>) {
    if entry.classification.subject_headings.is_empty()
        && entry.classification.call_number.segments.is_empty()
    {
        violations.push(Violation {
            kind: ViolationKind::UnclassifiedEntry,
            entry_id: entry.id.clone(),
            description: format!(
                "Entry '{}' has no subject headings and no call number.",
                entry.id,
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_entry(id: &str, state: MemoryState, sp: f64) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: "test".into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec!["testing".into()],
                call_number: CallNumber::parse("TEST"),
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: sp,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.95,
            },
            state,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        }
    }

    #[test]
    fn clean_store_passes() {
        let store = InMemoryStore::new();
        store.store(&make_entry("e1", MemoryState::Alive, 0.9)).unwrap();
        let report = IntegrityChecker::check(&store).unwrap();
        assert!(report.passes);
    }

    #[test]
    fn detects_state_mismatch() {
        let store = InMemoryStore::new();
        // sp=0.05 should be Deceased, but state is Alive.
        store.store(&make_entry("e1", MemoryState::Alive, 0.05)).unwrap();
        let report = IntegrityChecker::check(&store).unwrap();
        assert!(!report.passes);
        assert!(report.count_by_kind(ViolationKind::StateMismatch) > 0);
    }

    #[test]
    fn detects_orphaned_see_also() {
        let store = InMemoryStore::new();
        let mut entry = make_entry("e1", MemoryState::Alive, 0.9);
        entry.see_also.push(SeeAlsoLink {
            target_id: EntryId("nonexistent".into()),
            relationship: Relationship::RelatedTo,
            note: "test".into(),
        });
        store.store(&entry).unwrap();

        let report = IntegrityChecker::check(&store).unwrap();
        assert!(!report.passes);
        assert!(report.count_by_kind(ViolationKind::OrphanedSeeAlso) > 0);
    }

    #[test]
    fn detects_invalid_survival_probability() {
        let store = InMemoryStore::new();
        let mut entry = make_entry("e1", MemoryState::Alive, 1.5);
        // Also fix the state mismatch for this test by setting sp to something valid for alive.
        entry.survival.current_probability = 1.5;
        store.store(&entry).unwrap();

        let report = IntegrityChecker::check(&store).unwrap();
        assert!(report.count_by_kind(ViolationKind::InvalidSurvivalProbability) > 0);
    }
}
