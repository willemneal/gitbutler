//! Agent identity management.
//!
//! Identity is encoded as a colophon -- a signed document modeled on the
//! colophon pages found in hand-printed books. Each agent has a colophon
//! that records their role, capabilities, authorization scope, and signing key.

use crate::types::{
    AgentAuthorization, AgentId, Colophon, KeyEvent, KeyLifecycleEntry, WorkshopRole,
};

/// Manages the identity and authorization of workshop agents.
pub struct IdentityManager {
    /// Registered agent colophons.
    colophons: Vec<Colophon>,
    /// Key lifecycle log.
    key_log: Vec<KeyLifecycleEntry>,
}

impl IdentityManager {
    /// Create a new identity manager.
    pub fn new() -> Self {
        Self {
            colophons: Vec::new(),
            key_log: Vec::new(),
        }
    }

    /// Create a new identity manager pre-populated with the standard
    /// Loom & Verse workshop roles.
    pub fn with_standard_workshop(commune: &str, timestamp: &str) -> Self {
        let mut mgr = Self::new();

        // Orozco: the author.
        mgr.register(Colophon {
            agent_id: AgentId("orozco".to_string()),
            commune: commune.to_string(),
            role: WorkshopRole::Author,
            capabilities: vec![
                "patch_generation".to_string(),
                "refactoring".to_string(),
                "bug_fixes".to_string(),
            ],
            style: "concrete, iterative, material".to_string(),
            authorization: AgentAuthorization {
                branch_patterns: vec!["chapter/*".to_string(), "feat/*".to_string()],
                max_chapter_size: Some(500),
                repos: Vec::new(),
            },
            signing_key: Some(format!("openwallet:{}:orozco", commune)),
            first_chapter: None,
            chapters_authored: 0,
            created_at: timestamp.to_string(),
        });

        // Brenner: the editor.
        mgr.register(Colophon {
            agent_id: AgentId("brenner".to_string()),
            commune: commune.to_string(),
            role: WorkshopRole::Editor,
            capabilities: vec![
                "memory_curation".to_string(),
                "motif_tracking".to_string(),
                "narrative_annotation".to_string(),
            ],
            style: "lyrical, expansive, thematic".to_string(),
            authorization: AgentAuthorization {
                branch_patterns: vec!["refs/but-ai/story/*".to_string()],
                max_chapter_size: None,
                repos: Vec::new(),
            },
            signing_key: Some(format!("openwallet:{}:brenner", commune)),
            first_chapter: None,
            chapters_authored: 0,
            created_at: timestamp.to_string(),
        });

        // Sato: the continuity checker.
        mgr.register(Colophon {
            agent_id: AgentId("sato".to_string()),
            commune: commune.to_string(),
            role: WorkshopRole::ContinuityChecker,
            capabilities: vec![
                "consistency_validation".to_string(),
                "convention_tracking".to_string(),
            ],
            style: "precise, factual, brief".to_string(),
            authorization: AgentAuthorization {
                branch_patterns: Vec::new(), // Read-only.
                max_chapter_size: None,
                repos: Vec::new(),
            },
            signing_key: Some(format!("openwallet:{}:sato", commune)),
            first_chapter: None,
            chapters_authored: 0,
            created_at: timestamp.to_string(),
        });

        // Hartmann: the publisher.
        mgr.register(Colophon {
            agent_id: AgentId("hartmann".to_string()),
            commune: commune.to_string(),
            role: WorkshopRole::Publisher,
            capabilities: vec![
                "cross_repo_coordination".to_string(),
                "correspondence".to_string(),
                "translation".to_string(),
            ],
            style: "bilingual, precise, timing-aware".to_string(),
            authorization: AgentAuthorization {
                branch_patterns: Vec::new(), // No direct codebase writes.
                max_chapter_size: None,
                repos: Vec::new(),
            },
            signing_key: Some(format!("openwallet:{}:hartmann", commune)),
            first_chapter: None,
            chapters_authored: 0,
            created_at: timestamp.to_string(),
        });

        mgr
    }

    /// Register a new agent identity.
    pub fn register(&mut self, colophon: Colophon) {
        // Record key provisioning.
        if let Some(ref key) = colophon.signing_key {
            self.key_log.push(KeyLifecycleEntry {
                event: KeyEvent::Provisioned,
                key_id: key.clone(),
                timestamp: colophon.created_at.clone(),
                succeeded_by: None,
                forgery_detected_at: None,
            });
        }
        self.colophons.push(colophon);
    }

