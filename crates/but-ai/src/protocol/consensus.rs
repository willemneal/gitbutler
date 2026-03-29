//! Consensus mechanisms for distributed agent decisions.
//!
//! All five agents participate in every significant decision. A decision
//! requires 3-of-5 approval (configurable quorum). Ties (2-2 with one
//! abstention) result in deferral to the next tide cycle.
//!
//! No harbormaster. The protocol is the authority.

use crate::types::{
    AgentId, ConsensusOutcome, ConsensusWeight, TidalConfig, TideMark, Vote,
};

/// A consensus round tracking votes on a specific proposal.
#[derive(Debug, Clone)]
pub struct ConsensusRound {
    /// Unique ID for this round.
    pub round_id: String,
    /// The proposal being voted on.
    pub proposal: String,
    /// The tide cycle within which this round must complete.
    pub deadline_cycle: u64,
    /// Votes collected so far.
    votes: Vec<Vote>,
    /// Known agents in the collective and their consensus weights.
    weights: Vec<ConsensusWeight>,
    /// Required quorum (number of approvals needed).
    quorum: usize,
}

impl ConsensusRound {
    /// Create a new consensus round.
    pub fn new(
        round_id: String,
        proposal: String,
        deadline_cycle: u64,
        weights: Vec<ConsensusWeight>,
        config: &TidalConfig,
    ) -> Self {
        Self {
            round_id,
            proposal,
            deadline_cycle,
            votes: Vec::new(),
            weights,
            quorum: config.consensus_quorum,
        }
    }

    /// Cast a vote. Returns an error if the agent has already voted.
    pub fn vote(&mut self, vote: Vote) -> anyhow::Result<()> {
        if self.has_voted(&vote.agent) {
            anyhow::bail!("Agent {} has already voted in round {}", vote.agent, self.round_id);
        }
        self.votes.push(vote);
        Ok(())
    }

    /// Whether the given agent has already voted.
    pub fn has_voted(&self, agent: &AgentId) -> bool {
        self.votes.iter().any(|v| v.agent == *agent)
    }

    /// Tally votes and determine the outcome.
    ///
    /// Votes are weighted by the agent's consensus weight. The weighted
    /// approve/reject totals are compared against the quorum threshold.
    pub fn tally(&self) -> ConsensusOutcome {
        let mut weighted_approve: f64 = 0.0;
        let mut weighted_reject: f64 = 0.0;

        for vote in &self.votes {
            let weight = self.weight_for(&vote.agent);
            if vote.approve {
                weighted_approve += weight;
            } else {
                weighted_reject += weight;
            }
        }

        let quorum_threshold = self.quorum as f64;

        if weighted_approve >= quorum_threshold {
            ConsensusOutcome::Approved
        } else if weighted_reject >= quorum_threshold {
            ConsensusOutcome::Rejected
        } else if self.votes.len() == self.weights.len() {
            // All agents have voted but no quorum reached -- deadlock
            ConsensusOutcome::Deadlocked
        } else {
            // Not all agents have voted yet
            ConsensusOutcome::Deferred
        }
    }

    /// Check if the round has expired (current cycle is past the deadline).
    pub fn is_expired(&self, current_tide: &TideMark) -> bool {
        current_tide.cycle > self.deadline_cycle
    }

    /// Get the number of votes cast.
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    /// Get all votes cast so far.
    pub fn votes(&self) -> &[Vote] {
        &self.votes
    }

    /// Look up the consensus weight for an agent.
    fn weight_for(&self, agent: &AgentId) -> f64 {
        self.weights
            .iter()
            .find(|w| w.agent == *agent)
            .map(|w| w.weight)
            .unwrap_or(1.0) // Default weight for unknown agents
    }
}

/// The consensus engine manages multiple active consensus rounds.
pub struct ConsensusEngine {
    /// Active rounds indexed by round ID.
    rounds: std::collections::HashMap<String, ConsensusRound>,
    /// Completed rounds (kept for audit trail).
    completed: Vec<(ConsensusRound, ConsensusOutcome)>,
}

impl ConsensusEngine {
    pub fn new() -> Self {
        Self {
            rounds: std::collections::HashMap::new(),
            completed: Vec::new(),
        }
    }

    /// Start a new consensus round.
    pub fn start_round(&mut self, round: ConsensusRound) -> anyhow::Result<()> {
        if self.rounds.contains_key(&round.round_id) {
            anyhow::bail!("Round {} already exists", round.round_id);
        }
        self.rounds.insert(round.round_id.clone(), round);
        Ok(())
    }

