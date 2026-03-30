//! Three-state lifecycle transitions for memory entries.
//!
//! Memory entries progress through three states based on their survival
//! probability S(t):
//!
//! - **Alive**: S(t) >= 0.25 — actively relevant
//! - **Moribund**: 0.10 <= S(t) < 0.25 — under review, may recover
//! - **Deceased**: S(t) < 0.10 — expired but archived
//!
//! The moribund state prevents premature expiration. An entry can be
//! resuscitated if its survival probability recovers (e.g. after being
//! accessed again).

use crate::types::{EntryId, MemoryState, MemoryStore};

/// Threshold below which an alive entry transitions to moribund.
pub const MORIBUND_THRESHOLD: f64 = 0.25;

/// Threshold below which a moribund entry transitions to deceased.
pub const DECEASED_THRESHOLD: f64 = 0.10;

/// Result of a lifecycle audit on a single entry.
#[derive(Debug, Clone)]
pub struct AuditResult {
    /// The entry that was audited.
    pub entry_id: EntryId,
    /// The state before the audit.
    pub previous_state: MemoryState,
    /// The state after the audit.
    pub new_state: MemoryState,
    /// The survival probability at the time of audit.
    pub survival_probability: f64,
    /// Whether a transition occurred.
    pub transitioned: bool,
}

/// Audit all entries in the store and transition those whose survival
/// probability has fallen below threshold.
///
/// Returns a list of audit results for entries that changed state.
pub fn audit_lifecycle<S: MemoryStore>(store: &S) -> anyhow::Result<Vec<AuditResult>> {
    let mut results = Vec::new();

    // Check alive entries for demotion to moribund.
    let alive_ids = store.list(Some(MemoryState::Alive))?;
    for id in &alive_ids {
        if let Some(entry) = store.load(id)? {
            let sp = entry.survival.current_probability;
            if sp < DECEASED_THRESHOLD {
                // Skip moribund, go straight to deceased.
                store.transition(id, MemoryState::Deceased)?;
                results.push(AuditResult {
                    entry_id: id.clone(),
                    previous_state: MemoryState::Alive,
                    new_state: MemoryState::Deceased,
                    survival_probability: sp,
                    transitioned: true,
                });
            } else if sp < MORIBUND_THRESHOLD {
                store.transition(id, MemoryState::Moribund)?;
                results.push(AuditResult {
                    entry_id: id.clone(),
                    previous_state: MemoryState::Alive,
                    new_state: MemoryState::Moribund,
                    survival_probability: sp,
                    transitioned: true,
                });
            }
        }
    }

    // Check moribund entries for demotion to deceased or promotion back to alive.
    let moribund_ids = store.list(Some(MemoryState::Moribund))?;
    for id in &moribund_ids {
        if let Some(entry) = store.load(id)? {
            let sp = entry.survival.current_probability;
            if sp < DECEASED_THRESHOLD {
                store.transition(id, MemoryState::Deceased)?;
                results.push(AuditResult {
                    entry_id: id.clone(),
                    previous_state: MemoryState::Moribund,
                    new_state: MemoryState::Deceased,
                    survival_probability: sp,
                    transitioned: true,
                });
            } else if sp >= MORIBUND_THRESHOLD {
                // Resuscitation: survival probability recovered.
                store.transition(id, MemoryState::Alive)?;
                results.push(AuditResult {
                    entry_id: id.clone(),
                    previous_state: MemoryState::Moribund,
                    new_state: MemoryState::Alive,
                    survival_probability: sp,
                    transitioned: true,
                });
            }
        }
    }

    Ok(results)
}

/// Resuscitate a deceased entry back to alive state.
///
/// This is used when an entry is explicitly accessed again after being
/// marked deceased, indicating it is still relevant.
pub fn resuscitate<S: MemoryStore>(store: &S, id: &EntryId) -> anyhow::Result<bool> {
    match store.load(id)? {
        Some(entry) if entry.state == MemoryState::Deceased => {
            store.transition(id, MemoryState::Alive)?;
            Ok(true)
        }
        Some(_) => Ok(false), // Not deceased, nothing to do.
        None => anyhow::bail!("entry not found: {}", id),
    }
}

/// Determine the appropriate state for a given survival probability.
pub fn state_for_probability(sp: f64) -> MemoryState {
    if sp >= MORIBUND_THRESHOLD {
        MemoryState::Alive
    } else if sp >= DECEASED_THRESHOLD {
        MemoryState::Moribund
    } else {
        MemoryState::Deceased
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_entry(id: &str, sp: f64, state: MemoryState) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: "test".into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec![],
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
    fn audit_demotes_low_survival() {
        let store = InMemoryStore::new();
        // sp=0.20 < 0.25: should become moribund
        store
            .store(&make_entry("e1", 0.20, MemoryState::Alive))
            .unwrap();
        // sp=0.05 < 0.10: should become deceased
        store
            .store(&make_entry("e2", 0.05, MemoryState::Alive))
            .unwrap();
        // sp=0.50: should stay alive
        store
            .store(&make_entry("e3", 0.50, MemoryState::Alive))
            .unwrap();

        let results = audit_lifecycle(&store).unwrap();
        assert_eq!(results.len(), 2);

        let e1_result = results.iter().find(|r| r.entry_id.0 == "e1").unwrap();
        assert_eq!(e1_result.new_state, MemoryState::Moribund);

        let e2_result = results.iter().find(|r| r.entry_id.0 == "e2").unwrap();
        assert_eq!(e2_result.new_state, MemoryState::Deceased);
    }

    #[test]
    fn audit_resuscitates_recovered_moribund() {
        let store = InMemoryStore::new();
        // sp=0.30 >= 0.25: should be promoted back to alive
        store
            .store(&make_entry("e1", 0.30, MemoryState::Moribund))
            .unwrap();

        let results = audit_lifecycle(&store).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].new_state, MemoryState::Alive);
    }

    #[test]
    fn resuscitate_deceased_entry() {
        let store = InMemoryStore::new();
        store
            .store(&make_entry("e1", 0.05, MemoryState::Deceased))
            .unwrap();

        assert!(resuscitate(&store, &EntryId("e1".into())).unwrap());

        let entry = store.load(&EntryId("e1".into())).unwrap().unwrap();
        assert_eq!(entry.state, MemoryState::Alive);
    }

    #[test]
    fn state_for_probability_thresholds() {
        assert_eq!(state_for_probability(0.50), MemoryState::Alive);
        assert_eq!(state_for_probability(0.25), MemoryState::Alive);
        assert_eq!(state_for_probability(0.20), MemoryState::Moribund);
        assert_eq!(state_for_probability(0.10), MemoryState::Moribund);
        assert_eq!(state_for_probability(0.09), MemoryState::Deceased);
    }
}
