//! Agent identity with statistical credentials.
//!
//! Each agent in the LRRC has a life record: identity, role, capabilities,
//! authorization constraints, and performance history. The performance
//! history is itself a statistical summary -- mean confidence, mean patch
//! survival -- that allows the system and external observers to assess
//! the agent's reliability.

use crate::types::{AgentId, AgentRole, Authorization, LifeRecord, PerformanceHistory};

/// The complete roster of agents in the Longevity & Risk Research Centre.
pub struct ResearchCentre {
    agents: Vec<LifeRecord>,
    institution: String,
}

impl ResearchCentre {
    /// Create a new research centre with the default LRRC roster.
    pub fn lrrc() -> Self {
        let institution = "093-lrrc".to_string();
        let agents = vec![
            LifeRecord {
                agent_id: AgentId("vassiliev".to_string()),
                institution: institution.clone(),
                role: AgentRole::PrincipalInvestigator,
                specialty: "Bayesian survival modeling and system design".to_string(),
                capabilities: vec![
                    "study_design".to_string(),
                    "statistical_review".to_string(),
                    "architecture".to_string(),
                ],
                authorization: Authorization {
                    branch_patterns: vec!["*".to_string()],
                    max_patch_lines: None, // PI does not generate patches directly.
                    repos: vec!["*".to_string()],
                },
                signing_key: Some("openwallet:093-lrrc:vassiliev".to_string()),
                created_at: "2026-03-01T00:00:00Z".to_string(),
                performance_history: PerformanceHistory::default(),
            },
            LifeRecord {
                agent_id: AgentId("okonkwo".to_string()),
                institution: institution.clone(),
                role: AgentRole::Practitioner,
                specialty: "Actuarial translation and practical validation".to_string(),
                capabilities: vec![
                    "validation".to_string(),
                    "practitioner_review".to_string(),
                    "risk_assessment".to_string(),
                ],
                authorization: Authorization {
                    branch_patterns: vec!["study/*".to_string()],
                    max_patch_lines: None,
                    repos: vec!["*".to_string()],
                },
                signing_key: Some("openwallet:093-lrrc:okonkwo".to_string()),
                created_at: "2026-03-01T00:00:00Z".to_string(),
                performance_history: PerformanceHistory::default(),
            },
            LifeRecord {
                agent_id: AgentId("petrov".to_string()),
                institution: institution.clone(),
                role: AgentRole::ResearchFellow,
                specialty: "Code implementation and patch production".to_string(),
                capabilities: vec![
                    "patch_generation".to_string(),
                    "numerical_computation".to_string(),
                    "testing".to_string(),
                ],
                authorization: Authorization {
                    branch_patterns: vec!["study/*".to_string(), "feat/*".to_string()],
                    max_patch_lines: Some(800),
                    repos: vec!["*".to_string()],
                },
                signing_key: Some("openwallet:093-lrrc:petrov".to_string()),
                created_at: "2026-03-01T00:00:00Z".to_string(),
                performance_history: PerformanceHistory::default(),
            },
            LifeRecord {
                agent_id: AgentId("abebe".to_string()),
                institution: institution.clone(),
                role: AgentRole::DataCurator,
                specialty: "Survival function estimation for memory entries".to_string(),
                capabilities: vec![
                    "distribution_fitting".to_string(),
                    "memory_management".to_string(),
                    "cohort_analysis".to_string(),
                ],
                authorization: Authorization {
                    branch_patterns: vec!["refs/but-ai/actuarial/*".to_string()],
                    max_patch_lines: None, // Cannot modify code branches.
                    repos: vec!["*".to_string()],
                },
                signing_key: Some("openwallet:093-lrrc:abebe".to_string()),
                created_at: "2026-03-01T00:00:00Z".to_string(),
                performance_history: PerformanceHistory::default(),
            },
            LifeRecord {
                agent_id: AgentId("chen".to_string()),
                institution: institution.clone(),
                role: AgentRole::ResearchAssistant,
                specialty: "Cross-repo coordination and PR management".to_string(),
                capabilities: vec![
                    "coordination".to_string(),
                    "status_reporting".to_string(),
                    "dependency_tracking".to_string(),
                ],
                authorization: Authorization {
                    branch_patterns: Vec::new(), // Read-only for code.
                    max_patch_lines: None, // Cannot sign code changes.
                    repos: vec!["*".to_string()],
                },
                signing_key: Some("openwallet:093-lrrc:chen".to_string()),
                created_at: "2026-03-01T00:00:00Z".to_string(),
                performance_history: PerformanceHistory::default(),
            },
        ];

        Self {
            agents,
            institution,
        }
    }

    /// Create an empty research centre.
    pub fn empty(institution: String) -> Self {
        Self {
            agents: Vec::new(),
            institution,
        }
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, id: &AgentId) -> Option<&LifeRecord> {
        self.agents.iter().find(|a| a.agent_id == *id)
    }

    /// Get a mutable reference to an agent.
    pub fn get_agent_mut(&mut self, id: &AgentId) -> Option<&mut LifeRecord> {
        self.agents.iter_mut().find(|a| a.agent_id == *id)
    }

