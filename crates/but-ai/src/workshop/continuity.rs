//! Consistency checking -- Sato's role.
//!
//! The continuity checker catches contradictions. If a function returns a string
//! in one module, it must return a string in every module that calls it. If a
//! variable is named in snake_case in one file, it must be named in snake_case
//! everywhere. Inconsistency is not a style preference; it is a continuity error.

use crate::types::{AgentId, Chapter, TensionEntry, TensionId, TensionSeverity};

/// Classification of continuity findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSeverity {
    /// Blocking: must be fixed before publication.
    Contradiction,
    /// Should fix: inconsistency that will cause confusion.
    Inconsistency,
    /// Informational: something worth noting but not blocking.
    Observation,
}

/// A single finding from a continuity check.
#[derive(Debug, Clone)]
pub struct ContinuityFinding {
    pub severity: FindingSeverity,
    pub description: String,
    /// The chapter that the finding contradicts.
    pub contradicts_chapter: Option<u64>,
    /// Specific location (file:line or similar reference).
    pub location: Option<String>,
}

/// A complete continuity report for a chapter.
#[derive(Debug, Clone)]
pub struct ContinuityReport {
    pub chapter_number: u64,
    pub checked_by: AgentId,
    pub findings: Vec<ContinuityFinding>,
    /// Whether the chapter passes continuity (no contradictions).
    pub passes: bool,
}

