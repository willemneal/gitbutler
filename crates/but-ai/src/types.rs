//! Shared types for the actuarial-table memory architecture.
//!
//! Every type in this module is drawn from insurance actuarial science and
//! survival analysis. Memory entries have fitted survival functions, agents
//! carry statistical credentials, and tasks are modeled as clinical studies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for an agent in the research centre.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AgentId(pub String);

/// Unique identifier for a study (task).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct StudyId(pub String);

/// Unique identifier for a memory entry in the actuarial table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MemoryId(pub String);

/// Unique identifier for a site report (coordination message).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ReportId(pub String);

/// Reference to a repository (a research site).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

/// Reference to a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PrId {
    pub repo: RepoRef,
    pub number: u64,
}

// ---------------------------------------------------------------------------
// Memory classification
// ---------------------------------------------------------------------------

/// The type of a memory entry, which determines its default survival distribution.
///
/// Different memory types exhibit fundamentally different mortality patterns.
/// Architectural knowledge decays slowly (Weibull), bug knowledge expires
/// abruptly (exponential), and conventions follow a bathtub curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// Structural knowledge about the codebase. Weibull mortality.
    Architectural,
    /// Bug-related knowledge. Exponential mortality -- once fixed, irrelevant.
    BugFix,
    /// Coding conventions and team norms. Bathtub mortality.
    Convention,
    /// Dependency knowledge. Weibull mortality with medium scale.
    Dependency,
    /// Ephemeral task context. Exponential with short half-life.
    TaskContext,
    /// Cross-repo coordination knowledge. Log-normal (heavy-tailed).
    CrossRepo,
}

/// Lifecycle state of a memory entry in the actuarial table.
///
/// Mirrors the insurance concept of policy status: in-force, lapsed, terminated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// S(t) >= alive_threshold. The memory is considered relevant.
    Alive,
    /// S(t) < alive_threshold but >= deceased_threshold. Under review.
    Moribund,
    /// S(t) < deceased_threshold. Formally expired but archived.
    Deceased,
}

// ---------------------------------------------------------------------------
// Survival distributions
// ---------------------------------------------------------------------------

/// Family of parametric survival distributions supported by the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DistributionFamily {
    /// Exponential: constant hazard rate. Memoryless.
    Exponential,
    /// Weibull: monotone hazard (increasing or decreasing depending on shape).
    Weibull,
    /// Bathtub: high-low-high hazard curve (mixture model).
    Bathtub,
    /// Log-normal: heavy-tailed, for cross-repo knowledge.
    LogNormal,
}

/// Parameters for a fitted survival distribution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SurvivalDistribution {
    pub family: DistributionFamily,
    pub parameters: DistributionParameters,
    /// ISO 8601 timestamp of when the distribution was last fitted.
    pub fitted_at: String,
    /// Goodness-of-fit score in [0, 1]. Higher is better.
    pub goodness_of_fit: f64,
}

/// Concrete parameter sets for each distribution family.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DistributionParameters {
    /// lambda: rate parameter (events per day).
    Exponential {
        lambda: f64,
    },
    /// k: shape, lambda: scale (days).
    Weibull {
        k: f64,
        lambda: f64,
    },
    /// alpha: early hazard weight, beta: wearout hazard weight,
    /// gamma: transition rate, baseline: constant component.
    Bathtub {
        alpha: f64,
        beta: f64,
        gamma: f64,
        baseline: f64,
    },
    /// mu: log-mean, sigma: log-standard-deviation.
    LogNormal {
        mu: f64,
        sigma: f64,
    },
}

/// Instantaneous hazard rate at a specific time, with classification.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HazardRate {
    /// The computed hazard h(t) at evaluation time.
    pub value: f64,
    /// Qualitative classification derived from the hazard magnitude.
    pub classification: HazardClassification,
    /// The time (in days since creation) at which this was evaluated.
    pub evaluated_at_days: f64,
}

/// Qualitative hazard classification, mirroring actuarial risk grades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HazardClassification {
    /// h(t) < 0.005: negligible risk of irrelevance.
    Negligible,
    /// 0.005 <= h(t) < 0.02: low risk.
    Low,
    /// 0.02 <= h(t) < 0.05: moderate risk.
    Moderate,
    /// 0.05 <= h(t) < 0.10: elevated risk.
    Elevated,
    /// h(t) >= 0.10: critical -- memory is actively expiring.
    Critical,
}

// ---------------------------------------------------------------------------
// Memory entries
// ---------------------------------------------------------------------------

/// A record of when a memory was accessed and for which study.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccessRecord {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// The study that triggered the access.
    pub study: StudyId,
}

