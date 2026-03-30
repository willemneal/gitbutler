//! Role-based authorization checking.
//!
//! Validates whether an agent is authorized to perform an operation
//! based on branch patterns, patch size limits, and repo scope.

use crate::types::{AgentIdentity, AgentRole, AuthorizationScope};

/// Result of an authorization check.
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    /// Whether the operation is authorized.
    pub authorized: bool,
    /// If denied, the reason.
    pub denial_reason: Option<String>,
}

impl AuthorizationResult {
    pub fn allowed() -> Self {
        Self {
            authorized: true,
            denial_reason: None,
        }
    }

    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            authorized: false,
            denial_reason: Some(reason.into()),
        }
    }
}

/// Check whether an agent is authorized to commit to a branch.
pub fn check_branch_authorization(scope: &AuthorizationScope, branch: &str) -> AuthorizationResult {
    if scope.branch_patterns.is_empty() {
        return AuthorizationResult::denied("No branch patterns authorized");
    }

    for pattern in &scope.branch_patterns {
        if matches_glob(pattern, branch) {
            return AuthorizationResult::allowed();
        }
    }

    AuthorizationResult::denied(format!(
        "Branch '{}' does not match any authorized pattern: {:?}",
        branch, scope.branch_patterns
    ))
}

/// Check whether a patch size is within the agent's limit.
pub fn check_patch_size(scope: &AuthorizationScope, patch_lines: u32) -> AuthorizationResult {
    match scope.max_patch_lines {
        Some(max) if patch_lines > max => AuthorizationResult::denied(format!(
            "Patch size {} lines exceeds limit of {} lines",
            patch_lines, max
        )),
        _ => AuthorizationResult::allowed(),
    }
}

/// Check whether an agent is authorized for a repository.
pub fn check_repo_authorization(scope: &AuthorizationScope, repo: &str) -> AuthorizationResult {
    if scope.repos.is_empty() {
        return AuthorizationResult::denied("No repositories authorized");
    }

    for pattern in &scope.repos {
        if pattern == "*" || pattern == repo {
            return AuthorizationResult::allowed();
        }
    }

    AuthorizationResult::denied(format!(
        "Repository '{}' not in authorized list: {:?}",
        repo, scope.repos
    ))
}

/// Check whether an agent is authorized for a call number range.
pub fn check_call_number_authorization(
    scope: &AuthorizationScope,
    call_number: &str,
) -> AuthorizationResult {
    if scope.call_number_ranges.is_empty() {
        return AuthorizationResult::allowed();
    }

    for pattern in &scope.call_number_ranges {
        if matches_glob(pattern, call_number) {
            return AuthorizationResult::allowed();
        }
    }

    AuthorizationResult::denied(format!(
        "Call number '{}' not in authorized ranges: {:?}",
        call_number, scope.call_number_ranges
    ))
}

/// Full authorization check: branch + patch size + repo.
pub fn check_full_authorization(
    identity: &AgentIdentity,
    branch: &str,
    repo: &str,
    patch_lines: u32,
) -> AuthorizationResult {
    let scope = &identity.authorization;

    let branch_check = check_branch_authorization(scope, branch);
    if !branch_check.authorized {
        return branch_check;
    }

    let repo_check = check_repo_authorization(scope, repo);
    if !repo_check.authorized {
        return repo_check;
    }

    let patch_check = check_patch_size(scope, patch_lines);
    if !patch_check.authorized {
        return patch_check;
    }

    AuthorizationResult::allowed()
}

/// Whether a role is allowed to produce patches.
pub fn can_produce_patches(role: AgentRole) -> bool {
    matches!(role, AgentRole::Implementer | AgentRole::Architect)
}

/// Simple glob matching: supports `*` as a wildcard.
///
/// - `"*"` matches everything.
/// - `"feat/*"` matches `"feat/anything"`.
/// - `"SEC.*"` matches `"SEC.AUTH"`, `"SEC.CRYPTO"`, etc.
/// - Exact match otherwise.
fn matches_glob(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scope() -> AuthorizationScope {
        AuthorizationScope {
            branch_patterns: vec!["feat/*".to_string(), "fix/*".to_string()],
            max_patch_lines: Some(500),
            repos: vec!["*".to_string()],
            call_number_ranges: vec!["ARCH.*".to_string(), "SEC.*".to_string()],
        }
    }

    #[test]
    fn branch_authorized() {
        let scope = test_scope();
        assert!(check_branch_authorization(&scope, "feat/new-thing").authorized);
        assert!(check_branch_authorization(&scope, "fix/bug-123").authorized);
    }

    #[test]
    fn branch_denied() {
        let scope = test_scope();
        assert!(!check_branch_authorization(&scope, "main").authorized);
        assert!(!check_branch_authorization(&scope, "release/1.0").authorized);
    }

    #[test]
    fn patch_size_within_limit() {
        let scope = test_scope();
        assert!(check_patch_size(&scope, 100).authorized);
        assert!(check_patch_size(&scope, 500).authorized);
    }

    #[test]
    fn patch_size_exceeds_limit() {
        let scope = test_scope();
        assert!(!check_patch_size(&scope, 501).authorized);
    }

    #[test]
    fn wildcard_repo() {
        let scope = test_scope();
        assert!(check_repo_authorization(&scope, "any-repo").authorized);
    }

    #[test]
    fn call_number_authorization() {
        let scope = test_scope();
        assert!(check_call_number_authorization(&scope, "ARCH.AUTH").authorized);
        assert!(check_call_number_authorization(&scope, "SEC.CRYPTO").authorized);
        assert!(!check_call_number_authorization(&scope, "OPS.DEPLOY").authorized);
    }

    #[test]
    fn empty_call_number_ranges_means_unrestricted() {
        let scope = AuthorizationScope {
            branch_patterns: vec!["*".to_string()],
            max_patch_lines: None,
            repos: vec!["*".to_string()],
            call_number_ranges: vec![],
        };
        assert!(check_call_number_authorization(&scope, "ANYTHING").authorized);
    }
}
