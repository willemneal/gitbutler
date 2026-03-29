//! Patch generation with actuarial metadata.
//!
//! Produces INDEX.patch and COMMIT.msg files in the scientific format
//! defined by the LRRC proposal. Every commit message includes a
//! survival estimate for the change itself -- a prediction of how
//! long the change will remain relevant.

use crate::types::{
    DistributionFamily, DistributionParameters, PeerReviewNote, ReviewVerdict, StudyId,
    StudyMetadata, StudyPublication, SurvivalDistribution,
};

/// Builder for constructing a study publication (patch + commit message).
#[derive(Debug, Clone)]
pub struct PublicationBuilder {
    study_id: StudyId,
    /// Conventional commit prefix (e.g., "feat(auth)").
    commit_prefix: String,
    /// One-line summary of the change.
    summary: String,
    /// The unified diff content.
    patch_content: String,
    /// Protocol description (what was done).
    protocol_description: String,
    /// Method description (how it was done).
    method_description: String,
    /// Validation result.
    validation: Option<PeerReviewNote>,
    /// Confidence in correctness [0, 1].
    confidence: f64,
    /// Confidence in performance [0, 1].
    performance_confidence: f64,
    /// Known limitations.
    limitations: Vec<String>,
    /// Survival estimate for the change.
    survival_estimate: Option<SurvivalDistribution>,
}

impl PublicationBuilder {
    /// Create a new publication builder for a study.
    pub fn new(study_id: StudyId, commit_prefix: String, summary: String) -> Self {
        Self {
            study_id,
            commit_prefix,
            summary,
            patch_content: String::new(),
            protocol_description: String::new(),
            method_description: String::new(),
            validation: None,
            confidence: 0.0,
            performance_confidence: 0.0,
            limitations: Vec::new(),
            survival_estimate: None,
        }
    }

    /// Set the patch content (unified diff).
    pub fn patch(mut self, content: String) -> Self {
        self.patch_content = content;
        self
    }

    /// Set the protocol description.
    pub fn protocol(mut self, description: String) -> Self {
        self.protocol_description = description;
        self
    }

    /// Set the method description.
    pub fn method(mut self, description: String) -> Self {
        self.method_description = description;
        self
    }

    /// Set the validation (peer review) result.
    pub fn validation(mut self, note: PeerReviewNote) -> Self {
        self.validation = Some(note);
        self
    }

    /// Set the confidence levels.
    pub fn confidence(mut self, correctness: f64, performance: f64) -> Self {
        self.confidence = correctness;
        self.performance_confidence = performance;
        self
    }

    /// Add a known limitation.
    pub fn limitation(mut self, limitation: String) -> Self {
        self.limitations.push(limitation);
        self
    }

    /// Set the survival estimate for the change.
    pub fn survival_estimate(mut self, dist: SurvivalDistribution) -> Self {
        self.survival_estimate = Some(dist);
        self
    }

    /// Build the final publication.
    pub fn build(self) -> StudyPublication {
        let survival = self.survival_estimate.unwrap_or_else(default_survival_estimate);
        let commit_message = format_commit_message(
            &self.commit_prefix,
            &self.summary,
            &self.study_id,
            &self.protocol_description,
            &self.method_description,
            &self.validation,
            self.confidence,
            self.performance_confidence,
            &self.limitations,
            &survival,
        );

        StudyPublication {
            patch: self.patch_content,
            commit_message,
            metadata: StudyMetadata {
                study_id: self.study_id,
                protocol_mode: crate::types::ProtocolMode::Full,
                survival_estimate: survival,
                confidence: self.confidence,
                known_limitations: self.limitations,
                validated_by: self.validation.map(|v| v.reviewer),
            },
        }
    }
}

/// Format a commit message in the LRRC scientific format.
fn format_commit_message(
    prefix: &str,
    summary: &str,
    study_id: &StudyId,
    protocol: &str,
    method: &str,
    validation: &Option<PeerReviewNote>,
    confidence: f64,
    perf_confidence: f64,
    limitations: &[String],
    survival: &SurvivalDistribution,
) -> String {
    let mut msg = format!("{prefix}: {summary}\n\n");
    msg.push_str(&format!("Study: {}\n", study_id.0));

    if !protocol.is_empty() {
        msg.push_str(&format!("Protocol: {protocol}\n"));
    }
    if !method.is_empty() {
        msg.push_str(&format!("Method: {method}\n"));
    }

    // Validation line.
    if let Some(review) = validation {
        let verdict_str = match review.verdict {
            ReviewVerdict::Pass => "PASS",
            ReviewVerdict::MinorRevisions => "MINOR REVISIONS",
            ReviewVerdict::MajorRevisions => "MAJOR REVISIONS",
            ReviewVerdict::Reject => "REJECT",
        };
        msg.push_str(&format!(
            "Validation: {} practitioner review ({})\n",
            review.reviewer.0, verdict_str
        ));
    }

    // Confidence.
    msg.push_str(&format!(
        "Confidence: {confidence:.2} (correctness); {perf_confidence:.2} (performance)\n"
    ));

    // Known limitations.
    if !limitations.is_empty() {
        msg.push_str(&format!("Known limitations: {}\n", limitations.join("; ")));
    }

    // Survival estimate.
    msg.push_str(&format!(
        "Survival estimate: {}\n",
        format_survival_estimate(survival)
    ));

    msg
}

