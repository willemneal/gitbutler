use serde::Serialize;

use crate::types::Fabric;

/// A specific integrity violation found in the fabric.
#[derive(Debug, Clone, Serialize)]
pub struct IntegrityViolation {
    pub kind: IntegrityViolationKind,
    pub message: String,
    pub location: Option<String>,
}

/// Classification of integrity violations.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityViolationKind {
    /// Patch is empty or malformed.
    EmptyPatch,
    /// Patch contains conflict markers.
    ConflictMarkers,
    /// Commit message is missing required metadata.
    MissingMetadata,
    /// Thread count in metadata does not match actual thread usage.
    ThreadCountMismatch,
}

/// Report from integrity checking.
#[derive(Debug, Clone, Serialize)]
pub struct IntegrityReport {
    pub violations: Vec<IntegrityViolation>,
}

/// Checks the structural integrity of a produced fabric.
pub struct IntegrityChecker;

impl IntegrityChecker {
    pub fn new() -> Self {
        Self
    }

    /// Check a fabric for integrity violations.
    pub fn check(&self, fabric: &Fabric) -> IntegrityReport {
        let mut violations = Vec::new();

        if fabric.patch.trim().is_empty() {
            violations.push(IntegrityViolation {
                kind: IntegrityViolationKind::EmptyPatch,
                message: "Fabric contains no patch content".to_string(),
                location: None,
            });
        }

        if fabric.patch.contains("<<<<<<<") || fabric.patch.contains(">>>>>>>") {
            violations.push(IntegrityViolation {
                kind: IntegrityViolationKind::ConflictMarkers,
                message: "Fabric contains unresolved conflict markers".to_string(),
                location: None,
            });
        }

        if fabric.commit_message.trim().is_empty() {
            violations.push(IntegrityViolation {
                kind: IntegrityViolationKind::MissingMetadata,
                message: "Commit message is empty".to_string(),
                location: None,
            });
        }

        IntegrityReport { violations }
    }
}

impl Default for IntegrityChecker {
    fn default() -> Self {
        Self::new()
    }
}
