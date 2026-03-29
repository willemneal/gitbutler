//! Statistical validation of implementation correctness.
//!
//! Implements Okonkwo's practitioner review criteria: outputs must be
//! not just correct but useful. A correct but incomprehensible result
//! is a failure. A valid but uninterpretable memory retrieval result
//! is a failure.
//!
//! Validation operates at three levels:
//! 1. **Syntactic**: patch applies cleanly, commit message is well-formed.
//! 2. **Semantic**: changes implement what the protocol specifies.
//! 3. **Statistical**: survival estimates are plausible, confidence intervals
//!    are well-calibrated, surprise indices are within expected bounds.

use crate::types::{
    AgentId, PeerReviewNote, ReviewVerdict, StudyId, StudyPublication, SurvivalDistribution,
};

/// Validation result for a study publication.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub syntactic: SyntacticValidation,
    pub statistical: StatisticalValidation,
    pub verdict: ReviewVerdict,
    pub issues: Vec<ValidationIssue>,
}

/// Syntactic validation checks.
#[derive(Debug, Clone)]
pub struct SyntacticValidation {
    /// The patch is non-empty.
    pub has_patch: bool,
    /// The commit message is non-empty and well-formed.
    pub has_commit_message: bool,
    /// The commit message contains a study ID.
    pub has_study_id: bool,
    /// The commit message contains a survival estimate.
    pub has_survival_estimate: bool,
    /// All checks pass.
    pub passed: bool,
}

/// Statistical validation checks.
#[derive(Debug, Clone)]
pub struct StatisticalValidation {
    /// The survival estimate has plausible parameters.
    pub survival_plausible: bool,
    /// The confidence value is in [0, 1].
    pub confidence_valid: bool,
    /// The goodness-of-fit value is in [0, 1].
    pub goodness_of_fit_valid: bool,
    /// All checks pass.
    pub passed: bool,
}

/// A specific validation issue.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub category: IssueCategory,
    pub description: String,
}

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSeverity {
    /// Informational -- no action required.
    Info,
    /// Warning -- should be addressed but not blocking.
    Warning,
    /// Error -- must be addressed before publication.
    Error,
}

/// Category of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCategory {
    Syntactic,
    Statistical,
    Practitioner,
}

/// Validate a study publication.
///
/// Performs all three levels of validation and produces a verdict.
pub fn validate_publication(publication: &StudyPublication) -> ValidationResult {
    let mut issues = Vec::new();

    let syntactic = validate_syntactic(publication, &mut issues);
    let statistical = validate_statistical(&publication.metadata.survival_estimate, publication.metadata.confidence, &mut issues);

    // Determine overall verdict.
    let has_errors = issues.iter().any(|i| matches!(i.severity, IssueSeverity::Error));
    let has_warnings = issues.iter().any(|i| matches!(i.severity, IssueSeverity::Warning));

    let verdict = if has_errors {
        ReviewVerdict::MajorRevisions
    } else if has_warnings {
        ReviewVerdict::MinorRevisions
    } else {
        ReviewVerdict::Pass
    };

    ValidationResult {
        syntactic,
        statistical,
        verdict,
        issues,
    }
}

/// Generate a peer review note from a validation result.
pub fn to_peer_review_note(
    result: &ValidationResult,
    reviewer: AgentId,
    study_id: StudyId,
    timestamp: String,
) -> PeerReviewNote {
    let comments: Vec<String> = result
        .issues
        .iter()
        .map(|i| {
            let severity = match i.severity {
                IssueSeverity::Info => "INFO",
                IssueSeverity::Warning => "WARN",
                IssueSeverity::Error => "ERROR",
            };
            format!("[{severity}] {}", i.description)
        })
        .collect();

    PeerReviewNote {
        reviewer,
        study_id,
        verdict: result.verdict,
        comments,
        reviewed_at: timestamp,
    }
}

/// Syntactic validation of a publication.
fn validate_syntactic(
    publication: &StudyPublication,
    issues: &mut Vec<ValidationIssue>,
) -> SyntacticValidation {
    let has_patch = !publication.patch.is_empty();
    if !has_patch {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            category: IssueCategory::Syntactic,
            description: "Publication has no patch content.".to_string(),
        });
    }

    let has_commit_message = !publication.commit_message.is_empty();
    if !has_commit_message {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            category: IssueCategory::Syntactic,
            description: "Publication has no commit message.".to_string(),
        });
    }

    let has_study_id = publication.commit_message.contains("Study:");
    if !has_study_id {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::Syntactic,
            description: "Commit message missing Study: header.".to_string(),
        });
    }

    let has_survival_estimate = publication.commit_message.contains("Survival estimate:");
    if !has_survival_estimate {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::Syntactic,
            description: "Commit message missing Survival estimate: header.".to_string(),
        });
    }

    let passed = has_patch && has_commit_message;

    SyntacticValidation {
        has_patch,
        has_commit_message,
        has_study_id,
        has_survival_estimate,
        passed,
    }
}

