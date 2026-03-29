//! Study protocol execution.
//!
//! A study follows a six-phase lifecycle, with the protocol mode
//! adapting based on the remaining token budget. As budget depletes,
//! the protocol downgrades from Full to Abbreviated to Minimum
//! Publishable Unit to Emergency Halt.

use crate::types::{
    ActuarialConfig, AgentId, AgentRole, ProtocolMode, StudyId, StudyPhase, StudyProgress,
};

/// A study protocol governing how a task is executed.
///
/// The protocol tracks budget consumption, estimates completion probability,
/// and switches modes when the budget survival function drops below thresholds.
#[derive(Debug, Clone)]
pub struct StudyProtocol {
    pub study_id: StudyId,
    pub current_phase: StudyPhase,
    pub protocol_mode: ProtocolMode,
    pub tokens_used: u64,
    pub tokens_budget: u64,
    /// Estimated task complexity (1.0 = average, >1 = harder).
    pub estimated_complexity: f64,
    /// Accumulated confidence in the output so far.
    pub confidence: f64,
}

impl StudyProtocol {
    /// Create a new study protocol with the given budget.
    pub fn new(study_id: StudyId, config: &ActuarialConfig) -> Self {
        Self {
            study_id,
            current_phase: StudyPhase::LiteratureReview,
            protocol_mode: ProtocolMode::Full,
            tokens_used: 0,
            tokens_budget: config.token_budget,
            estimated_complexity: 1.0,
            confidence: 0.0,
        }
    }

    /// Record tokens consumed and update protocol mode.
    pub fn consume_tokens(&mut self, tokens: u64, config: &ActuarialConfig) {
        self.tokens_used += tokens;
        self.protocol_mode = determine_protocol_mode(
            self.tokens_used,
            self.tokens_budget,
            self.estimated_complexity,
            config,
        );
    }

    /// Estimate the probability of completing the study within budget.
    ///
    /// Uses a survival-model-inspired approach: the "budget survival function"
    /// models the probability that the remaining budget is sufficient.
    ///
    /// P(completion) = S_budget(consumed / allocated, complexity)
    pub fn estimate_completion_probability(&self) -> f64 {
        budget_survival(self.tokens_used, self.tokens_budget, self.estimated_complexity)
    }

    /// Advance to the next phase in the study lifecycle.
    ///
    /// The transition respects the current protocol mode:
    /// - Full: all six phases.
    /// - Abbreviated: skip peer review iteration.
    /// - MinimumPublishableUnit: jump to experiment then publication.
    /// - EmergencyHalt: jump directly to publication.
    pub fn advance_phase(&mut self) -> Option<StudyPhase> {
        let next = match self.protocol_mode {
            ProtocolMode::Full => next_phase_full(self.current_phase),
            ProtocolMode::Abbreviated => next_phase_abbreviated(self.current_phase),
            ProtocolMode::MinimumPublishableUnit => next_phase_mpu(self.current_phase),
            ProtocolMode::EmergencyHalt => Some(StudyPhase::Publication),
        };

        if let Some(phase) = next {
            self.current_phase = phase;
        }
        next
    }

    /// Generate a progress report for the current state.
    pub fn progress_report(
        &self,
        agent: AgentId,
        role: AgentRole,
        surprise_index: f64,
        memory_hazard_rate: f64,
    ) -> StudyProgress {
        StudyProgress {
            phase: self.current_phase,
            agent,
            role,
            study_id: self.study_id.clone(),
            tokens_used: self.tokens_used,
            tokens_budget: self.tokens_budget,
            p_completion: self.estimate_completion_probability(),
            confidence_in_output: self.confidence,
            surprise_index,
            memory_hazard_rate,
            protocol_mode: self.protocol_mode,
        }
    }

    /// Check if the study should halt immediately.
    pub fn should_halt(&self) -> bool {
        matches!(self.protocol_mode, ProtocolMode::EmergencyHalt)
            && self.current_phase != StudyPhase::Publication
    }

    /// Return the designated agent role for the current phase.
    pub fn phase_lead(&self) -> AgentRole {
        match self.current_phase {
            StudyPhase::LiteratureReview => AgentRole::PrincipalInvestigator,
            StudyPhase::Hypothesis => AgentRole::PrincipalInvestigator,
            StudyPhase::ProtocolDesign => AgentRole::Practitioner,
            StudyPhase::Experiment => AgentRole::ResearchFellow,
            StudyPhase::PeerReview => AgentRole::Practitioner,
            StudyPhase::Publication => AgentRole::ResearchFellow,
        }
    }
}

/// Determine the protocol mode based on budget consumption.
fn determine_protocol_mode(
    tokens_used: u64,
    tokens_budget: u64,
    complexity: f64,
    config: &ActuarialConfig,
) -> ProtocolMode {
    let p = budget_survival(tokens_used, tokens_budget, complexity);

    if p > 0.80 {
        ProtocolMode::Full
    } else if p > config.min_publishable_threshold {
        ProtocolMode::Abbreviated
    } else if p > config.halt_threshold {
        ProtocolMode::MinimumPublishableUnit
    } else {
        ProtocolMode::EmergencyHalt
    }
}

