use serde::Serialize;

use crate::types::Fabric;

/// A specific boundary violation found in the fabric.
#[derive(Debug, Clone, Serialize)]
pub struct BoundaryViolation {
    pub kind: BoundaryViolationKind,
    pub message: String,
    pub file_path: Option<String>,
}

/// Classification of boundary violations.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryViolationKind {
    /// Patch modifies files outside the agent's authorized scope.
    UnauthorizedFile,
    /// Patch exceeds the maximum allowed line count.
    PatchTooLarge,
    /// Patch modifies a module boundary (public API) without warp approval.
    BoundaryModification,
}

/// Report from boundary checking.
#[derive(Debug, Clone, Serialize)]
pub struct BoundaryReport {
    pub violations: Vec<BoundaryViolation>,
}

/// Checks that the fabric respects module boundaries and authorization constraints.
pub struct BoundaryChecker {
    /// Maximum patch size in lines. None means no limit.
    max_patch_lines: Option<u32>,
    /// File patterns the agent is authorized to modify.
    authorized_patterns: Vec<String>,
}

impl BoundaryChecker {
    pub fn new() -> Self {
        Self {
            max_patch_lines: None,
            authorized_patterns: Vec::new(),
        }
    }

    /// Configure the boundary checker with authorization constraints.
    pub fn with_authorization(mut self, max_lines: Option<u32>, patterns: Vec<String>) -> Self {
        self.max_patch_lines = max_lines;
        self.authorized_patterns = patterns;
        self
    }

    /// Check a fabric for boundary violations.
    pub fn check(&self, fabric: &Fabric) -> BoundaryReport {
        let mut violations = Vec::new();

        if let Some(max) = self.max_patch_lines {
            let line_count = fabric.patch.lines().count() as u32;
            if line_count > max {
                violations.push(BoundaryViolation {
                    kind: BoundaryViolationKind::PatchTooLarge,
                    message: format!(
                        "Patch is {} lines, exceeding maximum of {}",
                        line_count, max
                    ),
                    file_path: None,
                });
            }
        }

        if !self.authorized_patterns.is_empty() {
            for file_path in Self::extract_file_paths(&fabric.patch) {
                if !self.is_authorized(&file_path) {
                    violations.push(BoundaryViolation {
                        kind: BoundaryViolationKind::UnauthorizedFile,
                        message: format!("File '{}' is outside authorized scope", file_path),
                        file_path: Some(file_path),
                    });
                }
            }
        }

        BoundaryReport { violations }
    }

    /// Extract file paths from a unified diff.
    fn extract_file_paths(patch: &str) -> Vec<String> {
        patch
            .lines()
            .filter_map(|line| line.strip_prefix("+++ b/").map(String::from))
            .collect()
    }

    /// Check if a file path matches any authorized pattern.
    fn is_authorized(&self, path: &str) -> bool {
        self.authorized_patterns
            .iter()
            .any(|pattern| Self::glob_match(pattern, path))
    }

    /// Simple glob matching for file path authorization.
    fn glob_match(pattern: &str, path: &str) -> bool {
        if pattern == "**" {
            return true;
        }
        if let Some((prefix, suffix)) = pattern.split_once("**") {
            return path.starts_with(prefix) && path.ends_with(suffix);
        }
        pattern == path
    }
}

impl Default for BoundaryChecker {
    fn default() -> Self {
        Self::new()
    }
}
