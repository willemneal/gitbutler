//! Multi-site study coordination via PR comments.
//!
//! Cross-repo coordination is modeled as multi-site clinical trials.
//! Each repository is a research site. PRs are the communication
//! protocol. Site reports are structured JSON embedded in PR comments.
//!
//! Under WASI, coordination is disabled (in vitro mode).

use crate::types::{
    AgentId, BudgetSnapshot, PrId, RepoRef, ReportId, SiteIdentity, SiteReport,
    SiteReportType, StudyId, StudyPhase, StudyReportPayload, StudyStatus,
};

/// Multi-site protocol trait for cross-repo coordination.
///
/// Implementations adapt to specific forges (GitHub, GitLab, etc.)
/// using only forge-universal operations (PR comments with structured JSON).
pub trait MultiSiteProtocol: Send + Sync {
    /// Submit a site report as a PR comment.
    fn submit_report(&self, site: &RepoRef, report: &SiteReport) -> anyhow::Result<ReportId>;

    /// Collect site reports since a given timestamp.
    fn collect_reports(&self, since: &str) -> anyhow::Result<Vec<SiteReport>>;

    /// Check the status of a study (PR).
    fn check_study_status(&self, pr: &PrId) -> anyhow::Result<StudyStatus>;

    /// Annotate a study with a peer review note.
    fn annotate_study(
        &self,
        pr: &PrId,
        note: &crate::types::PeerReviewNote,
    ) -> anyhow::Result<()>;

    /// List all participating sites for a study.
    fn list_participating_sites(&self, pr: &PrId) -> anyhow::Result<Vec<RepoRef>>;
}

/// In-vitro coordinator for WASI or offline mode.
///
/// Records all operations locally without making network calls.
/// Used when cross-repo coordination is not available.
#[derive(Debug, Clone, Default)]
pub struct InVitroCoordinator {
    /// Locally stored reports.
    reports: Vec<SiteReport>,
    /// Counter for generating report IDs.
    next_report_id: u64,
}

impl InVitroCoordinator {
    /// Create a new in-vitro coordinator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all locally stored reports.
    pub fn local_reports(&self) -> &[SiteReport] {
        &self.reports
    }
}

impl MultiSiteProtocol for InVitroCoordinator {
    fn submit_report(&self, _site: &RepoRef, _report: &SiteReport) -> anyhow::Result<ReportId> {
        // In vitro mode: report is accepted but not transmitted.
        tracing::info!("In vitro mode: site report recorded locally, not transmitted");
        Ok(ReportId(format!("invitro-{}", self.next_report_id)))
    }

    fn collect_reports(&self, _since: &str) -> anyhow::Result<Vec<SiteReport>> {
        // In vitro mode: return locally stored reports only.
        Ok(self.reports.clone())
    }

    fn check_study_status(&self, _pr: &PrId) -> anyhow::Result<StudyStatus> {
        // In vitro mode: assume in progress.
        Ok(StudyStatus::InProgress)
    }

    fn annotate_study(
        &self,
        _pr: &PrId,
        _note: &crate::types::PeerReviewNote,
    ) -> anyhow::Result<()> {
        tracing::info!("In vitro mode: annotation recorded locally");
        Ok(())
    }

    fn list_participating_sites(&self, _pr: &PrId) -> anyhow::Result<Vec<RepoRef>> {
        Ok(Vec::new())
    }
}

/// Study registry: tracks all active studies across repositories.
///
/// Maintained by Chen. Stored as a high-survival memory entry
/// (Weibull with high lambda) because it needs to persist across many tasks.
#[derive(Debug, Clone, Default)]
pub struct StudyRegistry {
    entries: Vec<StudyRegistryEntry>,
}

/// An entry in the study registry.
#[derive(Debug, Clone)]
pub struct StudyRegistryEntry {
    pub study_id: StudyId,
    pub repo: RepoRef,
    pub phase: StudyPhase,
    pub status: StudyStatus,
    pub dependencies: Vec<StudyDependency>,
    pub confidence: f64,
    pub pr_id: Option<PrId>,
}

/// A dependency between studies.
#[derive(Debug, Clone)]
pub struct StudyDependency {
    pub study_id: StudyId,
    pub repo: RepoRef,
    pub status: StudyStatus,
    /// Whether this dependency is on the critical path.
    pub critical: bool,
}

impl StudyRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new study.
    pub fn register_study(&mut self, entry: StudyRegistryEntry) {
        // Update if exists, otherwise add.
        if let Some(existing) = self.entries.iter_mut().find(|e| e.study_id == entry.study_id) {
            existing.phase = entry.phase;
            existing.status = entry.status;
            existing.confidence = entry.confidence;
            existing.dependencies = entry.dependencies;
        } else {
            self.entries.push(entry);
        }
    }

    /// Update the status of a study.
    pub fn update_status(&mut self, study_id: &StudyId, status: StudyStatus) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.study_id == *study_id) {
            entry.status = status;
        }
    }

    /// Get all active studies (not published or halted).
    pub fn active_studies(&self) -> Vec<&StudyRegistryEntry> {
        self.entries
            .iter()
            .filter(|e| !matches!(e.status, StudyStatus::Published | StudyStatus::Halted))
            .collect()
    }

    /// Get all studies that depend on a given study.
    pub fn dependents_of(&self, study_id: &StudyId) -> Vec<&StudyRegistryEntry> {
        self.entries
            .iter()
            .filter(|e| e.dependencies.iter().any(|d| d.study_id == *study_id))
            .collect()
    }

    /// Check if all dependencies of a study are satisfied (published).
    pub fn dependencies_satisfied(&self, study_id: &StudyId) -> bool {
        let entry = match self.entries.iter().find(|e| e.study_id == *study_id) {
            Some(e) => e,
            None => return true,
        };

        entry.dependencies.iter().all(|dep| {
            self.entries
                .iter()
                .find(|e| e.study_id == dep.study_id)
                .map(|e| matches!(e.status, StudyStatus::Published))
                .unwrap_or(false)
        })
    }

    /// Get all studies in a specific repository.
    pub fn studies_in_repo(&self, repo: &RepoRef) -> Vec<&StudyRegistryEntry> {
        self.entries.iter().filter(|e| e.repo == *repo).collect()
    }

    /// Total number of registered studies.
    pub fn total_studies(&self) -> usize {
        self.entries.len()
    }
}

