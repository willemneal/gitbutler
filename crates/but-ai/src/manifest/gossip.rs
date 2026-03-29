//! Gossip protocol for manifest synchronization.
//!
//! Agents synchronize their manifests using a pull-based gossip protocol.
//! Each agent maintains a vector clock tracking the latest version it has
//! seen from every other agent. During a gossip round, an agent sends its
//! clock to a peer; the peer responds with any entries the requesting agent
//! is missing. The CRDT merge in `entry.rs` ensures convergence.
//!
//! The gossip protocol runs during the `Low` tide phase, when agents are
//! in maintenance mode. This prevents gossip from consuming execution budget.

use crate::types::{AgentId, ManifestEntry, TidePhase};
use std::collections::HashMap;

/// Vector clock tracking the latest version seen from each agent.
///
/// Each entry maps an agent ID to the highest `version` number seen
/// in that agent's manifest entries.
#[derive(Debug, Clone, Default)]
pub struct VectorClock {
    clocks: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that we have seen a version from the given agent.
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

    /// Compute the entries that `other` is missing relative to our clock.
    ///
    /// Returns a list of `(agent_id, min_version_needed)` pairs indicating
    /// which agents have newer data that `other` has not yet seen.
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
}

/// A gossip request sent from one agent to another.
///
/// Contains the sender's vector clock so the receiver can determine
/// which entries the sender is missing.
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
    pub entries: Vec<ManifestEntry>,
    /// The responder's updated vector clock.
    pub clock: VectorClock,
}

/// The gossip engine manages synchronization between agents.
pub struct GossipEngine {
    /// This agent's ID.
    agent: AgentId,
    /// Our current vector clock.
    clock: VectorClock,
    /// Local manifest entries indexed by entry ID.
    local_entries: HashMap<String, ManifestEntry>,
}

impl GossipEngine {
    pub fn new(agent: AgentId) -> Self {
        Self {
            agent,
            clock: VectorClock::new(),
            local_entries: HashMap::new(),
        }
    }

    /// Ingest a local entry (produced by this agent).
    pub fn ingest_local(&mut self, entry: ManifestEntry) {
        self.clock.observe(&entry.agent, entry.version);
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

        let mut entries = Vec::new();
        for entry in self.local_entries.values() {
            for (missing_agent, min_version) in &missing_agents {
                if entry.agent == *missing_agent && entry.version > *min_version {
                    entries.push(entry.clone());
                }
            }
        }

        GossipResponse {
            from: self.agent.clone(),
            entries,
            clock: self.clock.clone(),
        }
    }

    /// Process an incoming gossip response: merge received entries
    /// using CRDT merge and update our vector clock.
    pub fn process_response(&mut self, response: GossipResponse) -> Vec<ManifestEntry> {
        let mut merged = Vec::new();

        for incoming in response.entries {
            let entry_id = incoming.id.0.clone();
            if let Some(existing) = self.local_entries.get(&entry_id) {
                if let Some(result) = super::entry::merge(existing, &incoming) {
                    self.local_entries.insert(entry_id, result.clone());
                    merged.push(result);
                }
            } else {
                self.local_entries.insert(entry_id, incoming.clone());
                merged.push(incoming);
            }
        }

        // Merge the responder's clock into ours
        self.clock.merge(&response.clock);

        merged
    }

    /// Whether gossip should run right now, based on the current tide phase.
    /// Gossip is preferred during `Low` tide (maintenance phase) but allowed anytime.
    pub fn should_gossip(phase: TidePhase) -> bool {
        // Preferred during low tide, but allowed during any phase
        // with decreasing priority
        match phase {
            TidePhase::Low => true,
            TidePhase::Flood => true,
            TidePhase::Ebb => true,
            TidePhase::High => false, // Don't gossip during peak execution
        }
    }

    /// Get a reference to this engine's vector clock.
    pub fn clock(&self) -> &VectorClock {
        &self.clock
    }

    /// Get a reference to local entries.
    pub fn entries(&self) -> &HashMap<String, ManifestEntry> {
        &self.local_entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_clock_observe_and_query() {
        let mut clock = VectorClock::new();
        let agent = AgentId("dara".to_string());

        assert_eq!(clock.version_for(&agent), 0);
        clock.observe(&agent, 3);
        assert_eq!(clock.version_for(&agent), 3);
        clock.observe(&agent, 2); // older version, should not regress
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
        assert_eq!(a.version_for(&dara), 5); // kept our higher version
        assert_eq!(a.version_for(&ines), 3); // got their version
    }

    #[test]
    fn gossip_engine_round_trip() {
        let dara = AgentId("dara".to_string());
        let ines = AgentId("ines".to_string());

        let mut engine_dara = GossipEngine::new(dara.clone());
        let engine_ines = GossipEngine::new(ines.clone());

        // Dara creates an entry
        let entry = super::super::entry::create(
            crate::types::EntryId("e1".to_string()),
            dara.clone(),
            crate::types::ManifestCategory::Pattern,
            "test pattern".to_string(),
            vec!["test".to_string()],
            "2026-03-28T14:00:00Z".to_string(),
            "high tide".to_string(),
        );
        engine_dara.ingest_local(entry);

        // Ines sends a gossip request
        let request = engine_ines.build_request();

        // Dara handles the request -- should return the entry Ines is missing
        let response = engine_dara.handle_request(&request);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].content, "test pattern");
    }
}
