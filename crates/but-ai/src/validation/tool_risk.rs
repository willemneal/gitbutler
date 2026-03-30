//! Tool risk classification.
//!
//! Classifies tools by their reversibility to determine which operations
//! require validation before execution:
//!
//! - **Reversible**: read-only operations, branch creation (safe to auto-approve)
//! - **Cautious**: commits, amends, config changes (need review)
//! - **Irreversible**: push, force-push, delete (require explicit validation)

use serde::{Deserialize, Serialize};

/// Risk level for a tool or operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Safe: read-only or trivially reversible.
    Reversible,
    /// Needs review: modifies local state in ways that can be undone.
    Cautious,
    /// Dangerous: modifies shared state or cannot be undone.
    Irreversible,
}

/// Result of classifying a tool.
#[derive(Debug, Clone)]
pub struct ToolClassification {
    /// The tool name that was classified.
    pub tool_name: String,
    /// The determined risk level.
    pub risk_level: RiskLevel,
    /// Reason for the classification.
    pub reason: String,
}

/// The tool risk classifier.
pub struct ToolRiskClassifier;

impl ToolRiskClassifier {
    /// Classify a tool by name, returning its risk level with justification.
    pub fn classify_tool(name: &str) -> ToolClassification {
        let lower = name.to_lowercase();
        let (level, reason) = Self::determine_risk(&lower);

        ToolClassification {
            tool_name: name.to_string(),
            risk_level: level,
            reason: reason.to_string(),
        }
    }

    /// Whether a given risk level requires validation before execution.
    pub fn requires_validation(level: RiskLevel) -> bool {
        matches!(level, RiskLevel::Cautious | RiskLevel::Irreversible)
    }

    /// Whether a given risk level requires explicit user confirmation.
    pub fn requires_confirmation(level: RiskLevel) -> bool {
        matches!(level, RiskLevel::Irreversible)
    }

    fn determine_risk(tool_name: &str) -> (RiskLevel, &'static str) {
        // Irreversible operations: affect shared state or are destructive.
        if Self::matches_any(tool_name, &[
            "push", "force-push", "force_push",
            "delete-branch", "delete_branch",
            "delete-remote", "delete_remote",
            "drop", "truncate",
            "rm", "remove",
            "reset --hard", "reset_hard",
            "clean",
        ]) {
            return (RiskLevel::Irreversible, "modifies shared state or is destructive");
        }

        // Cautious operations: modify local state but can be undone.
        if Self::matches_any(tool_name, &[
            "commit", "amend",
            "rebase", "merge",
            "checkout", "switch",
            "stash",
            "reset", "restore",
            "config", "configure",
            "install", "uninstall",
            "write", "edit", "create",
            "rename", "move",
        ]) {
            return (RiskLevel::Cautious, "modifies local state");
        }

        // Everything else is considered reversible (read-only or safe).
        if Self::matches_any(tool_name, &[
            "read", "cat", "show", "log", "diff", "status",
            "list", "ls", "find", "grep", "search",
            "branch-create", "branch_create",
            "fetch", "pull",
            "blame", "annotate",
            "describe", "help",
        ]) {
            return (RiskLevel::Reversible, "read-only or trivially reversible");
        }

        // Default: unknown tools are cautious.
        (RiskLevel::Cautious, "unknown tool, defaulting to cautious")
    }

    fn matches_any(name: &str, patterns: &[&str]) -> bool {
        patterns.iter().any(|p| name.contains(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_operations_are_reversible() {
        let c = ToolRiskClassifier::classify_tool("git-log");
        assert_eq!(c.risk_level, RiskLevel::Reversible);
        assert!(!ToolRiskClassifier::requires_validation(c.risk_level));
    }

    #[test]
    fn commit_is_cautious() {
        let c = ToolRiskClassifier::classify_tool("git-commit");
        assert_eq!(c.risk_level, RiskLevel::Cautious);
        assert!(ToolRiskClassifier::requires_validation(c.risk_level));
        assert!(!ToolRiskClassifier::requires_confirmation(c.risk_level));
    }

    #[test]
    fn push_is_irreversible() {
        let c = ToolRiskClassifier::classify_tool("git-push");
        assert_eq!(c.risk_level, RiskLevel::Irreversible);
        assert!(ToolRiskClassifier::requires_validation(c.risk_level));
        assert!(ToolRiskClassifier::requires_confirmation(c.risk_level));
    }

    #[test]
    fn force_push_is_irreversible() {
        let c = ToolRiskClassifier::classify_tool("force-push");
        assert_eq!(c.risk_level, RiskLevel::Irreversible);
    }

    #[test]
    fn delete_branch_is_irreversible() {
        let c = ToolRiskClassifier::classify_tool("delete-branch");
        assert_eq!(c.risk_level, RiskLevel::Irreversible);
    }

    #[test]
    fn unknown_tool_is_cautious() {
        let c = ToolRiskClassifier::classify_tool("some-unknown-thing");
        assert_eq!(c.risk_level, RiskLevel::Cautious);
    }

    #[test]
    fn risk_levels_are_ordered() {
        assert!(RiskLevel::Reversible < RiskLevel::Cautious);
        assert!(RiskLevel::Cautious < RiskLevel::Irreversible);
    }
}
