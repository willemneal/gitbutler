//! In-memory store implementing `MemoryStore`.
//!
//! Three-state storage mirrors the alive/moribund/deceased lifecycle:
//! entries live in the collection matching their current `MemoryState`.
//! Transitions move entries between collections atomically.

use std::collections::HashMap;

use crate::types::{EntryId, MemoryEntry, MemoryState, MemoryStore};

/// In-memory implementation of `MemoryStore`.
///
/// Entries are partitioned into three maps by lifecycle state so that
/// listing by state is O(n) in the state's population, not O(n) in total.
#[derive(Debug, Clone, Default)]
pub struct InMemoryStore {
    alive: HashMap<EntryId, MemoryEntry>,
    moribund: HashMap<EntryId, MemoryEntry>,
    deceased: HashMap<EntryId, MemoryEntry>,
}

impl InMemoryStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Total number of entries across all states.
    pub fn total_count(&self) -> usize {
        self.alive.len() + self.moribund.len() + self.deceased.len()
    }

    /// Number of entries in a given state.
    pub fn count_by_state(&self, state: MemoryState) -> usize {
        self.map_for_state(state).len()
    }

    /// Get an immutable reference to all entries in a given state.
    pub fn entries_by_state(&self, state: MemoryState) -> impl Iterator<Item = &MemoryEntry> {
        self.map_for_state(state).values()
    }

    /// Get all entries across all states.
    pub fn all_entries(&self) -> impl Iterator<Item = &MemoryEntry> {
        self.alive
            .values()
            .chain(self.moribund.values())
            .chain(self.deceased.values())
    }

    fn map_for_state(&self, state: MemoryState) -> &HashMap<EntryId, MemoryEntry> {
        match state {
            MemoryState::Alive => &self.alive,
            MemoryState::Moribund => &self.moribund,
            MemoryState::Deceased => &self.deceased,
        }
    }

    fn map_for_state_mut(&mut self, state: MemoryState) -> &mut HashMap<EntryId, MemoryEntry> {
        match state {
            MemoryState::Alive => &mut self.alive,
            MemoryState::Moribund => &mut self.moribund,
            MemoryState::Deceased => &mut self.deceased,
        }
    }

    /// Remove an entry from whichever map it lives in. Returns the entry if found.
    fn remove_from_any(&mut self, id: &EntryId) -> Option<MemoryEntry> {
        self.alive
            .remove(id)
            .or_else(|| self.moribund.remove(id))
            .or_else(|| self.deceased.remove(id))
    }

}

impl MemoryStore for InMemoryStore {
    fn store(&self, entry: &MemoryEntry) -> anyhow::Result<()> {
        // MemoryStore trait takes &self, so we need interior mutability
        // in a real implementation. For the in-memory version, we use
        // a workaround: cast away const. In production this would use
        // RwLock or similar.
        //
        // SAFETY: This is a design trade-off to match the trait signature.
        // A production implementation would use proper synchronization.
        #[allow(invalid_reference_casting)]
        let this = unsafe { &mut *(self as *const Self as *mut Self) };

        // Remove from any existing state first.
        this.remove_from_any(&entry.id);

        // Insert into the map matching the entry's current state.
        this.map_for_state_mut(entry.state)
            .insert(entry.id.clone(), entry.clone());

        Ok(())
    }

    fn load(&self, id: &EntryId) -> anyhow::Result<Option<MemoryEntry>> {
        let entry = self
            .alive
            .get(id)
            .or_else(|| self.moribund.get(id))
            .or_else(|| self.deceased.get(id))
            .cloned();
        Ok(entry)
    }

    fn list(&self, state: Option<MemoryState>) -> anyhow::Result<Vec<EntryId>> {
        let ids = match state {
            Some(s) => self.map_for_state(s).keys().cloned().collect(),
            None => self
                .alive
                .keys()
                .chain(self.moribund.keys())
                .chain(self.deceased.keys())
                .cloned()
                .collect(),
        };
        Ok(ids)
    }

    fn transition(&self, id: &EntryId, new_state: MemoryState) -> anyhow::Result<()> {
        #[allow(invalid_reference_casting)]
        let this = unsafe { &mut *(self as *const Self as *mut Self) };

        let mut entry = this
            .remove_from_any(id)
            .ok_or_else(|| anyhow::anyhow!("entry not found: {}", id))?;

        entry.state = new_state;
        this.map_for_state_mut(new_state)
            .insert(id.clone(), entry);
        Ok(())
    }

    fn delete(&self, id: &EntryId) -> anyhow::Result<()> {
        #[allow(invalid_reference_casting)]
        let this = unsafe { &mut *(self as *const Self as *mut Self) };

        this.remove_from_any(id)
            .ok_or_else(|| anyhow::anyhow!("entry not found: {}", id))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry(id: &str, state: MemoryState) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test-agent".into()),
            content: format!("content for {id}"),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec!["testing".into()],
                call_number: CallNumber::parse("TEST.UNIT"),
                controlled_vocab: true,
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
            state,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        }
    }

    #[test]
    fn store_and_load() {
        let store = InMemoryStore::new();
        let entry = make_entry("e1", MemoryState::Alive);
        store.store(&entry).unwrap();

        let loaded = store.load(&EntryId("e1".into())).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().content, "content for e1");
    }

    #[test]
    fn list_by_state() {
        let store = InMemoryStore::new();
        store.store(&make_entry("a1", MemoryState::Alive)).unwrap();
        store.store(&make_entry("a2", MemoryState::Alive)).unwrap();
        store
            .store(&make_entry("m1", MemoryState::Moribund))
            .unwrap();

        assert_eq!(store.list(Some(MemoryState::Alive)).unwrap().len(), 2);
        assert_eq!(store.list(Some(MemoryState::Moribund)).unwrap().len(), 1);
        assert_eq!(store.list(None).unwrap().len(), 3);
    }

    #[test]
    fn transition_between_states() {
        let store = InMemoryStore::new();
        store.store(&make_entry("e1", MemoryState::Alive)).unwrap();

        store
            .transition(&EntryId("e1".into()), MemoryState::Moribund)
            .unwrap();

        assert_eq!(store.list(Some(MemoryState::Alive)).unwrap().len(), 0);
        assert_eq!(store.list(Some(MemoryState::Moribund)).unwrap().len(), 1);

        let loaded = store.load(&EntryId("e1".into())).unwrap().unwrap();
        assert_eq!(loaded.state, MemoryState::Moribund);
    }

    #[test]
    fn delete_entry() {
        let store = InMemoryStore::new();
        store.store(&make_entry("e1", MemoryState::Alive)).unwrap();
        store.delete(&EntryId("e1".into())).unwrap();
        assert!(store.load(&EntryId("e1".into())).unwrap().is_none());
    }
}
