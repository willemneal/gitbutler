//! Manifest module -- the distributed memory of the collective.
//!
//! The manifest is inspired by shipping manifests: documents that travel with cargo,
//! listing contents, origin, destination, and handling instructions. Every port
//! that touches the cargo reads the manifest, stamps it, and passes it along.
//!
//! Agent memory works the same way. Each memory entry is a manifest -- a JSON
//! document stored in a Git blob, referenced by a Git ref, synchronized across
//! agents using a CRDT-based gossip protocol.
//!
//! # Submodules
//!
//! - [`entry`] -- CRDT operations on manifest entries (create, merge, resolve).
//! - [`gossip`] -- Pull-based gossip protocol for manifest synchronization.
//! - [`relevance`] -- Consensus-weighted relevance scoring for retrieval.
//! - [`tide`] -- Tidal cycle clock (6-hour phases: flood, high, ebb, low).

pub mod entry;
pub mod gossip;
pub mod relevance;
pub mod tide;

pub use entry::{cite, content_hash, create, merge};
pub use gossip::{GossipEngine, GossipRequest, GossipResponse, VectorClock};
pub use relevance::{retrieve_relevant, score_entry, RelevanceScore};
pub use tide::{tide_label, TidalClock};

use crate::types::{AgentId, EntryId, ManifestEntry, TidalConfig};
use std::collections::HashMap;

/// The manifest store -- an agent's local collection of memory entries.
///
/// Each agent maintains its own manifest store. Entries are synchronized
/// with other agents via the gossip protocol. No agent can modify another's
/// entries directly; changes propagate through CRDT merge.
pub struct ManifestStore {
    /// The owning agent.
    agent: AgentId,
    /// All entries, indexed by entry ID.
    entries: HashMap<String, ManifestEntry>,
    /// The gossip engine for synchronization.
    gossip: GossipEngine,
    /// Configuration.
    config: TidalConfig,
}

impl ManifestStore {
    /// Create a new empty manifest store for the given agent.
    pub fn new(agent: AgentId, config: TidalConfig) -> Self {
        let gossip = GossipEngine::new(agent.clone());
        Self {
            agent,
            entries: HashMap::new(),
            gossip,
            config,
        }
    }

    /// Store a new manifest entry. Computes the content hash as the entry ID.
    pub fn store(&mut self, entry: ManifestEntry) -> EntryId {
        let id = entry.id.clone();
        self.gossip.ingest_local(entry.clone());
        self.entries.insert(id.0.clone(), entry);
        id
    }

    /// Retrieve an entry by ID.
    pub fn get(&self, id: &EntryId) -> Option<&ManifestEntry> {
        self.entries.get(&id.0)
    }

    /// Retrieve an entry by ID, mutably (for recording access).
    pub fn get_mut(&mut self, id: &EntryId) -> Option<&mut ManifestEntry> {
        self.entries.get_mut(&id.0)
    }

    /// Query entries by tags, returning the most relevant ones.
    pub fn query(
        &self,
        query_tags: &[String],
        hours_since_access_fn: impl Fn(&ManifestEntry) -> f64,
        collective_size: usize,
    ) -> Vec<(ManifestEntry, RelevanceScore)> {
        let all_entries: Vec<ManifestEntry> = self.entries.values().cloned().collect();
        retrieve_relevant(
            &all_entries,
            query_tags,
            hours_since_access_fn,
            collective_size,
            &self.config,
        )
    }

    /// Remove expired entries. Returns the IDs of removed entries.
    pub fn expire(&mut self, now: &str) -> Vec<EntryId> {
        let expired_ids: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(now))
            .map(|(id, _)| id.clone())
            .collect();

        let mut removed = Vec::new();
        for id in expired_ids {
            if let Some(entry) = self.entries.remove(&id) {
                removed.push(entry.id);
            }
        }
        removed
    }

    /// Get the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the owning agent's ID.
    pub fn agent(&self) -> &AgentId {
        &self.agent
    }

    /// Get a reference to the gossip engine for synchronization.
    pub fn gossip(&self) -> &GossipEngine {
        &self.gossip
    }

    /// Get a mutable reference to the gossip engine.
    pub fn gossip_mut(&mut self) -> &mut GossipEngine {
        &mut self.gossip
    }

    /// Merge incoming entries from a gossip response into the local store.
    pub fn merge_gossip_response(&mut self, response: GossipResponse) -> Vec<ManifestEntry> {
        let merged = self.gossip.process_response(response);
        for entry in &merged {
            self.entries.insert(entry.id.0.clone(), entry.clone());
        }
        merged
    }

    /// Serialize all entries to JSON (for writing to Git refs).
    pub fn to_json(&self) -> anyhow::Result<String> {
        let entries: Vec<&ManifestEntry> = self.entries.values().collect();
        Ok(serde_json::to_string_pretty(&entries)?)
    }

    /// Deserialize entries from JSON and merge them into the store.
    pub fn merge_from_json(&mut self, json: &str) -> anyhow::Result<usize> {
        let entries: Vec<ManifestEntry> = serde_json::from_str(json)?;
        let mut merged_count = 0;
        for incoming in entries {
            let id = incoming.id.0.clone();
            if let Some(existing) = self.entries.get(&id) {
                if let Some(result) = merge(existing, &incoming) {
                    self.entries.insert(id, result);
                    merged_count += 1;
                }
            } else {
                self.entries.insert(id, incoming);
                merged_count += 1;
            }
        }
        Ok(merged_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ManifestCategory;

    #[test]
    fn store_and_retrieve() {
        let agent = AgentId("dara".to_string());
        let config = TidalConfig::default();
        let mut store = ManifestStore::new(agent.clone(), config);

        let entry = create(
            content_hash("test content"),
            agent,
            ManifestCategory::Pattern,
            "test content".to_string(),
            vec!["auth".to_string()],
            "2026-03-28T14:00:00Z".to_string(),
            "high tide".to_string(),
        );
        let id = store.store(entry);

        assert_eq!(store.len(), 1);
        let retrieved = store.get(&id).unwrap();
        assert_eq!(retrieved.content, "test content");
    }

    #[test]
    fn expiration_removes_old_entries() {
        let agent = AgentId("dara".to_string());
        let config = TidalConfig::default();
        let mut store = ManifestStore::new(agent.clone(), config);

        let mut entry = create(
            EntryId("will-expire".to_string()),
            agent,
            ManifestCategory::Error,
            "old error".to_string(),
            vec![],
            "2026-01-01T00:00:00Z".to_string(),
            "low tide".to_string(),
        );
        entry.expires = Some("2026-01-03T00:00:00Z".to_string());
        store.store(entry);

        assert_eq!(store.len(), 1);
        let removed = store.expire("2026-03-28T00:00:00Z");
        assert_eq!(removed.len(), 1);
        assert_eq!(store.len(), 0);
    }
}
