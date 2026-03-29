//! Code placement logic — where does this belong?
//!
//! Shelver's distinctive quality is attention to placement. A function
//! that works but is in the wrong module is like a correctly cataloged
//! book on the wrong shelf — technically present, practically lost.
//!
//! This module determines the best file/module placement for new code
//! by consulting the catalog's call number hierarchy and existing
//! codebase structure.

use crate::catalog::call_number::{call_number_distance, call_number_from_path};
use crate::types::CallNumber;

/// A candidate placement for new code.
#[derive(Debug, Clone)]
pub struct PlacementCandidate {
    /// The file path where the code could be placed.
    pub path: String,
    /// The call number derived from the path.
    pub call_number: CallNumber,
    /// Placement score (0.0 to 1.0, higher is better).
    pub score: f64,
    /// Reason for this placement recommendation.
    pub reason: String,
}

/// Determine the best placement for code classified under the given
/// call number, considering the existing file structure.
///
/// Returns candidates sorted by score (best first).
pub fn find_placement(
    target_call_number: &CallNumber,
    existing_paths: &[String],
    max_call_number_depth: usize,
) -> Vec<PlacementCandidate> {
    let mut candidates: Vec<PlacementCandidate> = existing_paths
        .iter()
        .map(|path| {
            let path_cn = call_number_from_path(path, max_call_number_depth);
            let distance = call_number_distance(target_call_number, &path_cn);
            let max_possible = target_call_number.depth() + path_cn.depth();
            let score = if max_possible == 0 {
                0.0
            } else {
                1.0 - (distance as f64 / max_possible as f64)
            };

            let reason = if distance == 0 {
                format!("exact match: path maps to {}", path_cn)
            } else {
                let shared = target_call_number.shared_depth(&path_cn);
                format!(
                    "shared {} of {} segments with {}",
                    shared,
                    target_call_number.depth(),
                    path_cn
                )
            };

            PlacementCandidate {
                path: path.clone(),
                call_number: path_cn,
                score,
                reason,
            }
        })
        .collect();

    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

/// Check if a new file should be created or if existing files can
/// accommodate the new code.
///
/// Returns `true` if the best placement score is below the threshold,
/// indicating that no existing file is a good fit.
pub fn needs_new_file(candidates: &[PlacementCandidate], threshold: f64) -> bool {
    candidates
        .first()
        .map(|c| c.score < threshold)
        .unwrap_or(true) // No candidates means we definitely need a new file.
}

/// Suggest a file path for new code based on its call number.
///
/// Converts a call number like `ARCH.AUTH.MIDDLEWARE` into a path
/// suggestion like `src/arch/auth/middleware.rs`.
pub fn suggest_path(call_number: &CallNumber, base_dir: &str) -> String {
    let segments: Vec<String> = call_number
        .segments
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    if segments.is_empty() {
        return format!("{}/lib.rs", base_dir);
    }

    if segments.len() == 1 {
        return format!("{}/{}.rs", base_dir, segments[0]);
    }

    // Last segment becomes the file name, rest become directories.
    let dirs = &segments[..segments.len() - 1];
    let file = &segments[segments.len() - 1];
    format!("{}/{}/{}.rs", base_dir, dirs.join("/"), file)
}

/// Assess the organizational health of a module based on its files.
///
/// Returns a score from 0.0 (poorly organized) to 1.0 (well organized)
/// based on naming consistency, depth consistency, and file count.
pub fn assess_organization(paths: &[String], max_depth: usize) -> OrganizationAssessment {
    if paths.is_empty() {
        return OrganizationAssessment {
            score: 1.0,
            file_count: 0,
            avg_depth: 0.0,
            depth_variance: 0.0,
            issues: Vec::new(),
        };
    }

    let call_numbers: Vec<CallNumber> = paths
        .iter()
        .map(|p| call_number_from_path(p, max_depth))
        .collect();

    let file_count = paths.len();

    // Compute average depth.
    let total_depth: usize = call_numbers.iter().map(|cn| cn.depth()).sum();
    let avg_depth = total_depth as f64 / file_count as f64;

    // Compute depth variance.
    let depth_variance = call_numbers
        .iter()
        .map(|cn| {
            let diff = cn.depth() as f64 - avg_depth;
            diff * diff
        })
        .sum::<f64>()
        / file_count as f64;

    let mut issues = Vec::new();
    let mut score: f64 = 1.0;

    // High depth variance suggests inconsistent organization.
    if depth_variance > 2.0 {
        score -= 0.2;
        issues.push("inconsistent nesting depth across files".to_string());
    }

    // Too many files at the same level may indicate a flat, unstructured module.
    if file_count > 10 {
        let top_levels: std::collections::HashSet<_> = call_numbers
            .iter()
            .filter_map(|cn| cn.top_level().map(|s| s.to_string()))
            .collect();
        if top_levels.len() <= 2 {
            score -= 0.15;
            issues.push("many files under few top-level categories".to_string());
        }
    }

    // Files with very deep paths may indicate over-nesting.
    let deep_files = call_numbers.iter().filter(|cn| cn.depth() > max_depth).count();
    if deep_files > 0 {
        score -= 0.1;
        issues.push(format!(
            "{} files exceed recommended nesting depth of {}",
            deep_files, max_depth
        ));
    }

    OrganizationAssessment {
        score: score.clamp(0.0, 1.0),
        file_count,
        avg_depth,
        depth_variance,
        issues,
    }
}

/// Assessment of a module's organizational health.
#[derive(Debug, Clone)]
pub struct OrganizationAssessment {
    /// Overall organization score (0.0 to 1.0).
    pub score: f64,
    /// Number of files assessed.
    pub file_count: usize,
    /// Average nesting depth.
    pub avg_depth: f64,
    /// Variance in nesting depth.
    pub depth_variance: f64,
    /// Specific organizational issues found.
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_ranking() {
        let target = CallNumber::parse("ARCH.AUTH.MIDDLEWARE");
        let paths = vec![
            "crates/auth/src/middleware.rs".to_string(),
            "crates/api/src/routes.rs".to_string(),
            "crates/db/src/models.rs".to_string(),
        ];

        let candidates = find_placement(&target, &paths, 5);
        // The auth/middleware path should score highest.
        assert!(!candidates.is_empty());
        assert!(candidates[0].path.contains("auth"));
    }

    #[test]
    fn suggest_path_single_segment() {
        let cn = CallNumber::parse("ARCH");
        assert_eq!(suggest_path(&cn, "src"), "src/arch.rs");
    }

    #[test]
    fn suggest_path_multi_segment() {
        let cn = CallNumber::parse("ARCH.AUTH.MIDDLEWARE");
        assert_eq!(suggest_path(&cn, "src"), "src/arch/auth/middleware.rs");
    }

    #[test]
    fn needs_new_file_below_threshold() {
        let candidates = vec![PlacementCandidate {
            path: "src/unrelated.rs".into(),
            call_number: CallNumber::parse("TOOL.BUILD"),
            score: 0.1,
            reason: "low match".into(),
        }];
        assert!(needs_new_file(&candidates, 0.3));
    }

    #[test]
    fn organization_assessment_empty() {
        let result = assess_organization(&[], 5);
        assert_eq!(result.score, 1.0);
        assert_eq!(result.file_count, 0);
    }
}