/// A single memory entry in the actuarial table.
///
/// Every entry carries its own survival function, access history, and
/// lifecycle metadata. The survival function is periodically re-fitted
/// as access data accumulates.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub memory_type: MemoryType,
    pub content: String,
    /// ISO 8601 timestamp of creation.
    pub created_at: String,
    /// ISO 8601 timestamp of last access.
    pub last_accessed: String,
    /// Full access history for distribution fitting.
    pub access_history: Vec<AccessRecord>,
    /// The fitted survival distribution governing this entry's mortality.
    pub survival_distribution: SurvivalDistribution,
    /// S(t) evaluated at the current time. Range [0, 1].
    pub current_survival_probability: f64,
    /// Instantaneous hazard rate at the current time.
    pub current_hazard_rate: f64,
    /// KL divergence between predicted and observed access pattern.
    pub surprise_index: f64,
    /// Optional embedding for semantic similarity scoring.
    pub embedding_vector: Option<Vec<f64>>,
    /// Source commit hash, if the memory originated from a code change.
    pub source_commit: Option<String>,
    /// Practitioner-friendly summary (Okonkwo's simplified version).
    pub practitioner_summary: String,
    /// Current lifecycle state.
    pub lifecycle_state: LifecycleState,
}

/// Relevance score components, exposing the full scoring breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelevanceScore {
    /// Final composite score in [0, 1].
    pub composite: f64,
    /// Embedding similarity component.
    pub embedding_similarity: f64,
    /// Survival probability component.
    pub survival_probability: f64,
    /// Hazard-adjusted recency component.
    pub hazard_adjusted_recency: f64,
    /// Access frequency component.
    pub access_frequency: f64,
    /// Goodness-of-fit component.
    pub goodness_of_fit: f64,
}

// ---------------------------------------------------------------------------
// Study protocol
// ---------------------------------------------------------------------------

/// Phases of the study protocol lifecycle.
///
/// Every task is a research study conducted by the five-member team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StudyPhase {
    /// Vassiliev and Abebe query memory; Chen gathers cross-repo context.
    LiteratureReview,
    /// Vassiliev formulates the approach with uncertainty estimates.
    Hypothesis,
    /// Okonkwo simplifies into implementable steps.
    ProtocolDesign,
    /// Petrov generates patches.
    Experiment,
    /// Okonkwo validates; Vassiliev checks statistical properties.
    PeerReview,
    /// INDEX.patch + COMMIT.msg produced.
    Publication,
}

/// Protocol execution mode, determined by remaining token budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolMode {
    /// P(completion) > 80%. Full five-phase protocol.
    Full,
    /// 50% < P(completion) <= 80%. Skip peer review iteration.
    Abbreviated,
    /// 20% < P(completion) <= 50%. Smallest valid patch.
    MinimumPublishableUnit,
    /// P(completion) <= 20%. Publish partial results and halt.
    EmergencyHalt,
}

/// Progress report emitted during study execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StudyProgress {
    pub phase: StudyPhase,
    pub agent: AgentId,
    pub role: AgentRole,
    pub study_id: StudyId,
    pub tokens_used: u64,
    pub tokens_budget: u64,
    /// Estimated probability of completion given remaining budget.
    pub p_completion: f64,
    /// Confidence in the quality of current output.
    pub confidence_in_output: f64,
    /// Current surprise index reading.
    pub surprise_index: f64,
    /// Current aggregate hazard rate of accessed memories.
    pub memory_hazard_rate: f64,
    /// Current protocol mode.
    pub protocol_mode: ProtocolMode,
}

/// The output of a completed study.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StudyPublication {
    /// The unified diff (INDEX.patch content).
    pub patch: String,
    /// The commit message (COMMIT.msg content).
    pub commit_message: String,
    /// Study metadata for the commit trailer.
    pub metadata: StudyMetadata,
}

/// Metadata attached to a study publication (commit trailer).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StudyMetadata {
    pub study_id: StudyId,
    pub protocol_mode: ProtocolMode,
    /// Survival estimate for the change itself.
    pub survival_estimate: SurvivalDistribution,
    /// Confidence in correctness.
    pub confidence: f64,
    /// Known limitations documented during peer review.
    pub known_limitations: Vec<String>,
    /// Agent who performed validation.
    pub validated_by: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// Agent identity
// ---------------------------------------------------------------------------

/// The role an agent plays in the research centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Principal Investigator. Designs studies and reviews statistics.
    PrincipalInvestigator,
    /// Practitioner Liaison. Validates for practical use.
    Practitioner,
    /// Research Fellow. Implements patches.
    ResearchFellow,
    /// Data Curator. Manages memory survival functions.
    DataCurator,
    /// Research Assistant. Coordinates cross-repo work.
    ResearchAssistant,
}

/// Agent identity -- the life record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LifeRecord {
    pub agent_id: AgentId,
    pub institution: String,
    pub role: AgentRole,
    pub specialty: String,
    pub capabilities: Vec<String>,
    pub authorization: Authorization,
    pub signing_key: Option<String>,
    /// ISO 8601 timestamp of agent provisioning.
    pub created_at: String,
    pub performance_history: PerformanceHistory,
}

/// Authorization constraints for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Authorization {
    pub branch_patterns: Vec<String>,
    pub max_patch_lines: Option<u32>,
    pub repos: Vec<String>,
}