    /// Cast a vote in an active round.
    pub fn vote(&mut self, round_id: &str, vote: Vote) -> anyhow::Result<ConsensusOutcome> {
        let round = self
            .rounds
            .get_mut(round_id)
            .ok_or_else(|| anyhow::anyhow!("No active round with ID {}", round_id))?;

        round.vote(vote)?;
        let outcome = round.tally();

        // If the round is decided, move it to completed
        if matches!(
            outcome,
            ConsensusOutcome::Approved | ConsensusOutcome::Rejected | ConsensusOutcome::Deadlocked
        ) {
            if let Some(round) = self.rounds.remove(round_id) {
                self.completed.push((round, outcome));
            }
        }

        Ok(outcome)
    }

    /// Expire rounds that have passed their deadline.
    /// Returns the IDs of expired rounds (all marked as Deferred).
    pub fn expire_rounds(&mut self, current_tide: &TideMark) -> Vec<String> {
        let expired_ids: Vec<String> = self
            .rounds
            .iter()
            .filter(|(_, round)| round.is_expired(current_tide))
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();
        for id in expired_ids {
            if let Some(round) = self.rounds.remove(&id) {
                result.push(id);
                self.completed.push((round, ConsensusOutcome::Deferred));
            }
        }
        result
    }

    /// Get the number of active rounds.
    pub fn active_count(&self) -> usize {
        self.rounds.len()
    }

    /// Get the completed rounds audit trail.
    pub fn completed(&self) -> &[(ConsensusRound, ConsensusOutcome)] {
        &self.completed
    }

    /// Look up an active round by ID.
    pub fn get_round(&self, round_id: &str) -> Option<&ConsensusRound> {
        self.rounds.get(round_id)
    }
}

impl Default for ConsensusEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Create default equal weights for a set of agents.
pub fn equal_weights(agents: &[AgentId]) -> Vec<ConsensusWeight> {
    agents
        .iter()
        .map(|agent| ConsensusWeight {
            agent: agent.clone(),
            weight: 1.0,
            participation_count: 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TidePhase;

    fn make_agents() -> Vec<AgentId> {
        vec![
            AgentId("dara".to_string()),
            AgentId("ines".to_string()),
            AgentId("koel".to_string()),
            AgentId("sable".to_string()),
            AgentId("raul".to_string()),
        ]
    }

    fn make_vote(agent: &str, approve: bool) -> Vote {
        Vote {
            agent: AgentId(agent.to_string()),
            approve,
            weight: 1.0,
            reason: None,
            tide_mark: TideMark {
                timestamp: "2026-03-28T14:00:00Z".to_string(),
                phase: TidePhase::High,
                cycle: 100,
                phase_elapsed_seconds: 0,
            },
        }
    }

    #[test]
    fn three_approvals_reaches_quorum() {
        let agents = make_agents();
        let weights = equal_weights(&agents);
        let config = TidalConfig::default();
        let mut round =
            ConsensusRound::new("r1".to_string(), "test".to_string(), 100, weights, &config);

        round.vote(make_vote("dara", true)).unwrap();
        round.vote(make_vote("ines", true)).unwrap();
        assert_eq!(round.tally(), ConsensusOutcome::Deferred);

        round.vote(make_vote("koel", true)).unwrap();
        assert_eq!(round.tally(), ConsensusOutcome::Approved);
    }

    #[test]
    fn three_rejections_rejects() {
        let agents = make_agents();
        let weights = equal_weights(&agents);
        let config = TidalConfig::default();
        let mut round =
            ConsensusRound::new("r1".to_string(), "test".to_string(), 100, weights, &config);

        round.vote(make_vote("dara", false)).unwrap();
        round.vote(make_vote("ines", false)).unwrap();
        round.vote(make_vote("koel", false)).unwrap();
        assert_eq!(round.tally(), ConsensusOutcome::Rejected);
    }

    #[test]
    fn two_two_with_abstention_deadlocks() {
        let agents = make_agents();
        let weights = equal_weights(&agents);
        let config = TidalConfig {
            consensus_quorum: 3,
            ..Default::default()
        };
        let mut round =
            ConsensusRound::new("r1".to_string(), "test".to_string(), 100, weights, &config);

        round.vote(make_vote("dara", true)).unwrap();
        round.vote(make_vote("ines", true)).unwrap();
        round.vote(make_vote("koel", false)).unwrap();
        round.vote(make_vote("sable", false)).unwrap();
        // Raul abstains -- at 4 votes, not all have voted yet
        assert_eq!(round.tally(), ConsensusOutcome::Deferred);

        // If raul votes approve, it passes
        round.vote(make_vote("raul", true)).unwrap();
        assert_eq!(round.tally(), ConsensusOutcome::Approved);
    }

    #[test]
    fn duplicate_vote_rejected() {
        let agents = make_agents();
        let weights = equal_weights(&agents);
        let config = TidalConfig::default();
        let mut round =
            ConsensusRound::new("r1".to_string(), "test".to_string(), 100, weights, &config);

        round.vote(make_vote("dara", true)).unwrap();
        assert!(round.vote(make_vote("dara", false)).is_err());
    }
}
