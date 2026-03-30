//! CRDT-based memory synchronization via pull-based gossip.
//!
//! Agents synchronize their memory stores using a pull-based gossip protocol.
//! Each agent maintains a vector clock tracking the latest version seen from
//! every other agent. During a gossip round, an agent sends its clock to a
//! peer; the peer responds with any entries the requesting agent is missing.
//!
//! Memory entries are merged using last-writer-wins on `access_count` as a
//! tiebreaker (higher access count wins), ensuring convergence without
//! requiring a central authority.

use crate::types::{AgentId, EntryId, MemoryEntry};
use std::collections::HashMap;

/// Vector clock tracking the latest version seen from each agent.
///
/// Each entry maps an agent ID string to the highest version number
/// observed from that agent's memory entries.
#[derive(Debug, Clone, Default)]
pub struct VectorClock {
    clocks: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that we have seen a version from the given agent.
    /// Only advances the clock -- never goes backward.
    pub fn observe(&mut self, agent: &AgentId, version: u64) {
        let current = self.clocks.entry(agent.0.clone()).or_insert(0);
        if version > *current {
            *current = version;
        }
    }

    /// Get the latest version we have seen from the given agent.
    pub fn version_for(&self, agent: &AgentId) -> u64 {
        self.clocks.get(&agent.0).copied().unwrap_or(0)
    }

    /// Compute which agents have data that `other` has not yet seen.
    ///
    /// Returns `(agent_id, min_version_needed)` pairs: the peer needs
    /// all entries from `agent_id` with version > `min_version_needed`.
    pub fn missing_for(&self, other: &VectorClock) -> Vec<(AgentId, u64)> {
        let mut missing = Vec::new();
        for (agent_str, &our_version) in &self.clocks {
            let agent = AgentId(agent_str.clone());
            let their_version = other.version_for(&agent);
            if our_version > their_version {
                missing.push((agent, their_version));
            }
        }
        missing
    }

    /// Merge another clock into this one (element-wise maximum).
    pub fn merge(&mut self, other: &VectorClock) {
        for (agent_str, &version) in &other.clocks {
            let current = self.clocks.entry(agent_str.clone()).or_insert(0);
            if version > *current {
                *current = version;
            }
        }
    }

    /// Get all known agents and their versions.
    pub fn agents(&self) -> impl Iterator<Item = (AgentId, u64)> + '_ {
        self.clocks
            .iter()
            .map(|(k, &v)| (AgentId(k.clone()), v))
    }
}

/// A gossip request sent from one agent to another.
#[derive(Debug, Clone)]
pub struct GossipRequest {
    /// The agent initiating the gossip round.
    pub from: AgentId,
    /// The sender's current vector clock.
    pub clock: VectorClock,
}

/// A gossip response containing entries the requester was missing.
#[derive(Debug, Clone)]
pub struct GossipResponse {
    /// The agent responding to the gossip request.
    pub from: AgentId,
    /// Entries that the requester was missing.
    pub entries: Vec<MemoryEntry>,
    /// The responder's current vector clock.
    pub clock: VectorClock,
}

/// The gossip engine manages pull-based memory synchronization.
///
/// Each agent runs one `GossipEngine`. When two agents exchange gossip,
/// the requesting agent sends its `VectorClock`; the responding agent
/// determines which entries the requester is missing and sends them back.
pub struct GossipEngine {
    /// This agent's ID.
    agent: AgentId,
    /// Our current vector clock.
    clock: VectorClock,
    /// Local memory entries indexed by entry ID.
    local_entries: HashMap<String, MemoryEntry>,
    /// Version counter for entries created by this agent.
    local_version: u64,
}

impl GossipEngine {
    pub fn new(agent: AgentId) -> Self {
        Self {
            agent,
            clock: VectorClock::new(),
            local_entries: HashMap::new(),
            local_version: 0,
        }
    }

    /// Ingest a locally-produced entry.
    ///
    /// Advances the local version counter and updates the vector clock.
    pub fn ingest_local(&mut self, entry: MemoryEntry) {
        self.local_version += 1;
        self.clock.observe(&self.agent, self.local_version);
        self.local_entries.insert(entry.id.0.clone(), entry);
    }

    /// Build a gossip request to send to a peer.
    pub fn build_request(&self) -> GossipRequest {
        GossipRequest {
            from: self.agent.clone(),
            clock: self.clock.clone(),
        }
    }

    /// Handle an incoming gossip request: determine which of our entries
    /// the requester is missing and return them.
    pub fn handle_request(&self, request: &GossipRequest) -> GossipResponse {
        let missing_agents = self.clock.missing_for(&request.clock);

        let entries: Vec<MemoryEntry> = self
            .local_entries
            .values()
            .filter(|entry| {
                missing_agents
                    .iter()
                    .any(|(agent, _)| entry.agent == *agent)
            })
            .cloned()
            .collect();

        GossipResponse {
            from: self.agent.clone(),
            entries,
            clock: self.clock.clone(),
        }
    }

