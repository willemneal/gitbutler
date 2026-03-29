//! Validation and boundary checking.
//!
//! The selvedge is the tightly woven edge that prevents fabric from unraveling.
//! This module validates produced fabrics for structural integrity and
//! module boundary conformance.

mod boundary;
mod integrity;

pub use boundary::{BoundaryChecker, BoundaryReport, BoundaryViolation, BoundaryViolationKind};
pub use integrity::{IntegrityChecker, IntegrityReport, IntegrityViolation, IntegrityViolationKind};

use crate::types::Fabric;

/// Orchestrates all validation checks on a produced fabric.
pub struct SelvedgeValidator {
    integrity: IntegrityChecker,
    boundary: BoundaryChecker,
}

impl SelvedgeValidator {
    pub fn new() -> Self {
        Self {
            integrity: IntegrityChecker::new(),
            boundary: BoundaryChecker::new(),
        }
    }

    /// Validate a complete fabric. Returns a report describing all violations found.
    pub fn validate(&self, fabric: &Fabric) -> ValidationReport {
        let integrity = self.integrity.check(fabric);
        let boundary = self.boundary.check(fabric);
        ValidationReport {
            integrity,
            boundary,
        }
    }
}

impl Default for SelvedgeValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined validation report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationReport {
    pub integrity: IntegrityReport,
    pub boundary: BoundaryReport,
}

impl ValidationReport {
    /// Returns true if no violations were found.
    pub fn is_sound(&self) -> bool {
        self.integrity.violations.is_empty() && self.boundary.violations.is_empty()
    }

    /// Total number of violations.
    pub fn violation_count(&self) -> usize {
        self.integrity.violations.len() + self.boundary.violations.len()
    }
}