/// Format a survival distribution as a human-readable string.
///
/// Example: "Weibull(k=1.8, lambda=180d)"
fn format_survival_estimate(dist: &SurvivalDistribution) -> String {
    match &dist.parameters {
        DistributionParameters::Exponential { lambda } => {
            format!("Exponential(lambda={lambda:.3}/d)")
        }
        DistributionParameters::Weibull { k, lambda } => {
            format!("Weibull(k={k:.1}, lambda={lambda:.0}d)")
        }
        DistributionParameters::Bathtub {
            alpha,
            beta,
            gamma,
            baseline,
        } => {
            format!("Bathtub(alpha={alpha:.3}, beta={beta:.4}, gamma={gamma:.2}, baseline={baseline:.4})")
        }
        DistributionParameters::LogNormal { mu, sigma } => {
            format!("LogNormal(mu={mu:.2}, sigma={sigma:.2})")
        }
    }
}

/// Generate a default survival estimate for a change when none is provided.
///
/// Defaults to Weibull(k=1.5, lambda=120d) -- a moderate architectural change.
fn default_survival_estimate() -> SurvivalDistribution {
    SurvivalDistribution {
        family: DistributionFamily::Weibull,
        parameters: DistributionParameters::Weibull {
            k: 1.5,
            lambda: 120.0,
        },
        fitted_at: String::new(),
        goodness_of_fit: 0.5,
    }
}

/// Estimate survival distribution for a patch based on heuristics.
///
/// The estimate considers:
/// - Number of files changed (more files = shorter survival)
/// - Lines added/removed (larger changes = shorter survival)
/// - Whether it's a new feature, bug fix, or refactor
pub fn estimate_patch_survival(
    files_changed: usize,
    lines_added: usize,
    lines_removed: usize,
    is_bug_fix: bool,
) -> SurvivalDistribution {
    if is_bug_fix {
        // Bug fixes are exponential -- they become irrelevant once the fix lands.
        return SurvivalDistribution {
            family: DistributionFamily::Exponential,
            parameters: DistributionParameters::Exponential {
                lambda: 1.0 / 5.0, // ~5 day median.
            },
            fitted_at: String::new(),
            goodness_of_fit: 0.6,
        };
    }

    // For non-bug changes, use Weibull with parameters scaled by change size.
    let total_lines = lines_added + lines_removed;
    let complexity_factor = 1.0 + (files_changed as f64).ln().max(0.0);

    // Larger, more complex changes tend to need modification sooner.
    let lambda = (200.0 / complexity_factor).max(30.0);

    // Shape parameter: larger changes have slightly higher k (more rapid aging).
    let k = 1.5 + 0.1 * (total_lines as f64 / 100.0).min(1.0);

    SurvivalDistribution {
        family: DistributionFamily::Weibull,
        parameters: DistributionParameters::Weibull { k, lambda },
        fitted_at: String::new(),
        goodness_of_fit: 0.5,
    }
}

/// Parse a patch to count files and lines.
pub fn analyze_patch(patch_content: &str) -> PatchAnalysis {
    let mut files_changed = 0;
    let mut lines_added = 0;
    let mut lines_removed = 0;

    for line in patch_content.lines() {
        if line.starts_with("diff --git") || line.starts_with("--- ") && line.len() > 4 {
            // Only count "diff --git" as new file indicators.
            if line.starts_with("diff --git") {
                files_changed += 1;
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            lines_added += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            lines_removed += 1;
        }
    }

    PatchAnalysis {
        files_changed,
        lines_added,
        lines_removed,
    }
}

/// Summary statistics for a patch.
#[derive(Debug, Clone)]
pub struct PatchAnalysis {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

impl PatchAnalysis {
    /// Total lines changed (added + removed).
    pub fn total_lines(&self) -> usize {
        self.lines_added + self.lines_removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_builder_produces_valid_output() {
        let pub_result = PublicationBuilder::new(
            StudyId("LRRC-2026-042".into()),
            "feat(auth)".into(),
            "implement session token validation".into(),
        )
        .patch("--- a/auth.rs\n+++ b/auth.rs\n+fn validate() {}".into())
        .protocol("Add validation layer".into())
        .method("Middleware insertion".into())
        .confidence(0.90, 0.75)
        .limitation("Timeout handling deferred".into())
        .build();

        assert!(pub_result.commit_message.contains("LRRC-2026-042"));
        assert!(pub_result.commit_message.contains("feat(auth)"));
        assert!(pub_result.commit_message.contains("Survival estimate:"));
    }

    #[test]
    fn bug_fix_gets_exponential_survival() {
        let dist = estimate_patch_survival(1, 5, 3, true);
        assert_eq!(dist.family, DistributionFamily::Exponential);
    }

    #[test]
    fn patch_analysis_counts_correctly() {
        let patch = "\
diff --git a/foo.rs b/foo.rs
--- a/foo.rs
+++ b/foo.rs
-old line
+new line
+another new line
diff --git a/bar.rs b/bar.rs
--- a/bar.rs
+++ b/bar.rs
-removed
";
        let analysis = analyze_patch(patch);
        assert_eq!(analysis.files_changed, 2);
        assert_eq!(analysis.lines_added, 2);
        assert_eq!(analysis.lines_removed, 2);
    }
}