    /// Process an incoming gossip response: merge received entries and
    /// update our vector clock.
    ///
    /// Returns the list of entries that were new or updated locally.
    pub fn process_response(&mut self, response: GossipResponse) -> Vec<EntryId> {
        let mut updated = Vec::new();

        for incoming in response.entries {
            let entry_id = incoming.id.0.clone();
            let should_insert = match self.local_entries.get(&entry_id) {
                Some(existing) => {
                    // Last-writer-wins: prefer higher access count, then later timestamp.
                    incoming.access_count > existing.access_count
                        || (incoming.access_count == existing.access_count
                            && incoming.last_accessed > existing.last_accessed)
                }
                None => true,
            };

            if should_insert {
                updated.push(incoming.id.clone());
                self.local_entries.insert(entry_id, incoming);
            }
        }

        self.clock.merge(&response.clock);
        updated
    }

    /// Get a reference to this engine's vector clock.
    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }

    /// Get a reference to a specific local entry.
    pub fn get(&self, id: &EntryId) -> Option<&MemoryEntry> {
        self.local_entries.get(&id.0)
    }

    /// Get all local entries.
    pub fn entries(&self) -> &HashMap<String, MemoryEntry> {
        &self.local_entries
    }

    /// Get this engine's agent ID.
    pub fn agent(&self) -> &AgentId {
        &self.agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn make_entry(id: &str, agent: &str) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.to_string()),
            agent: AgentId(agent.to_string()),
            content: format!("content-{id}"),
            created_at: "2026-03-29T00:00:00Z".to_string(),
            last_accessed: "2026-03-29T00:00:00Z".to_string(),
            classification: Classification {
                subject_headings: vec![],
                call_number: CallNumber::parse("TEST"),
                controlled_vocab: false,
            },
            see_also: vec![],
            motifs: vec![],
            tension_refs: vec![],
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 0.95,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.9,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 1,
            source_commit: None,
        }
    }

    #[test]
    fn vector_clock_observe_and_query() {
        let mut clock = VectorClock::new();
        let agent = AgentId("dara".to_string());

        assert_eq!(clock.version_for(&agent), 0);
        clock.observe(&agent, 3);
        assert_eq!(clock.version_for(&agent), 3);
        clock.observe(&agent, 2); // older version -- should not regress
        assert_eq!(clock.version_for(&agent), 3);
    }

    #[test]
    fn vector_clock_merge() {
        let mut a = VectorClock::new();
        let mut b = VectorClock::new();
        let dara = AgentId("dara".to_string());
        let ines = AgentId("ines".to_string());

        a.observe(&dara, 5);
        b.observe(&ines, 3);
        b.observe(&dara, 2);

        a.merge(&b);
        assert_eq!(a.version_for(&dara), 5); // kept our higher
        assert_eq!(a.version_for(&ines), 3); // got theirs
    }

    #[test]
    fn vector_clock_missing_for() {
        let mut ours = VectorClock::new();
        let mut theirs = VectorClock::new();
        let dara = AgentId("dara".to_string());
        let ines = AgentId("ines".to_string());

        ours.observe(&dara, 5);
        ours.observe(&ines, 3);
        theirs.observe(&dara, 3);

        let missing = ours.missing_for(&theirs);
        assert_eq!(missing.len(), 2); // dara (5 > 3) and ines (3 > 0)
    }

    #[test]
    fn gossip_round_trip() {
        let dara = AgentId("dara".to_string());
        let ines = AgentId("ines".to_string());

        let mut engine_dara = GossipEngine::new(dara.clone());
        let engine_ines = GossipEngine::new(ines.clone());

        // Dara creates an entry
        engine_dara.ingest_local(make_entry("e1", "dara"));

        // Ines sends a gossip request
        let request = engine_ines.build_request();

        // Dara handles the request -- returns the entry Ines is missing
        let response = engine_dara.handle_request(&request);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].id.0, "e1");
    }

    #[test]
    fn gossip_merge_prefers_higher_access_count() {
        let dara = AgentId("dara".to_string());
        let ines = AgentId("ines".to_string());

        let mut engine = GossipEngine::new(dara.clone());

        let mut entry_old = make_entry("e1", "ines");
        entry_old.access_count = 1;
        engine.ingest_local(entry_old);

        let mut entry_new = make_entry("e1", "ines");
        entry_new.access_count = 5;
        entry_new.content = "updated content".to_string();

        let response = GossipResponse {
            from: ines,
            entries: vec![entry_new],
            clock: VectorClock::new(),
        };

        let updated = engine.process_response(response);
        assert_eq!(updated.len(), 1);
        assert_eq!(
            engine.get(&EntryId("e1".to_string())).unwrap().content,
            "updated content"
        );
    }
}