    /// Get an agent's colophon by ID.
    pub fn get_colophon(&self, agent_id: &AgentId) -> Option<&Colophon> {
        self.colophons.iter().find(|c| c.agent_id == *agent_id)
    }

    /// Get a mutable reference to an agent's colophon.
    pub fn get_colophon_mut(&mut self, agent_id: &AgentId) -> Option<&mut Colophon> {
        self.colophons.iter_mut().find(|c| c.agent_id == *agent_id)
    }

    /// Check if an agent is authorized for a given branch pattern.
    pub fn is_authorized_for_branch(&self, agent_id: &AgentId, branch: &str) -> bool {
        self.get_colophon(agent_id)
            .map(|c| {
                c.authorization
                    .branch_patterns
                    .iter()
                    .any(|pattern| branch_matches(pattern, branch))
            })
            .unwrap_or(false)
    }

    /// Increment the chapters authored count for an agent.
    pub fn record_chapter_authored(&mut self, agent_id: &AgentId, chapter_number: u64) {
        if let Some(colophon) = self.get_colophon_mut(agent_id) {
            colophon.chapters_authored += 1;
            if colophon.first_chapter.is_none() {
                colophon.first_chapter = Some(chapter_number);
            }
        }
    }

    /// Rotate an agent's signing key.
    pub fn rotate_key(
        &mut self,
        agent_id: &AgentId,
        new_key: &str,
        timestamp: &str,
    ) -> anyhow::Result<()> {
        let colophon = self
            .get_colophon_mut(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {:?}", agent_id))?;

        let old_key = colophon.signing_key.take();

        // Record rotation.
        if let Some(ref old) = old_key {
            self.key_log.push(KeyLifecycleEntry {
                event: KeyEvent::Rotated,
                key_id: old.clone(),
                timestamp: timestamp.to_string(),
                succeeded_by: Some(new_key.to_string()),
                forgery_detected_at: None,
            });
        }

        // Need to re-find because of borrow checker.
        if let Some(colophon) = self.colophons.iter_mut().find(|c| c.agent_id == *agent_id) {
            colophon.signing_key = Some(new_key.to_string());
        }

        self.key_log.push(KeyLifecycleEntry {
            event: KeyEvent::Provisioned,
            key_id: new_key.to_string(),
            timestamp: timestamp.to_string(),
            succeeded_by: None,
            forgery_detected_at: None,
        });

        Ok(())
    }

    /// Report a compromised key.
    pub fn report_compromise(
        &mut self,
        agent_id: &AgentId,
        timestamp: &str,
    ) -> anyhow::Result<()> {
        // Collect the key ID first, then modify.
        let key_id = self
            .get_colophon(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {:?}", agent_id))?
            .signing_key
            .clone();

        if let Some(ref key) = key_id {
            self.key_log.push(KeyLifecycleEntry {
                event: KeyEvent::Compromised,
                key_id: key.clone(),
                timestamp: timestamp.to_string(),
                succeeded_by: None,
                forgery_detected_at: Some(timestamp.to_string()),
            });
        }

        if let Some(colophon) = self.colophons.iter_mut().find(|c| c.agent_id == *agent_id) {
            colophon.signing_key = None;
            colophon.authorization.branch_patterns.clear();
            colophon.authorization.repos.clear();
        }

        Ok(())
    }

    /// Get the full key lifecycle log.
    pub fn key_log(&self) -> &[KeyLifecycleEntry] {
        &self.key_log
    }

    /// Get all registered agents.
    pub fn all_agents(&self) -> &[Colophon] {
        &self.colophons
    }

    /// Get agents by role.
    pub fn agents_by_role(&self, role: WorkshopRole) -> Vec<&Colophon> {
        self.colophons.iter().filter(|c| c.role == role).collect()
    }
}

impl Default for IdentityManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob-style branch pattern matching.
///
/// Supports `*` as a wildcard for a single path component and `**` is not
/// needed since branches use `/` separators.
fn branch_matches(pattern: &str, branch: &str) -> bool {
    if pattern == branch {
        return true;
    }

    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        return branch.starts_with(prefix);
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return branch.starts_with(prefix);
    }

    false
}
