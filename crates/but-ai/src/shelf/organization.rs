//! Module and file organization assessment.
//!
//! Shelver has an intuitive sense for when a codebase's organizational
//! scheme is breaking down — when a module has grown too large, when a
//! classification boundary is blurring, when a new concept has emerged
//! that doesn't fit the existing categories.
//!
//! This module provides the analytical tools for those assessments.

use crate::catalog::call_number::call_number_from_path;
use crate::types::CallNumber;
use std::collections::HashMap;

/// A structural observation about codebase organization.
#[derive(Debug, Clone)]
pub struct StructuralObservation {
    /// The type of observation.
    pub kind: ObservationKind,
    /// Human-readable description.
    pub description: String,
    /// Severity: 0.0 (informational) to 1.0 (critical).
    pub severity: f64,
    /// Affected paths.
    pub affected_paths: Vec<String>,
    /// Suggested call number for cataloging this observation.
    pub suggested_call_number: CallNumber,
}

/// Types of structural observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationKind {
    /// A module has grown beyond a reasonable size.
    ModuleTooLarge,
    /// A classification boundary is blurring (files mixing concerns).
    BlurringBoundary,
    /// A new concept has emerged that doesn't fit existing categories.
    EmergentConcept,
    /// Orphaned code: files that don't fit any classification well.
    OrphanedCode,
    /// Consistent and well-organized module.
    WellOrganized,
}

/// Analyze a set of file paths for structural observations.
///
/// This is Shelver's "shelf read" — verifying that the workspace
/// state matches the catalog's expectations.
pub fn analyze_structure(
    paths: &[String],
    max_call_number_depth: usize,
    large_module_threshold: usize,
) -> Vec<StructuralObservation> {
    let mut observations = Vec::new();

    // Group paths by their top-level call number category.
    let mut by_category: HashMap<String, Vec<String>> = HashMap::new();
    for path in paths {
        let cn = call_number_from_path(path, max_call_number_depth);
        let category = cn
            .top_level()
            .unwrap_or("UNCATEGORIZED")
            .to_string();
        by_category.entry(category).or_default().push(path.clone());
    }

    // Check for oversized modules.
    for (category, cat_paths) in &by_category {
        if cat_paths.len() > large_module_threshold {
            observations.push(StructuralObservation {
                kind: ObservationKind::ModuleTooLarge,
                description: format!(
                    "Category '{}' contains {} files (threshold: {}). Consider splitting into sub-categories.",
                    category,
                    cat_paths.len(),
                    large_module_threshold
                ),
                severity: (cat_paths.len() as f64 / (large_module_threshold as f64 * 2.0)).min(1.0),
                affected_paths: cat_paths.clone(),
                suggested_call_number: CallNumber::parse(&format!("ARCH.STRUCTURE.{}", category)),
            });
        }
    }

    // Check for blurring boundaries: paths that share a parent directory
    // but map to different top-level categories.
    let mut by_parent_dir: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for path in paths {
        let cn = call_number_from_path(path, max_call_number_depth);
        let parent = path
            .rsplit_once('/')
            .map(|(p, _)| p.to_string())
            .unwrap_or_default();
        let category = cn.top_level().unwrap_or("UNCATEGORIZED").to_string();
        by_parent_dir
            .entry(parent)
            .or_default()
            .push((path.clone(), category));
    }

    for (parent, entries) in &by_parent_dir {
        let categories: std::collections::HashSet<&str> =
            entries.iter().map(|(_, cat)| cat.as_str()).collect();
        if categories.len() > 2 && entries.len() > 3 {
            observations.push(StructuralObservation {
                kind: ObservationKind::BlurringBoundary,
                description: format!(
                    "Directory '{}' contains files from {} different categories: {}. This may indicate a blurring classification boundary.",
                    parent,
                    categories.len(),
                    categories.into_iter().collect::<Vec<_>>().join(", ")
                ),
                severity: 0.5,
                affected_paths: entries.iter().map(|(p, _)| p.clone()).collect(),
                suggested_call_number: CallNumber::parse("ARCH.STRUCTURE.BOUNDARY"),
            });
        }
    }

    // If no issues found, note the well-organized state.
    if observations.is_empty() && !paths.is_empty() {
        observations.push(StructuralObservation {
            kind: ObservationKind::WellOrganized,
            description: format!(
                "Codebase structure is consistent across {} categories and {} files.",
                by_category.len(),
                paths.len()
            ),
            severity: 0.0,
            affected_paths: Vec::new(),
            suggested_call_number: CallNumber::parse("ARCH.STRUCTURE.HEALTH"),
        });
    }

    observations
}

/// Determine if a new file path fits the existing organizational pattern.
///
/// Returns a fitness score (0.0 to 1.0) and a recommendation.
pub fn assess_fit(
    proposed_path: &str,
    existing_paths: &[String],
    max_depth: usize,
) -> FitAssessment {
    let proposed_cn = call_number_from_path(proposed_path, max_depth);

    // Find how many existing paths share segments with the proposed path.
    let mut max_shared = 0;
    let mut best_match = None;

    for path in existing_paths {
        let existing_cn = call_number_from_path(path, max_depth);
        let shared = proposed_cn.shared_depth(&existing_cn);
        if shared > max_shared {
            max_shared = shared;
            best_match = Some(path.clone());
        }
    }

    let score = if proposed_cn.depth() == 0 {
        0.0
    } else {
        max_shared as f64 / proposed_cn.depth() as f64
    };

    let recommendation = if score > 0.7 {
        "fits well with existing structure".to_string()
    } else if score > 0.4 {
        "partially matches existing patterns; consider aligning naming".to_string()
    } else {
        "does not match existing patterns; may be introducing a new concept".to_string()
    };

    FitAssessment {
        score,
        best_match,
        recommendation,
        proposed_call_number: proposed_cn,
    }
}

/// Result of assessing how well a proposed path fits the existing structure.
#[derive(Debug, Clone)]
pub struct FitAssessment {
    /// Fitness score (0.0 to 1.0).
    pub score: f64,
    /// The closest matching existing path, if any.
    pub best_match: Option<String>,
    /// Human-readable recommendation.
    pub recommendation: String,
    /// Call number derived from the proposed path.
    pub proposed_call_number: CallNumber,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_large_module() {
        let paths: Vec<String> = (0..20)
            .map(|i| format!("crates/auth/src/handler_{}.rs", i))
            .collect();

        let observations = analyze_structure(&paths, 5, 10);
        assert!(observations
            .iter()
            .any(|o| o.kind == ObservationKind::ModuleTooLarge));
    }

    #[test]
    fn well_organized_detection() {
        let paths = vec![
            "crates/auth/src/middleware.rs".to_string(),
            "crates/auth/src/handler.rs".to_string(),
        ];

        let observations = analyze_structure(&paths, 5, 10);
        assert!(observations
            .iter()
            .any(|o| o.kind == ObservationKind::WellOrganized));
    }

    #[test]
    fn fit_assessment() {
        let existing = vec![
            "crates/auth/src/middleware.rs".to_string(),
            "crates/auth/src/handler.rs".to_string(),
        ];

        let result = assess_fit("crates/auth/src/validator.rs", &existing, 5);
        assert!(result.score > 0.5);
    }
}
