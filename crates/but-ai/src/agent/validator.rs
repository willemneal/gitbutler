//! Conditional validator agent.
//!
//! Runs continuity and contradiction checks from the validation module
//! against the memory store. Only activated when budget allows **or**
//! when active tensions are detected (tensions always warrant validation
//! regardless of budget).

use crate::types::{
    AgentId, CallNumber, Classification, EntryId, MemoryEntry, MemoryState, MemoryStore,
    SurvivalDistribution, SurvivalMetadata, Tension, TensionSeverity, TensionId,
};
use crate::validation;

use super::{PatchOutput, ValidationResult};

/// The validator agent. Conditionally runs checks based on budget and tensions.
pub struct Validator;

impl Validator {
    /// Validate a patch output against the memory store.
    ///
    /// Creates a synthetic memory entry from the patch and runs the
    /// validation module's continuity and contradiction checks against
    /// all alive entries in the store.
    pub fn validate(
        patch: &PatchOutput,
        store: &dyn MemoryStore,
    ) -> anyhow::Result<ValidationResult> {
        // Create a synthetic memory entry representing the patch output.
        let synthetic = Self::patch_to_entry(patch);

        // Run validation against the store.
        let report = validation::validate(&synthetic, store)?;

        // Convert validation report to our result type.
        let passed = report.passes();
        let issues: Vec<String> = report
            .continuity
            .observations
            .iter()
            .map(|obs| format!("[{:?}] {}", obs.severity, obs.description))
            .collect();

        let tensions_detected: Vec<Tension> = report.tensions;

        Ok(ValidationResult {
            passed,
            issues,
            tensions_detected,
        })
    }

    /// Check whether validation should run based on budget and tension state.
    ///
    /// Validation is activated when:
    /// - Budget is above 50% remaining (enough room for the check), OR
    /// - The store contains entries with unresolved tensions (always check
    ///   when contradictions may exist).
    pub fn should_validate(
        budget: &crate::types::TokenBudget,
        store: &dyn MemoryStore,
    ) -> anyhow::Result<bool> {
        // Always validate if tensions exist.
        if Self::has_active_tensions(store)? {
            return Ok(true);
        }

        // Otherwise, only validate if we have budget headroom.
        let remaining_fraction = 1.0 - budget.utilization();
        Ok(remaining_fraction > 0.50)
    }

    /// Check whether any alive entry in the store has unresolved tensions.
    fn has_active_tensions(store: &dyn MemoryStore) -> anyhow::Result<bool> {
        let alive_ids = store.list(Some(MemoryState::Alive))?;
        for id in &alive_ids {
            if let Some(entry) = store.load(id)? {
                let has_unresolved = entry.tension_refs.iter().any(|tr| {
                    matches!(
                        tr.role,
                        crate::types::TensionRole::Introduced
                            | crate::types::TensionRole::Referenced
                    )
                });
                if has_unresolved {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Convert a `PatchOutput` into a synthetic `MemoryEntry` for validation.
    ///
    /// The synthetic entry captures the patch's content and file list as
    /// subject headings, allowing the continuity checker to detect conflicts
    /// with existing entries that cover the same files/topics.
    fn patch_to_entry(patch: &PatchOutput) -> MemoryEntry {
        let subject_headings: Vec<String> = patch
            .files_touched
            .iter()
            .flat_map(|f| {
                // Extract meaningful path segments as subject headings.
                f.split('/')
                    .filter(|seg| !seg.is_empty() && *seg != "src" && *seg != "crates")
                    .map(|seg| seg.replace(".rs", "").replace(".ts", ""))
                    .collect::<Vec<_>>()
            })
            .collect();

        let call_number = if let Some(first_file) = patch.files_touched.first() {
            crate::memory::call_number::call_number_from_path(first_file, 4)
        } else {
            CallNumber::parse("UNKNOWN")
        };

        MemoryEntry {
            id: EntryId(format!("patch-{}", hash_content(&patch.patch))),
            agent: AgentId("validator".into()),
            content: patch.commit_msg.clone(),
            created_at: String::new(),
            last_accessed: String::new(),
            classification: Classification {
                subject_headings,
                call_number,
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 1.0,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 1.0,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        }
    }
}

/// Simple content hash for generating entry IDs.
fn hash_content(content: &str) -> String {
    let mut hash: u64 = 0;
    for byte in content.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    format!("{hash:016x}")
}

/// Create a tension from a validation issue for tracking.
pub fn tension_from_issue(issue: &str, entry_id: &EntryId) -> Tension {
    let severity = if issue.contains("[Contradiction]") {
        TensionSeverity::High
    } else if issue.contains("[Warning]") {
        TensionSeverity::Moderate
    } else {
        TensionSeverity::Low
    };

    Tension {
        id: TensionId(format!("val-{}", hash_content(issue))),
        description: issue.to_string(),
        severity,
        introduced_in: entry_id.clone(),
        resolved_in: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_patch() -> PatchOutput {
        PatchOutput {
            patch: "diff --git a/src/auth.rs b/src/auth.rs\n--- a/src/auth.rs\n+++ b/src/auth.rs\n@@ -1,1 +1,1 @@\n+fn authenticate() {}".into(),
            commit_msg: "Add authentication handler".into(),
            files_touched: vec!["src/auth.rs".into()],
            tokens_used: 500,
        }
    }

    fn make_entry(id: &str, content: &str, subjects: &[&str]) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: content.into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: subjects.iter().map(|s| s.to_string()).collect(),
                call_number: CallNumber::parse("TEST.UNIT"),
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 0.9,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.95,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count: 0,
            source_commit: None,
        }
    }

    #[test]
    fn validate_clean_patch() {
        let store = InMemoryStore::new();
        let patch = make_patch();
        let result = Validator::validate(&patch, &store).unwrap();
        assert!(result.passed);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn should_validate_with_budget() {
        let store = InMemoryStore::new();
        let budget = TokenBudget::new(32_000);
        assert!(Validator::should_validate(&budget, &store).unwrap());
    }

    #[test]
    fn should_not_validate_low_budget() {
        let store = InMemoryStore::new();
        let mut budget = TokenBudget::new(10_000);
        budget.used = 6_000; // 60% used, 40% remaining -> below 50% threshold
        assert!(!Validator::should_validate(&budget, &store).unwrap());
    }

    #[test]
    fn should_validate_with_tensions_despite_low_budget() {
        let store = InMemoryStore::new();
        let mut entry = make_entry("e1", "test content", &["auth"]);
        entry.tension_refs.push(TensionRef {
            tension_id: TensionId("t1".into()),
            role: TensionRole::Introduced,
        });
        store.store(&entry).unwrap();

        let mut budget = TokenBudget::new(10_000);
        budget.used = 6_000;
        // Should still validate because tensions exist.
        assert!(Validator::should_validate(&budget, &store).unwrap());
    }

    #[test]
    fn tension_from_issue_severity() {
        let entry_id = EntryId("test".into());
        let t = tension_from_issue("[Contradiction] conflicting auth", &entry_id);
        assert_eq!(t.severity, TensionSeverity::High);

        let t = tension_from_issue("[Warning] possible duplicate", &entry_id);
        assert_eq!(t.severity, TensionSeverity::Moderate);

        let t = tension_from_issue("[Info] related entries", &entry_id);
        assert_eq!(t.severity, TensionSeverity::Low);
    }
}
