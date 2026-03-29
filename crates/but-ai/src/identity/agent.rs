//! Peer agent identity -- no hierarchy.
//!
//! Every agent in the collective is a peer. No agent has more authority
//! than any other. Identity records are stored in Git refs and never expire.
//! They are versioned -- each update increments the version counter and
//! the old version is preserved in Git history.

use crate::types::{
    AgentCapability, AgentId, AgentIdentity, AuthorizationScope,
};

/// Registry of known agents in the collective.
///
/// The registry is populated from `refs/but-ai/memory/<agent-id>/identity/self`
/// entries. In this in-memory implementation, agents are registered manually.
pub struct AgentRegistry {
    agents: std::collections::HashMap<String, AgentIdentity>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: std::collections::HashMap::new(),
        }
    }

    /// Register an agent identity. If the agent already exists,
    /// the new identity must have a higher version number.
    pub fn register(&mut self, identity: AgentIdentity) -> anyhow::Result<()> {
        let key = identity.name.0.clone();
        if let Some(existing) = self.agents.get(&key) {
            if identity.version <= existing.version {
                anyhow::bail!(
                    "Agent {} version {} is not newer than existing version {}",
                    key,
                    identity.version,
                    existing.version
                );
            }
        }
        self.agents.insert(key, identity);
        Ok(())
    }

    /// Look up an agent by ID.
    pub fn get(&self, agent: &AgentId) -> Option<&AgentIdentity> {
        self.agents.get(&agent.0)
    }

    /// List all registered agents.
    pub fn all(&self) -> Vec<&AgentIdentity> {
        self.agents.values().collect()
    }

    /// List all agent IDs.
    pub fn all_ids(&self) -> Vec<AgentId> {
        self.agents.keys().map(|k| AgentId(k.clone())).collect()
    }

    /// Number of registered agents.
    pub fn count(&self) -> usize {
        self.agents.len()
    }

    /// Check whether an agent is authorized for a specific branch and repo.
    pub fn is_authorized(
        &self,
        agent: &AgentId,
        branch: &str,
        repo: &str,
    ) -> AuthorizationCheck {
        match self.get(agent) {
            None => AuthorizationCheck::Denied {
                reason: format!("Agent {} not registered", agent),
            },
            Some(identity) => check_authorization(&identity.authorization_scope, branch, repo),
        }
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of an authorization check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationCheck {
    Allowed,
    Denied { reason: String },
}

/// Check a specific authorization scope against a branch and repo.
fn check_authorization(
    scope: &AuthorizationScope,
    branch: &str,
    repo: &str,
) -> AuthorizationCheck {
    // Check repo authorization
    let repo_allowed = scope.repos.is_empty()
        || scope.repos.iter().any(|r| {
            if r.contains('*') {
                glob_match(r, repo)
            } else {
                r == repo
            }
        });

    if !repo_allowed {
        return AuthorizationCheck::Denied {
            reason: format!("Agent not authorized for repo {}", repo),
        };
    }

    // Check branch authorization
    let branch_allowed = scope.branches.is_empty()
        || scope.branches.iter().any(|b| {
            if b.contains('*') {
                glob_match(b, branch)
            } else {
                b == branch
            }
        });

    if !branch_allowed {
        return AuthorizationCheck::Denied {
            reason: format!("Agent not authorized for branch {}", branch),
        };
    }

    AuthorizationCheck::Allowed
}

/// Simple glob matching for branch/repo patterns.
/// Supports `*` (matches any sequence within a path segment) and `**` (matches anything).
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "**" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix("/*") {
        // "feat/*" matches "feat/anything"
        if let Some(rest) = value.strip_prefix(prefix) {
            return rest.starts_with('/') && !rest[1..].contains('/');
        }
        return false;
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        // "feat/**" matches "feat/anything/nested"
        return value.starts_with(prefix) && value[prefix.len()..].starts_with('/');
    }

    pattern == value
}

/// Create a builder for constructing agent identities.
pub struct AgentIdentityBuilder {
    name: AgentId,
    organization: String,
    capabilities: Vec<AgentCapability>,
    branches: Vec<String>,
    repos: Vec<String>,
    max_patch_lines: Option<u32>,
}

impl AgentIdentityBuilder {
    pub fn new(name: impl Into<String>, organization: impl Into<String>) -> Self {
        Self {
            name: AgentId(name.into()),
            organization: organization.into(),
            capabilities: Vec::new(),
            branches: Vec::new(),
            repos: Vec::new(),
            max_patch_lines: None,
        }
    }

    pub fn capability(mut self, cap: AgentCapability) -> Self {
        self.capabilities.push(cap);
        self
    }

    pub fn branch_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.branches.push(pattern.into());
        self
    }

    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.repos.push(repo.into());
        self
    }

    pub fn max_patch_lines(mut self, max: u32) -> Self {
        self.max_patch_lines = Some(max);
        self
    }

    pub fn build(self, now: String) -> AgentIdentity {
        AgentIdentity {
            name: self.name,
            organization: self.organization,
            capabilities: self.capabilities,
            authorization_scope: AuthorizationScope {
                branches: self.branches,
                repos: self.repos,
                max_patch_lines: self.max_patch_lines,
            },
            signing_key_fingerprint: None,
            created: now,
            version: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dara_identity() -> AgentIdentity {
        AgentIdentityBuilder::new("dara", "tidal-protocol-collective")
            .capability(AgentCapability::PatchGeneration)
            .capability(AgentCapability::DiffAnalysis)
            .branch_pattern("feat/*")
            .branch_pattern("fix/*")
            .repo("gitbutler/but")
            .max_patch_lines(1000)
            .build("2026-01-15T00:00:00Z".to_string())
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = AgentRegistry::new();
        registry.register(dara_identity()).unwrap();

        let dara = AgentId("dara".to_string());
        assert!(registry.get(&dara).is_some());
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn version_must_increase() {
        let mut registry = AgentRegistry::new();
        registry.register(dara_identity()).unwrap();

        // Same version should fail
        assert!(registry.register(dara_identity()).is_err());

        // Higher version should succeed
        let mut updated = dara_identity();
        updated.version = 2;
        registry.register(updated).unwrap();
    }

    #[test]
    fn authorization_check_branch_pattern() {
        let mut registry = AgentRegistry::new();
        registry.register(dara_identity()).unwrap();

        let dara = AgentId("dara".to_string());
        assert_eq!(
            registry.is_authorized(&dara, "feat/auth", "gitbutler/but"),
            AuthorizationCheck::Allowed
        );
        assert!(matches!(
            registry.is_authorized(&dara, "main", "gitbutler/but"),
            AuthorizationCheck::Denied { .. }
        ));
    }

    #[test]
    fn glob_match_patterns() {
        assert!(glob_match("feat/*", "feat/auth"));
        assert!(!glob_match("feat/*", "feat/auth/nested"));
        assert!(glob_match("**", "anything/goes/here"));
        assert!(!glob_match("feat/*", "fix/bug"));
    }
}