impl ContinuityReport {
    /// Count findings by severity.
    pub fn count_by_severity(&self, severity: FindingSeverity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Extract blocking findings (contradictions) as tension entries.
    pub fn blocking_tensions(&self) -> Vec<TensionEntry> {
        self.findings
            .iter()
            .filter(|f| f.severity == FindingSeverity::Contradiction)
            .enumerate()
            .map(|(i, f)| TensionEntry {
                id: TensionId(format!("continuity-{}-{}", self.chapter_number, i)),
                description: f.description.clone(),
                severity: TensionSeverity::High,
                suggested_resolution: Some("Resolve contradiction before publication".to_string()),
            })
            .collect()
    }
}

/// The continuity checker agent. Reviews chapters for consistency with
/// the existing narrative.
pub struct ContinuityChecker {
    agent_id: AgentId,
    /// The "style bible" -- a list of active conventions.
    conventions: Vec<Convention>,
}

/// A recorded convention (from the style bible).
#[derive(Debug, Clone)]
pub struct Convention {
    pub name: String,
    pub description: String,
    /// The chapter where this convention was established.
    pub established_in: u64,
}

impl ContinuityChecker {
    /// Create a new continuity checker.
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            conventions: Vec::new(),
        }
    }

    /// Record a new convention in the style bible.
    pub fn record_convention(&mut self, name: String, description: String, chapter: u64) {
        // If a convention with this name already exists, update it.
        if let Some(existing) = self.conventions.iter_mut().find(|c| c.name == name) {
            existing.description = description;
            existing.established_in = chapter;
        } else {
            self.conventions.push(Convention {
                name,
                description,
                established_in: chapter,
            });
        }
    }

    /// Check a chapter for continuity against existing chapters.
    ///
    /// Produces a report listing contradictions, inconsistencies, and observations.
    pub fn check_continuity(
        &self,
        chapter: &Chapter,
        existing_chapters: &[&Chapter],
    ) -> ContinuityReport {
        let mut findings = Vec::new();

        // Check for file-level contradictions.
        findings.extend(self.check_file_overlap(chapter, existing_chapters));

        // Check for convention violations.
        findings.extend(self.check_conventions(chapter));

        // Check for arc consistency.
        findings.extend(self.check_arc_consistency(chapter, existing_chapters));

        let passes = !findings
            .iter()
            .any(|f| f.severity == FindingSeverity::Contradiction);

        ContinuityReport {
            chapter_number: chapter.chapter_number,
            checked_by: self.agent_id.clone(),
            findings,
            passes,
        }
    }

    /// Get the checker's agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Get all recorded conventions.
    pub fn conventions(&self) -> &[Convention] {
        &self.conventions
    }

    /// Check for files modified in both the new chapter and recent existing chapters.
    fn check_file_overlap(
        &self,
        chapter: &Chapter,
        existing: &[&Chapter],
    ) -> Vec<ContinuityFinding> {
        let mut findings = Vec::new();

        // Only check against chapters in the same arc.
        let same_arc: Vec<&&Chapter> = existing
            .iter()
            .filter(|c| c.arc == chapter.arc)
            .collect();

        for existing_ch in same_arc {
            let overlap: Vec<&String> = chapter
                .characters
                .iter()
                .filter(|f| existing_ch.characters.contains(f))
                .collect();

            if overlap.len() > 3 {
                // Modifying many of the same files as a recent chapter is suspicious.
                findings.push(ContinuityFinding {
                    severity: FindingSeverity::Inconsistency,
                    description: format!(
                        "Chapter {} modifies {} files also modified in chapter {}: potential \
                         conflicting changes",
                        chapter.chapter_number,
                        overlap.len(),
                        existing_ch.chapter_number,
                    ),
                    contradicts_chapter: Some(existing_ch.chapter_number),
                    location: None,
                });
            }
        }

        findings
    }

    /// Check the chapter against the style bible conventions.
    fn check_conventions(&self, chapter: &Chapter) -> Vec<ContinuityFinding> {
        let mut findings = Vec::new();

        // For each convention, check if the chapter's content might violate it.
        // This is a heuristic check based on naming patterns.
        for convention in &self.conventions {
            let convention_lower = convention.name.to_lowercase();

            // If a convention mentions a file that this chapter modifies,
            // flag it as needing manual verification.
            for character in &chapter.characters {
                let char_lower = character.to_lowercase();
                if convention_lower.contains(&char_lower) || char_lower.contains(&convention_lower)
                {
                    findings.push(ContinuityFinding {
                        severity: FindingSeverity::Observation,
                        description: format!(
                            "Chapter {} modifies '{}' which is covered by convention '{}' \
                             (established in ch.{}). Verify compliance.",
                            chapter.chapter_number,
                            character,
                            convention.name,
                            convention.established_in,
                        ),
                        contradicts_chapter: Some(convention.established_in),
                        location: Some(character.clone()),
                    });
                }
            }
        }

        findings
    }

    /// Check arc-level consistency.
    fn check_arc_consistency(
        &self,
        chapter: &Chapter,
        existing: &[&Chapter],
    ) -> Vec<ContinuityFinding> {
        let mut findings = Vec::new();

        let same_arc: Vec<&&Chapter> = existing
            .iter()
            .filter(|c| c.arc == chapter.arc)
            .collect();

        if same_arc.is_empty() {
            return findings;
        }

        // Check for unresolved tensions in the same arc that this chapter
        // does not address.
        let existing_tensions: Vec<&TensionEntry> = same_arc
            .iter()
            .flat_map(|c| c.tensions_introduced.iter())
            .collect();

        let resolved: Vec<&TensionId> = same_arc
            .iter()
            .flat_map(|c| c.tensions_resolved.iter())
            .chain(chapter.tensions_resolved.iter())
            .collect();

        let still_unresolved: Vec<&&TensionEntry> = existing_tensions
            .iter()
            .filter(|t| !resolved.contains(&&t.id))
            .collect();

        if still_unresolved.len() > 3 {
            findings.push(ContinuityFinding {
                severity: FindingSeverity::Observation,
                description: format!(
                    "Arc '{}' has {} unresolved tensions. Consider addressing some \
                     in this chapter.",
                    chapter.arc.0,
                    still_unresolved.len(),
                ),
                contradicts_chapter: None,
                location: None,
            });
        }

        findings
    }
}
