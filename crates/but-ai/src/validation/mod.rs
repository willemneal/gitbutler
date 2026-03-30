//! Validation and consistency checking.
//!
//! This module provides four validation subsystems:
//! - **Continuity**: detect contradictions between new and existing entries
//! - **Contradiction**: produce `Tension` objects for conflicting entries
//! - **Tool risk**: classify operations by reversibility
//! - **Integrity**: validate memory store internal consistency

pub mod continuity;
pub mod contradiction;
pub mod integrity;
pub mod tool_risk;

use crate::types::{MemoryEntry, MemoryStore, Tension};
use continuity::{ContinuityChecker, ContinuityReport};
use integrity::{IntegrityChecker, IntegrityReport};

/// Combined validation report for a new memory entry.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Continuity check results.
    pub continuity: ContinuityReport,
    /// Detected contradictions as tensions.
    pub tensions: Vec<Tension>,
    /// Store integrity check results (if requested).
    pub integrity: Option<IntegrityReport>,
}

impl ValidationReport {
    /// Whether the entry passes all validation checks.
    pub fn passes(&self) -> bool {
        self.continuity.passes && self.tensions.is_empty()
    }
}

/// Validate a new memory entry against the store.
///
/// Runs continuity checking and contradiction detection against all
/// alive entries in the store. Returns a combined report.
pub fn validate<S: MemoryStore + ?Sized>(
    entry: &MemoryEntry,
    store: &S,
) -> anyhow::Result<ValidationReport> {
    // Load all alive entries for comparison.
    let alive_ids = store.list(Some(crate::types::MemoryState::Alive))?;
    let mut existing_entries = Vec::new();
    for id in &alive_ids {
        if let Some(e) = store.load(id)? {
            existing_entries.push(e);
        }
    }

    let existing_refs: Vec<&MemoryEntry> = existing_entries.iter().collect();

    let continuity = ContinuityChecker::check(entry, &existing_refs);
    let tensions = contradiction::detect_contradictions(entry, &existing_refs);

    Ok(ValidationReport {
        continuity,
        tensions,
        integrity: None,
    })
}

/// Run a full validation including store integrity check.
pub fn validate_full<S: MemoryStore>(
    entry: &MemoryEntry,
    store: &S,
) -> anyhow::Result<ValidationReport> {
    let mut report = validate(entry, store)?;
    report.integrity = Some(IntegrityChecker::check(store)?);
    Ok(report)
}