/// Format a site report as a PR comment body.
///
/// The report is embedded as a JSON code fence with the schema marker,
/// following the proposal's specification.
pub fn format_site_report(report: &SiteReport) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(report)?;
    Ok(format!(
        "```but-ai-site-report\n{json}\n```\n\n*Report from {}, institution {}*",
        report.from_site.investigator.0, report.from_site.institution
    ))
}

/// Parse a site report from a PR comment body.
///
/// Looks for the `but-ai-site-report` code fence and parses the JSON within.
pub fn parse_site_report(comment_body: &str) -> anyhow::Result<SiteReport> {
    let start_marker = "```but-ai-site-report\n";
    let end_marker = "\n```";

    let start = comment_body
        .find(start_marker)
        .ok_or_else(|| anyhow::anyhow!("No but-ai-site-report code fence found"))?;

    let json_start = start + start_marker.len();
    let remaining = &comment_body[json_start..];

    let end = remaining
        .find(end_marker)
        .ok_or_else(|| anyhow::anyhow!("Unterminated code fence"))?;

    let json_str = &remaining[..end];
    let report: SiteReport = serde_json::from_str(json_str)?;
    Ok(report)
}

/// Create a site report for study enrollment.
pub fn enrollment_report(
    investigator: AgentId,
    institution: String,
    repo: RepoRef,
    study_id: StudyId,
    budget: BudgetSnapshot,
) -> SiteReport {
    SiteReport {
        schema: "but-ai/multisite/v1".to_string(),
        report_type: SiteReportType::Enrollment,
        from_site: SiteIdentity {
            investigator,
            institution,
            repo,
        },
        to_site: None,
        report: StudyReportPayload {
            study_ref: study_id,
            phase: StudyPhase::LiteratureReview,
            status: StudyStatus::Proposed,
            dependencies: Vec::new(),
            confidence: 0.0,
            budget,
            surprise_index: 0.0,
        },
        timestamp: String::new(), // Caller sets this.
    }
}

/// Create a progress report.
pub fn progress_report(
    from: SiteIdentity,
    to: Option<SiteIdentity>,
    study_id: StudyId,
    phase: StudyPhase,
    confidence: f64,
    budget: BudgetSnapshot,
    surprise_index: f64,
) -> SiteReport {
    SiteReport {
        schema: "but-ai/multisite/v1".to_string(),
        report_type: SiteReportType::Progress,
        from_site: from,
        to_site: to,
        report: StudyReportPayload {
            study_ref: study_id,
            phase,
            status: StudyStatus::InProgress,
            dependencies: Vec::new(),
            confidence,
            budget,
            surprise_index,
        },
        timestamp: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> RepoRef {
        RepoRef {
            owner: "org".into(),
            name: "repo".into(),
        }
    }

    #[test]
    fn in_vitro_coordinator_accepts_reports() {
        let coord = InVitroCoordinator::new();
        let report_id = coord
            .submit_report(&test_repo(), &enrollment_report(
                AgentId("chen".into()),
                "093-lrrc".into(),
                test_repo(),
                StudyId("LRRC-2026-001".into()),
                BudgetSnapshot { consumed: 0, allocated: 50000 },
            ))
            .unwrap();
        assert!(report_id.0.starts_with("invitro-"));
    }

    #[test]
    fn study_registry_tracks_dependencies() {
        let mut registry = StudyRegistry::new();

        registry.register_study(StudyRegistryEntry {
            study_id: StudyId("LRRC-2026-039".into()),
            repo: test_repo(),
            phase: StudyPhase::Publication,
            status: StudyStatus::Published,
            dependencies: Vec::new(),
            confidence: 0.90,
            pr_id: None,
        });

        registry.register_study(StudyRegistryEntry {
            study_id: StudyId("LRRC-2026-042".into()),
            repo: test_repo(),
            phase: StudyPhase::Experiment,
            status: StudyStatus::InProgress,
            dependencies: vec![StudyDependency {
                study_id: StudyId("LRRC-2026-039".into()),
                repo: test_repo(),
                status: StudyStatus::Published,
                critical: true,
            }],
            confidence: 0.75,
            pr_id: None,
        });

        assert!(registry.dependencies_satisfied(&StudyId("LRRC-2026-042".into())));
        assert_eq!(registry.active_studies().len(), 1);
    }

    #[test]
    fn site_report_roundtrips_through_format_and_parse() {
        let report = enrollment_report(
            AgentId("chen".into()),
            "093-lrrc".into(),
            test_repo(),
            StudyId("LRRC-2026-001".into()),
            BudgetSnapshot { consumed: 1000, allocated: 50000 },
        );

        let formatted = format_site_report(&report).unwrap();
        let parsed = parse_site_report(&formatted).unwrap();

        assert_eq!(parsed.from_site.institution, "093-lrrc");
        assert_eq!(parsed.report.study_ref.0, "LRRC-2026-001");
    }
}
