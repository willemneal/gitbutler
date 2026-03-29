//! Bounded negotiation within tide cycles.
//!
//! Negotiation is the process by which agents reach agreement on task
//! decomposition, dependency ordering, and conflict resolution. All
//! negotiation is bounded by the tide cycle -- when the ebb phase begins,
//! negotiation must conclude. If it cannot, the unresolved issue is
//! deferred to the next tide.
//!
//! The negotiation protocol has three stages:
//! 1. **Propose** (flood phase): An agent proposes a plan or resolution.
//! 2. **Counter** (high phase): Other agents may counter-propose.
//! 3. **Resolve** (ebb phase): Voting occurs and the outcome is recorded.
//!
//! If no resolution is reached by the ebb phase, the negotiation is
//! automatically deferred with both positions recorded in the manifest.

use crate::types::{AgentId, ConsensusOutcome, TideMark, TidePhase};

/// The current stage of a negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationStage {
    /// Initial proposal submitted, awaiting counter-proposals.
    Propose,
    /// Counter-proposals received, discussion ongoing.
    Counter,
    /// Voting stage, must resolve before ebb ends.
    Resolve,
    /// Negotiation concluded with an outcome.
    Concluded,
    /// Negotiation deferred to next tide cycle.
    Deferred,
}

/// A proposal in a negotiation.
#[derive(Debug, Clone)]
pub struct Proposal {
    /// The agent making the proposal.
    pub author: AgentId,
    /// The proposal text.
    pub content: String,
    /// ISO-8601 timestamp when the proposal was made.
    pub timestamp: String,
    /// The tide phase when the proposal was made.
    pub phase: TidePhase,
}

/// A counter-proposal responding to an existing proposal.
#[derive(Debug, Clone)]
pub struct CounterProposal {
    /// The agent making the counter-proposal.
    pub author: AgentId,
    /// The counter-proposal text.
    pub content: String,
    /// Which proposal this counters (index into the proposals list).
    pub counters: usize,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// A negotiation session bounded by a single tide cycle.
pub struct Negotiation {
    /// Unique ID for this negotiation.
    pub id: String,
    /// The tide cycle this negotiation must complete within.
    pub deadline_cycle: u64,
    /// Current stage.
    stage: NegotiationStage,
    /// Proposals submitted.
    proposals: Vec<Proposal>,
    /// Counter-proposals submitted.
    counter_proposals: Vec<CounterProposal>,
    /// Maximum number of counter-proposals allowed (prevents infinite negotiation).
    max_counters: usize,
    /// The final outcome, if concluded.
    outcome: Option<NegotiationOutcome>,
}

/// The outcome of a concluded negotiation.
#[derive(Debug, Clone)]
pub struct NegotiationOutcome {
    /// The winning proposal index (into the proposals list).
    pub winning_proposal: Option<usize>,
    /// The consensus result.
    pub consensus: ConsensusOutcome,
    /// Summary of the resolution for the manifest.
    pub summary: String,
    /// Agents who participated.
    pub participants: Vec<AgentId>,
}

impl Negotiation {
    /// Create a new negotiation session.
    pub fn new(id: String, deadline_cycle: u64, max_counters: usize) -> Self {
        Self {
            id,
            deadline_cycle,
            stage: NegotiationStage::Propose,
            proposals: Vec::new(),
            counter_proposals: Vec::new(),
            max_counters,
            outcome: None,
        }
    }

    /// Submit an initial proposal. Only allowed during Propose stage.
    pub fn propose(&mut self, proposal: Proposal) -> anyhow::Result<()> {
        if self.stage != NegotiationStage::Propose {
            anyhow::bail!(
                "Cannot propose in stage {:?} -- proposals only accepted during Propose stage",
                self.stage
            );
        }

        self.proposals.push(proposal);
        Ok(())
    }

    /// Submit a counter-proposal. Allowed during Propose or Counter stages.
    pub fn counter(&mut self, counter: CounterProposal) -> anyhow::Result<()> {
        match self.stage {
            NegotiationStage::Propose => {
                // First counter-proposal moves us to Counter stage
                self.stage = NegotiationStage::Counter;
            }
            NegotiationStage::Counter => {
                if self.counter_proposals.len() >= self.max_counters {
                    anyhow::bail!(
                        "Maximum counter-proposals ({}) reached -- move to resolution",
                        self.max_counters
                    );
                }
            }
            _ => {
                anyhow::bail!(
                    "Cannot counter in stage {:?}",
                    self.stage
                );
            }
        }

        if counter.counters >= self.proposals.len() {
            anyhow::bail!(
                "Counter references proposal index {} but only {} proposals exist",
                counter.counters,
                self.proposals.len()
            );
        }

        self.counter_proposals.push(counter);
        Ok(())
    }

    /// Advance to the resolution stage. Called when the tide enters ebb phase.
    pub fn advance_to_resolve(&mut self) -> anyhow::Result<()> {
        match self.stage {
            NegotiationStage::Propose | NegotiationStage::Counter => {
                self.stage = NegotiationStage::Resolve;
                Ok(())
            }
            NegotiationStage::Resolve => Ok(()), // already there
            _ => {
                anyhow::bail!("Cannot advance to resolve from stage {:?}", self.stage);
            }
        }
    }