/// Budget survival function.
///
/// Models the probability that the remaining budget is sufficient for completion.
/// Uses a logistic model: P(completion) = 1 / (1 + exp(k * (consumed/budget - threshold)))
/// where the threshold and steepness depend on estimated complexity.
fn budget_survival(tokens_used: u64, tokens_budget: u64, complexity: f64) -> f64 {
    if tokens_budget == 0 {
        return 0.0;
    }

    let fraction_consumed = tokens_used as f64 / tokens_budget as f64;

    // The threshold shifts left with increasing complexity.
    // For complexity 1.0, threshold is 0.70 (expect completion at 70% usage).
    // For complexity 2.0, threshold is 0.50.
    let threshold = (0.70 / complexity).min(0.95);

    // Steepness of the transition.
    let steepness = 8.0 * complexity;

    // Logistic survival: high when fraction_consumed << threshold, drops off.
    1.0 / (1.0 + (steepness * (fraction_consumed - threshold)).exp())
}

/// Full protocol phase transitions.
fn next_phase_full(current: StudyPhase) -> Option<StudyPhase> {
    match current {
        StudyPhase::LiteratureReview => Some(StudyPhase::Hypothesis),
        StudyPhase::Hypothesis => Some(StudyPhase::ProtocolDesign),
        StudyPhase::ProtocolDesign => Some(StudyPhase::Experiment),
        StudyPhase::Experiment => Some(StudyPhase::PeerReview),
        StudyPhase::PeerReview => Some(StudyPhase::Publication),
        StudyPhase::Publication => None,
    }
}

/// Abbreviated protocol: skip peer review iteration.
fn next_phase_abbreviated(current: StudyPhase) -> Option<StudyPhase> {
    match current {
        StudyPhase::LiteratureReview => Some(StudyPhase::Hypothesis),
        StudyPhase::Hypothesis => Some(StudyPhase::ProtocolDesign),
        StudyPhase::ProtocolDesign => Some(StudyPhase::Experiment),
        StudyPhase::Experiment => Some(StudyPhase::Publication),
        StudyPhase::PeerReview => Some(StudyPhase::Publication),
        StudyPhase::Publication => None,
    }
}

/// Minimum publishable unit: literature review, experiment, publication.
fn next_phase_mpu(current: StudyPhase) -> Option<StudyPhase> {
    match current {
        StudyPhase::LiteratureReview => Some(StudyPhase::Experiment),
        StudyPhase::Hypothesis => Some(StudyPhase::Experiment),
        StudyPhase::ProtocolDesign => Some(StudyPhase::Experiment),
        StudyPhase::Experiment => Some(StudyPhase::Publication),
        StudyPhase::PeerReview => Some(StudyPhase::Publication),
        StudyPhase::Publication => None,
    }
}

/// Generate a study ID in the LRRC format.
///
/// Format: LRRC-{year}-{sequence}
pub fn generate_study_id(year: u32, sequence: u32) -> StudyId {
    StudyId(format!("LRRC-{year}-{sequence:03}"))
}

/// Compute the estimated token cost for a phase.
///
/// Based on the budget table from the proposal.
pub fn estimated_phase_cost(phase: StudyPhase) -> (u64, u64) {
    match phase {
        StudyPhase::LiteratureReview => (3000, 800),
        StudyPhase::Hypothesis => (2000, 1200),
        StudyPhase::ProtocolDesign => (1500, 800),
        StudyPhase::Experiment => (3600, 4800), // ~3 steps * per-step cost
        StudyPhase::PeerReview => (2000, 800),
        StudyPhase::Publication => (400, 500),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ActuarialConfig;

    #[test]
    fn new_study_starts_in_literature_review() {
        let config = ActuarialConfig::default();
        let protocol = StudyProtocol::new(StudyId("test".into()), &config);
        assert_eq!(protocol.current_phase, StudyPhase::LiteratureReview);
        assert_eq!(protocol.protocol_mode, ProtocolMode::Full);
    }

    #[test]
    fn full_protocol_traverses_all_phases() {
        let config = ActuarialConfig::default();
        let mut protocol = StudyProtocol::new(StudyId("test".into()), &config);

        let phases = [
            StudyPhase::Hypothesis,
            StudyPhase::ProtocolDesign,
            StudyPhase::Experiment,
            StudyPhase::PeerReview,
            StudyPhase::Publication,
        ];

        for expected in &phases {
            let next = protocol.advance_phase();
            assert_eq!(next, Some(*expected));
        }

        assert_eq!(protocol.advance_phase(), None);
    }

    #[test]
    fn budget_survival_decreases_with_consumption() {
        let p1 = budget_survival(10_000, 50_000, 1.0);
        let p2 = budget_survival(30_000, 50_000, 1.0);
        let p3 = budget_survival(45_000, 50_000, 1.0);
        assert!(p1 > p2);
        assert!(p2 > p3);
    }

    #[test]
    fn high_complexity_reduces_completion_probability() {
        let p_easy = budget_survival(25_000, 50_000, 1.0);
        let p_hard = budget_survival(25_000, 50_000, 2.0);
        assert!(p_easy > p_hard);
    }

    #[test]
    fn protocol_mode_downgrades_with_consumption() {
        let config = ActuarialConfig::default();
        let mut protocol = StudyProtocol::new(StudyId("test".into()), &config);

        protocol.consume_tokens(5_000, &config);
        assert_eq!(protocol.protocol_mode, ProtocolMode::Full);

        protocol.consume_tokens(35_000, &config);
        // Should be at least Abbreviated by now.
        assert!(matches!(
            protocol.protocol_mode,
            ProtocolMode::Abbreviated
                | ProtocolMode::MinimumPublishableUnit
                | ProtocolMode::EmergencyHalt
        ));
    }
}