/// Historical performance statistics for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceHistory {
    pub tasks_completed: u64,
    pub mean_confidence: f64,
    /// Mean survival time of patches produced (in days).
    pub mean_patch_survival_days: f64,
}

// ---------------------------------------------------------------------------
// Coordination
// ---------------------------------------------------------------------------

/// Type of a site report in multi-site coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SiteReportType {
    Enrollment,
    Progress,
    Results,
    Dependency,
    Budget,
}

/// A site report exchanged during multi-site study coordination.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SiteReport {
    pub schema: String,
    pub report_type: SiteReportType,
    pub from_site: SiteIdentity,
    pub to_site: Option<SiteIdentity>,
    pub report: StudyReportPayload,
    /// ISO 8601 timestamp.
    pub timestamp: String,
}

/// Identity of a research site in cross-repo coordination.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SiteIdentity {
    pub investigator: AgentId,
    pub institution: String,
    pub repo: RepoRef,
}

/// Payload within a site report.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StudyReportPayload {
    pub study_ref: StudyId,
    pub phase: StudyPhase,
    pub status: StudyStatus,
    pub dependencies: Vec<String>,
    pub confidence: f64,
    pub budget: BudgetSnapshot,
    pub surprise_index: f64,
}

/// Status of a study at a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StudyStatus {
    Proposed,
    InProgress,
    PeerReview,
    Published,
    Halted,
}

/// Snapshot of token budget consumption.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BudgetSnapshot {
    pub consumed: u64,
    pub allocated: u64,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Actuarial configuration read from git config `[but-ai.actuarial.*]`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActuarialConfig {
    /// Base ref for actuarial memory storage.
    pub branch: String,
    /// S(t) below which a memory enters moribund state.
    pub alive_threshold: f64,
    /// S(t) below which a memory is declared deceased.
    pub deceased_threshold: f64,
    /// Surprise index triggering cohort review.
    pub surprise_threshold: f64,
    /// Tasks between survival distribution re-fitting.
    pub refit_interval: u32,
    /// Maximum alive memory entries.
    pub max_alive_entries: usize,
    /// Default Weibull shape parameter for new architectural memories.
    pub default_weibull_k: f64,
    /// Default Weibull scale parameter (days).
    pub default_weibull_lambda: f64,
    /// Default exponential rate parameter (days).
    pub default_exponential_lambda: f64,
    /// Total token budget per study.
    pub token_budget: u64,
    /// P(completion) triggering minimum publishable unit mode.
    pub min_publishable_threshold: f64,
    /// P(completion) triggering emergency halt.
    pub halt_threshold: f64,
}

impl Default for ActuarialConfig {
    fn default() -> Self {
        Self {
            branch: "refs/but-ai/actuarial".to_string(),
            alive_threshold: 0.25,
            deceased_threshold: 0.10,
            surprise_threshold: 0.50,
            refit_interval: 5,
            max_alive_entries: 500,
            default_weibull_k: 1.8,
            default_weibull_lambda: 180.0,
            default_exponential_lambda: 3.0,
            token_budget: 50_000,
            min_publishable_threshold: 0.50,
            halt_threshold: 0.20,
        }
    }
}

/// Weights for the relevance scoring formula.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelevanceWeights {
    pub embedding_similarity: f64,
    pub survival_probability: f64,
    pub hazard_adjusted_recency: f64,
    pub access_frequency: f64,
    pub goodness_of_fit: f64,
}

impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            embedding_similarity: 0.30,
            survival_probability: 0.25,
            hazard_adjusted_recency: 0.20,
            access_frequency: 0.15,
            goodness_of_fit: 0.10,
        }
    }
}

/// Aggregate survival statistics for a cohort of memories.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LifeTable {
    /// Total number of entries that have ever been tracked.
    pub total_entries: u64,
    /// Currently alive.
    pub alive_count: u64,
    /// Currently moribund.
    pub moribund_count: u64,
    /// Currently deceased.
    pub deceased_count: u64,
    /// Aggregate hazard rate across all alive entries.
    pub aggregate_hazard_rate: f64,
    /// Mean survival probability of alive entries.
    pub mean_survival_probability: f64,
    /// Cohort identifier (e.g. "2026-Q1").
    pub cohort_label: String,
}

/// Compaction tier, determining how much of a memory is retained in context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTier {
    /// S(t) > 0.75: full content retained.
    Full,
    /// 0.25 < S(t) <= 0.75: practitioner summary only.
    Summary,
    /// S(t) <= 0.25: embedding + survival metadata only.
    Skeleton,
}

/// A peer review note from Okonkwo's validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerReviewNote {
    pub reviewer: AgentId,
    pub study_id: StudyId,
    pub verdict: ReviewVerdict,
    pub comments: Vec<String>,
    /// ISO 8601 timestamp.
    pub reviewed_at: String,
}

/// Outcome of a peer review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    /// Accepted without changes.
    Pass,
    /// Accepted with minor revisions needed.
    MinorRevisions,
    /// Rejected; requires significant rework.
    MajorRevisions,
    /// Rejected outright.
    Reject,
}