    /// Get all agents with a specific role.
    pub fn agents_by_role(&self, role: AgentRole) -> Vec<&LifeRecord> {
        self.agents.iter().filter(|a| a.role == role).collect()
    }

    /// Get the institution identifier.
    pub fn institution(&self) -> &str {
        &self.institution
    }

    /// Register a new agent.
    pub fn register(&mut self, agent: LifeRecord) {
        self.agents.push(agent);
    }

    /// Check if an agent is authorized for a branch pattern.
    pub fn is_authorized(&self, agent_id: &AgentId, branch: &str) -> bool {
        let agent = match self.get_agent(agent_id) {
            Some(a) => a,
            None => return false,
        };

        agent.authorization.branch_patterns.iter().any(|pattern| {
            if pattern == "*" {
                return true;
            }
            // Simple glob matching: "study/*" matches "study/anything".
            if let Some(prefix) = pattern.strip_suffix('*') {
                return branch.starts_with(prefix);
            }
            pattern == branch
        })
    }

    /// Check if a patch size is within an agent's authorization.
    pub fn is_within_patch_limit(&self, agent_id: &AgentId, patch_lines: u32) -> bool {
        let agent = match self.get_agent(agent_id) {
            Some(a) => a,
            None => return false,
        };

        match agent.authorization.max_patch_lines {
            Some(max) => patch_lines <= max,
            None => true, // No limit (or agent does not generate patches).
        }
    }

    /// Update an agent's performance history after a task completion.
    pub fn record_task_completion(
        &mut self,
        agent_id: &AgentId,
        confidence: f64,
        patch_survival_days: f64,
    ) {
        if let Some(agent) = self.get_agent_mut(agent_id) {
            let history = &mut agent.performance_history;
            let n = history.tasks_completed as f64;

            // Running mean update.
            history.mean_confidence =
                (history.mean_confidence * n + confidence) / (n + 1.0);
            history.mean_patch_survival_days =
                (history.mean_patch_survival_days * n + patch_survival_days) / (n + 1.0);
            history.tasks_completed += 1;
        }
    }

    /// Get all agents.
    pub fn all_agents(&self) -> &[LifeRecord] {
        &self.agents
    }
}

impl Default for PerformanceHistory {
    fn default() -> Self {
        Self {
            tasks_completed: 0,
            mean_confidence: 0.0,
            mean_patch_survival_days: 0.0,
        }
    }
}

/// Generate the branch name for a study.
///
/// Format: study/{study_id}/{phase}[.{dependency}]
pub fn study_branch_name(study_id: &str, phase: &str, dependency: Option<&str>) -> String {
    match dependency {
        Some(dep) => format!("study/{study_id}/{phase}.{dep}"),
        None => format!("study/{study_id}/{phase}"),
    }
}

/// Generate the signing key reference for an agent.
pub fn signing_key_ref(institution: &str, agent_id: &str) -> String {
    format!("openwallet:{institution}:{agent_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrrc_has_five_agents() {
        let centre = ResearchCentre::lrrc();
        assert_eq!(centre.all_agents().len(), 5);
    }

    #[test]
    fn vassiliev_is_pi() {
        let centre = ResearchCentre::lrrc();
        let v = centre.get_agent(&AgentId("vassiliev".into())).unwrap();
        assert_eq!(v.role, AgentRole::PrincipalInvestigator);
    }

    #[test]
    fn petrov_has_patch_limit() {
        let centre = ResearchCentre::lrrc();
        assert!(centre.is_within_patch_limit(&AgentId("petrov".into()), 500));
        assert!(!centre.is_within_patch_limit(&AgentId("petrov".into()), 900));
    }

    #[test]
    fn branch_authorization_works() {
        let centre = ResearchCentre::lrrc();
        assert!(centre.is_authorized(&AgentId("petrov".into()), "study/LRRC-2026-001/experiment"));
        assert!(centre.is_authorized(&AgentId("petrov".into()), "feat/new-feature"));
        assert!(centre.is_authorized(&AgentId("vassiliev".into()), "any-branch"));
    }

    #[test]
    fn performance_history_updates() {
        let mut centre = ResearchCentre::lrrc();
        centre.record_task_completion(&AgentId("petrov".into()), 0.90, 180.0);
        centre.record_task_completion(&AgentId("petrov".into()), 0.80, 120.0);

        let petrov = centre.get_agent(&AgentId("petrov".into())).unwrap();
        assert_eq!(petrov.performance_history.tasks_completed, 2);
        assert!((petrov.performance_history.mean_confidence - 0.85).abs() < 1e-10);
    }

    #[test]
    fn study_branch_name_formats_correctly() {
        assert_eq!(
            study_branch_name("LRRC-2026-042", "experiment", None),
            "study/LRRC-2026-042/experiment"
        );
        assert_eq!(
            study_branch_name("LRRC-2026-042", "experiment", Some("LRRC-2026-039")),
            "study/LRRC-2026-042/experiment.LRRC-2026-039"
        );
    }
}