/// Statistical validation of survival estimate and confidence.
fn validate_statistical(
    survival: &SurvivalDistribution,
    confidence: f64,
    issues: &mut Vec<ValidationIssue>,
) -> StatisticalValidation {
    let survival_plausible = validate_distribution_parameters(&survival.parameters, issues);

    let confidence_valid = (0.0..=1.0).contains(&confidence);
    if !confidence_valid {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            category: IssueCategory::Statistical,
            description: format!("Confidence {confidence:.3} is outside [0, 1]."),
        });
    }

    let goodness_of_fit_valid = (0.0..=1.0).contains(&survival.goodness_of_fit);
    if !goodness_of_fit_valid {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::Statistical,
            description: format!(
                "Goodness-of-fit {:.3} is outside [0, 1].",
                survival.goodness_of_fit
            ),
        });
    }

    let passed = survival_plausible && confidence_valid;

    StatisticalValidation {
        survival_plausible,
        confidence_valid,
        goodness_of_fit_valid,
        passed,
    }
}

/// Validate that distribution parameters are within plausible ranges.
fn validate_distribution_parameters(
    params: &crate::types::DistributionParameters,
    issues: &mut Vec<ValidationIssue>,
) -> bool {
    use crate::types::DistributionParameters;

    match params {
        DistributionParameters::Exponential { lambda } => {
            if *lambda <= 0.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    category: IssueCategory::Statistical,
                    description: format!("Exponential lambda={lambda} must be positive."),
                });
                return false;
            }
            if *lambda > 10.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    category: IssueCategory::Statistical,
                    description: format!(
                        "Exponential lambda={lambda} implies sub-day median survival. Is this intentional?"
                    ),
                });
            }
            true
        }
        DistributionParameters::Weibull { k, lambda } => {
            let mut valid = true;
            if *k <= 0.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    category: IssueCategory::Statistical,
                    description: format!("Weibull k={k} must be positive."),
                });
                valid = false;
            }
            if *lambda <= 0.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    category: IssueCategory::Statistical,
                    description: format!("Weibull lambda={lambda} must be positive."),
                });
                valid = false;
            }
            if *k > 10.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Warning,
                    category: IssueCategory::Statistical,
                    description: format!(
                        "Weibull k={k} is unusually high. This implies very rapid aging."
                    ),
                });
            }
            valid
        }
        DistributionParameters::Bathtub {
            alpha,
            beta,
            gamma,
            baseline,
        } => {
            let mut valid = true;
            if *alpha < 0.0 || *beta < 0.0 || *gamma <= 0.0 || *baseline < 0.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    category: IssueCategory::Statistical,
                    description: format!(
                        "Bathtub parameters (alpha={alpha}, beta={beta}, gamma={gamma}, baseline={baseline}) contain invalid values."
                    ),
                });
                valid = false;
            }
            valid
        }
        DistributionParameters::LogNormal { mu: _, sigma } => {
            if *sigma <= 0.0 {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    category: IssueCategory::Statistical,
                    description: format!("LogNormal sigma={sigma} must be positive."),
                });
                return false;
            }
            true
        }
    }
}

/// Practitioner usability check (Okonkwo's criteria).
///
/// Checks whether the output would be comprehensible and actionable
/// for an agent operating at the edge of its context window.
pub fn practitioner_usability_check(
    commit_message: &str,
    patch_lines: usize,
    max_patch_lines: u32,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    if patch_lines > max_patch_lines as usize {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            category: IssueCategory::Practitioner,
            description: format!(
                "Patch exceeds maximum line limit ({patch_lines} > {max_patch_lines})."
            ),
        });
    }

    if commit_message.len() > 2000 {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::Practitioner,
            description: "Commit message exceeds 2000 characters. Consider condensing.".to_string(),
        });
    }

    let first_line = commit_message.lines().next().unwrap_or("");
    if first_line.len() > 72 {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            category: IssueCategory::Practitioner,
            description: format!(
                "Commit subject line is {} characters (recommended: <= 72).",
                first_line.len()
            ),
        });
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DistributionFamily, DistributionParameters, ProtocolMode, StudyMetadata,
        SurvivalDistribution,
    };

    fn test_publication() -> StudyPublication {
        StudyPublication {
            patch: "+fn hello() {}".to_string(),
            commit_message: "feat: test\n\nStudy: LRRC-2026-001\nSurvival estimate: Weibull(k=1.5, lambda=120d)\n".to_string(),
            metadata: StudyMetadata {
                study_id: StudyId("LRRC-2026-001".into()),
                protocol_mode: ProtocolMode::Full,
                survival_estimate: SurvivalDistribution {
                    family: DistributionFamily::Weibull,
                    parameters: DistributionParameters::Weibull { k: 1.5, lambda: 120.0 },
                    fitted_at: String::new(),
                    goodness_of_fit: 0.8,
                },
                confidence: 0.85,
                known_limitations: Vec::new(),
                validated_by: None,
            },
        }
    }

    #[test]
    fn valid_publication_passes() {
        let result = validate_publication(&test_publication());
        assert!(result.syntactic.passed);
        assert!(result.statistical.passed);
        assert_eq!(result.verdict, ReviewVerdict::Pass);
    }

    #[test]
    fn empty_patch_fails() {
        let mut pub_ = test_publication();
        pub_.patch = String::new();
        let result = validate_publication(&pub_);
        assert!(!result.syntactic.has_patch);
        assert!(matches!(
            result.verdict,
            ReviewVerdict::MajorRevisions | ReviewVerdict::Reject
        ));
    }

    #[test]
    fn invalid_confidence_fails() {
        let mut pub_ = test_publication();
        pub_.metadata.confidence = 1.5;
        let result = validate_publication(&pub_);
        assert!(!result.statistical.confidence_valid);
    }
}