    /// Conclude the negotiation with an outcome.
    pub fn conclude(&mut self, outcome: NegotiationOutcome) -> anyhow::Result<()> {
        if self.stage != NegotiationStage::Resolve {
            anyhow::bail!(
                "Cannot conclude in stage {:?} -- must be in Resolve stage",
                self.stage
            );
        }
        self.outcome = Some(outcome);
        self.stage = NegotiationStage::Concluded;
        Ok(())
    }

    /// Defer the negotiation to the next tide cycle.
    /// Records both positions as an unresolved disagreement.
    pub fn defer(&mut self, summary: String) {
        self.outcome = Some(NegotiationOutcome {
            winning_proposal: None,
            consensus: ConsensusOutcome::Deferred,
            summary,
            participants: self.all_participants(),
        });
        self.stage = NegotiationStage::Deferred;
    }

    /// Check the negotiation against the current tide and auto-advance stages.
    ///
    /// - If the tide is in ebb and we are still in Propose/Counter, advance to Resolve.
    /// - If the tide has moved past our deadline cycle, defer the negotiation.
    pub fn check_tide(&mut self, current_tide: &TideMark) {
        if current_tide.cycle > self.deadline_cycle {
            if !matches!(
                self.stage,
                NegotiationStage::Concluded | NegotiationStage::Deferred
            ) {
                self.defer(format!(
                    "Negotiation {} deferred: tide cycle {} exceeded deadline {}",
                    self.id, current_tide.cycle, self.deadline_cycle
                ));
            }
            return;
        }

        if current_tide.phase == TidePhase::Ebb
            && matches!(
                self.stage,
                NegotiationStage::Propose | NegotiationStage::Counter
            )
        {
            let _ = self.advance_to_resolve();
        }
    }

    /// Get the current stage.
    pub fn stage(&self) -> NegotiationStage {
        self.stage
    }

    /// Get the outcome, if the negotiation has concluded or been deferred.
    pub fn outcome(&self) -> Option<&NegotiationOutcome> {
        self.outcome.as_ref()
    }

    /// Get all proposals.
    pub fn proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    /// Get all counter-proposals.
    pub fn counter_proposals(&self) -> &[CounterProposal] {
        &self.counter_proposals
    }

    /// Collect all unique participants.
    fn all_participants(&self) -> Vec<AgentId> {
        let mut participants: Vec<AgentId> = Vec::new();
        for p in &self.proposals {
            if !participants.contains(&p.author) {
                participants.push(p.author.clone());
            }
        }
        for c in &self.counter_proposals {
            if !participants.contains(&c.author) {
                participants.push(c.author.clone());
            }
        }
        participants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_proposal(agent: &str, content: &str) -> Proposal {
        Proposal {
            author: AgentId(agent.to_string()),
            content: content.to_string(),
            timestamp: "2026-03-28T14:00:00Z".to_string(),
            phase: TidePhase::Flood,
        }
    }

    fn make_counter(agent: &str, content: &str, counters: usize) -> CounterProposal {
        CounterProposal {
            author: AgentId(agent.to_string()),
            content: content.to_string(),
            counters,
            timestamp: "2026-03-28T15:00:00Z".to_string(),
        }
    }

    #[test]
    fn proposal_and_counter_flow() {
        let mut neg = Negotiation::new("n1".to_string(), 100, 3);

        neg.propose(make_proposal("dara", "Use provider trait")).unwrap();
        assert_eq!(neg.stage(), NegotiationStage::Propose);

        neg.counter(make_counter("ines", "Use message passing instead", 0))
            .unwrap();
        assert_eq!(neg.stage(), NegotiationStage::Counter);
    }

    #[test]
    fn counter_limit_enforced() {
        let mut neg = Negotiation::new("n1".to_string(), 100, 2);
        neg.propose(make_proposal("dara", "Plan A")).unwrap();

        neg.counter(make_counter("ines", "Counter 1", 0)).unwrap();
        neg.counter(make_counter("koel", "Counter 2", 0)).unwrap();
        assert!(neg.counter(make_counter("sable", "Counter 3", 0)).is_err());
    }

    #[test]
    fn tide_ebb_auto_advances_to_resolve() {
        let mut neg = Negotiation::new("n1".to_string(), 100, 5);
        neg.propose(make_proposal("dara", "Test")).unwrap();

        let ebb_tide = TideMark {
            timestamp: "2026-03-28T15:30:00Z".to_string(),
            phase: TidePhase::Ebb,
            cycle: 100,
            phase_elapsed_seconds: 0,
        };
        neg.check_tide(&ebb_tide);
        assert_eq!(neg.stage(), NegotiationStage::Resolve);
    }

    #[test]
    fn expired_cycle_defers() {
        let mut neg = Negotiation::new("n1".to_string(), 100, 5);
        neg.propose(make_proposal("dara", "Test")).unwrap();

        let late_tide = TideMark {
            timestamp: "2026-03-29T00:00:00Z".to_string(),
            phase: TidePhase::Flood,
            cycle: 101,
            phase_elapsed_seconds: 0,
        };
        neg.check_tide(&late_tide);
        assert_eq!(neg.stage(), NegotiationStage::Deferred);
        assert!(neg.outcome().is_some());
    }
}
