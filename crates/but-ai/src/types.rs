//! Unified type system for the `but-ai` plugin.
//!
//! This module integrates the best types from all five organizational proposals:
//! - **001 (Tidal Protocol):** Coordination protocol, CRDT provenance, forge adapter
//! - **083 (Textile Morphology):** Adaptive retrieval density
//! - **084 (Loom & Verse):** Motif-based retrieval, tension tracking, narrative metadata
//! - **093 (Longevity & Risk):** Survival function expiration, hazard rates, confidence
//! - **145 (ShelfOS):** Call number classification, see-also graph, controlled vocabulary

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Git storage constants
// ---------------------------------------------------------------------------

/// Base ref namespace for all but-ai data stored in git refs.
pub const REF_PREFIX: &str = "refs/but-ai";

/// Generate a ref path for an agent's memory entry.
///
/// Format: `refs/but-ai/memory/<agent_id>/<entry_hash>`
pub fn memory_ref(agent_id: &str, entry_hash: &str) -> String {
    format!("{REF_PREFIX}/memory/{agent_id}/{entry_hash}")
}

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for an agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AgentId(pub String);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique identifier for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique identifier for a memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct EntryId(pub String);

impl std::fmt::Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique identifier for a coordination message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique identifier for a motif (recurring theme).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MotifId(pub String);

impl std::fmt::Display for MotifId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Unique identifier for a tension (contradiction or unresolved issue).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TensionId(pub String);

impl std::fmt::Display for TensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Agent identity (from 001, 093, 145)
// ---------------------------------------------------------------------------

/// The functional role an agent plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Designs architecture and reviews high-level structure.
    Architect,
    /// Produces patches and writes code.
    Implementer,
    /// Validates correctness, consistency, and quality.
    Validator,
    /// Manages cross-repo and cross-agent coordination.
    Coordinator,
}

/// Full agent identity record, stored in `refs/but-ai/memory/<agent-id>/identity/self`.
///
/// Combines the identity models from 001 (AgentIdentity), 084 (Colophon),
/// 093 (LifeRecord), and 145 (AgentIdentity).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentIdentity {
    /// The agent's unique identifier.
    pub agent_id: AgentId,
    /// Functional role within the system.
    pub role: AgentRole,
    /// What the agent can do.
    pub capabilities: Vec<String>,
    /// Authorization scope constraining allowed operations.
    pub authorization: AuthorizationScope,
    /// Signing key fingerprint for commit verification.
    pub signing_key: Option<String>,
    /// Historical performance statistics (from 093).
    pub performance_history: PerformanceHistory,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
}

/// Authorization scope constraining what an agent may do.
///
/// Combines branch patterns + max patch lines (consensus from all 5),
/// repo scope, and call number ranges (from 145).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuthorizationScope {
    /// Glob patterns for allowed branches (e.g. `"feat/*"`, `"fix/*"`).
    pub branch_patterns: Vec<String>,
    /// Maximum number of lines in a single patch.
    pub max_patch_lines: Option<u32>,
    /// Repositories the agent may operate on. `["*"]` means all.
    pub repos: Vec<String>,
    /// Call number ranges the agent is authorized for (from 145).
    /// E.g. `["ARCH.*", "SEC.*"]`. Empty means unrestricted.
    pub call_number_ranges: Vec<String>,
}

/// Historical performance statistics for an agent (from 093).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PerformanceHistory {
    /// Total tasks completed.
    pub tasks_completed: u64,
    /// Mean confidence score across completed tasks.
    pub mean_confidence: f64,
    /// Mean survival time of patches produced (in days).
    pub mean_patch_survival_days: f64,
}

impl Default for PerformanceHistory {
    fn default() -> Self {
        Self {
            tasks_completed: 0,
            mean_confidence: 0.0,
            mean_patch_survival_days: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Memory types (from 093, 145, 084, 001)
// ---------------------------------------------------------------------------

/// Lifecycle state of a memory entry.
///
/// From 093's three-state model: alive/moribund/deceased. The intermediate
/// moribund state prevents premature expiration of outlier memories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    /// `S(t) >= alive_threshold`. The memory is considered relevant.
    Alive,
    /// `S(t) < alive_threshold` but `>= deceased_threshold`. Under review.
    Moribund,
    /// `S(t) < deceased_threshold`. Formally expired but archived.
    Deceased,
}

/// A single memory entry -- the atomic unit of agent memory.
///
/// This is the big unified type combining:
/// - Classification with call numbers and subject headings (145)
/// - Narrative metadata with motifs and tension refs (084)
/// - Survival statistics with fitted distributions (093)
/// - CRDT provenance with consensus citations (001)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryEntry {
    /// Unique identifier (SHA-256 hash of content).
    pub id: EntryId,
    /// The agent that created this entry.
    pub agent: AgentId,
    /// The actual memory content.
    pub content: String,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-accessed timestamp.
    pub last_accessed: String,

    // -- Classification (from 145) --
    /// Multi-system classification with subject headings, call number, etc.
    pub classification: Classification,
    /// Cross-reference links to related entries.
    pub see_also: Vec<SeeAlsoLink>,

    // -- Narrative metadata (from 084) --
    /// Motifs (recurring themes) associated with this entry.
    pub motifs: Vec<MotifId>,
    /// Tension references (contradictions introduced/referenced/resolved).
    pub tension_refs: Vec<TensionRef>,

    // -- Survival statistics (from 093) --
    /// The fitted survival distribution governing this entry's mortality.
    pub survival: SurvivalMetadata,
    /// Current lifecycle state.
    pub state: MemoryState,

    // -- CRDT provenance (from 001) --
    /// Number of distinct agents that have cited this entry.
    pub consensus_citations: u64,
    /// Total number of times this entry has been accessed.
    pub access_count: u64,
    /// Source commit hash, if the memory originated from a code change.
    pub source_commit: Option<String>,
}

// ---------------------------------------------------------------------------
// Classification (from 145)
// ---------------------------------------------------------------------------

/// Multi-system classification for a memory entry.
///
/// Adopts three of ShelfOS's five classification systems:
/// subject headings, call numbers, and see-also links. Source and temporal
/// classification are fields on the entry itself.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Classification {
    /// Topical descriptors from the controlled vocabulary.
    pub subject_headings: Vec<String>,
    /// Hierarchical call number positioning the memory in the knowledge tree.
    pub call_number: CallNumber,
    /// Whether subject headings are from the controlled vocabulary.
    pub controlled_vocab: bool,
}

/// A hierarchical call number that positions a memory in the knowledge
/// structure. Segments are separated by dots: `ARCH.AUTH.MIDDLEWARE`.
///
/// Analogous to a Library of Congress call number -- it encodes both
/// subject and relative position within that subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CallNumber {
    /// The ordered segments, e.g. `["ARCH", "AUTH", "MIDDLEWARE"]`.
    pub segments: Vec<String>,
}

impl CallNumber {
    /// Create a call number from a dot-separated string.
    pub fn parse(s: &str) -> Self {
        Self {
            segments: s.split('.').map(|seg| seg.to_uppercase()).collect(),
        }
    }

    /// Render the call number as a dot-separated string.
    pub fn to_string_repr(&self) -> String {
        self.segments.join(".")
    }

    /// Return the depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// True if `self` is a prefix of (or equal to) `other`.
    pub fn is_ancestor_of(&self, other: &CallNumber) -> bool {
        if self.segments.len() > other.segments.len() {
            return false;
        }
        self.segments
            .iter()
            .zip(other.segments.iter())
            .all(|(a, b)| a == b)
    }

    /// Number of shared prefix segments with `other`.
    pub fn shared_depth(&self, other: &CallNumber) -> usize {
        self.segments
            .iter()
            .zip(other.segments.iter())
            .take_while(|(a, b)| a == b)
            .count()
    }
}

impl std::fmt::Display for CallNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string_repr())
    }
}

/// The type of relationship between two memory entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    /// Topically related.
    RelatedTo,
    /// The source depends on the target.
    DependsOn,
    /// The source contrasts with the target.
    ContrastsWith,
    /// The source supersedes the target.
    SupersededBy,
}

/// A single "see also" cross-reference link (from 145).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SeeAlsoLink {
    /// The entry this link points to.
    pub target_id: EntryId,
    /// The kind of relationship.
    pub relationship: Relationship,
    /// Human-readable note explaining the connection.
    pub note: String,
}

// ---------------------------------------------------------------------------
// Survival types (from 093)
// ---------------------------------------------------------------------------

/// Parametric survival distribution governing memory mortality.
///
/// Different memory types exhibit different mortality patterns:
/// architectural knowledge decays slowly (Weibull), bug knowledge expires
/// abruptly (Exponential), conventions follow a bathtub curve.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SurvivalDistribution {
    /// Constant hazard rate. Memoryless.
    Exponential {
        /// Rate parameter (events per day).
        lambda: f64,
    },
    /// Monotone hazard (increasing or decreasing depending on shape).
    Weibull {
        /// Shape parameter.
        k: f64,
        /// Scale parameter (days).
        lambda: f64,
    },
    /// High-low-high hazard curve (mixture model).
    Bathtub {
        /// Early hazard weight.
        alpha: f64,
        /// Wearout hazard weight.
        beta: f64,
        /// Transition rate.
        gamma: f64,
    },
    /// Heavy-tailed, for cross-repo knowledge.
    LogNormal {
        /// Log-mean.
        mu: f64,
        /// Log-standard-deviation.
        sigma: f64,
    },
}

/// Survival metadata attached to a memory entry (from 093).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SurvivalMetadata {
    /// The fitted survival distribution.
    pub distribution: SurvivalDistribution,
    /// `S(t)` evaluated at the current time. Range `[0, 1]`.
    pub current_probability: f64,
    /// Instantaneous hazard rate `h(t)` at the current time.
    pub hazard_rate: f64,
    /// KL divergence between predicted and observed access pattern.
    pub surprise_index: f64,
    /// Goodness-of-fit score in `[0, 1]`. Higher is better.
    pub goodness_of_fit: f64,
}

// ---------------------------------------------------------------------------
// Narrative types (from 084)
// ---------------------------------------------------------------------------

/// Severity of a narrative tension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TensionSeverity {
    /// Informational only.
    Low,
    /// Should be addressed in a future task.
    Moderate,
    /// Blocks further work in this area.
    High,
    /// Escalated: unresolved for longer than the escalation threshold.
    Critical,
}

/// A tracked tension -- a contradiction or unresolved issue (from 084).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tension {
    /// Unique identifier.
    pub id: TensionId,
    /// Description of the contradiction.
    pub description: String,
    /// Severity level.
    pub severity: TensionSeverity,
    /// Entry ID where this tension was introduced.
    pub introduced_in: EntryId,
    /// Entry ID where this tension was resolved, if applicable.
    pub resolved_in: Option<EntryId>,
}

/// The role a tension plays relative to a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TensionRole {
    /// This entry introduced the tension.
    Introduced,
    /// This entry references the tension.
    Referenced,
    /// This entry resolved the tension.
    Resolved,
}

/// A reference from a memory entry to a tension (from 084).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TensionRef {
    /// The tension being referenced.
    pub tension_id: TensionId,
    /// How this entry relates to the tension.
    pub role: TensionRole,
}

/// A recurring theme identified across multiple tasks (from 084).
///
/// When a theme appears in 3+ tasks, it becomes a motif -- a retrieval
/// anchor that captures thematic resonance beyond keyword matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Motif {
    /// Unique identifier.
    pub id: MotifId,
    /// Description of the recurring theme.
    pub description: String,
    /// Entry IDs where this motif appears.
    pub appearances: Vec<EntryId>,
    /// Related motifs.
    pub related_motifs: Vec<MotifId>,
}

// ---------------------------------------------------------------------------
// Coordination types (from 001)
// ---------------------------------------------------------------------------

/// Reference to a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct RepoRef {
    /// Forge type (github, gitlab, etc.).
    pub forge: ForgeType,
    /// Repository owner (user or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
}

impl std::fmt::Display for RepoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

/// Forge (hosting platform) type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForgeType {
    GitHub,
    GitLab,
    Bitbucket,
    Gitea,
}

/// Reference to a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PrRef {
    /// The repository this PR belongs to.
    pub repo: RepoRef,
    /// PR number within the repository.
    pub number: u64,
}

impl std::fmt::Display for PrRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.repo, self.number)
    }
}

/// Status of a pull request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrStatus {
    Open,
    Closed,
    Merged,
    Draft,
}

/// Types of structured coordination messages between agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    TaskAssignment,
    StatusReport,
    DependencyDeclaration,
    PatchHandoff,
    BudgetReport,
}

/// A structured coordination message exchanged via PR comments (from 001).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoordinationMessage {
    /// Protocol schema version, e.g. `"but-ai/coordination/v1"`.
    pub schema: String,
    /// The type of message.
    pub message_type: MessageType,
    /// Sending agent.
    pub from: AgentId,
    /// Receiving agent (or broadcast).
    pub to: Option<AgentId>,
    /// Type-specific payload.
    pub payload: serde_json::Value,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Budget types (from 145, 001)
// ---------------------------------------------------------------------------

/// Token budget tracking with mandatory reserves for cataloging and
/// coordination -- because an unclassified result is a lost result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TokenBudget {
    /// Total tokens available for the task.
    pub total: u64,
    /// Tokens consumed so far.
    pub used: u64,
    /// Tokens reserved for post-task cataloging (never skipped).
    pub catalog_reserve: u64,
    /// Tokens reserved for coordination (never skipped).
    pub coordination_reserve: u64,
}

impl TokenBudget {
    /// Create a new budget with standard reserves.
    pub fn new(total: u64) -> Self {
        Self {
            total,
            used: 0,
            catalog_reserve: 1_500,
            coordination_reserve: 2_000,
        }
    }

    /// Tokens remaining (total - used).
    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.used)
    }

    /// Fraction of total budget consumed.
    pub fn utilization(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.used as f64 / self.total as f64
    }

    /// Tokens available for the current work phase (excludes reserves).
    pub fn available_for_work(&self) -> u64 {
        let reserved = self.catalog_reserve + self.coordination_reserve;
        self.total.saturating_sub(self.used).saturating_sub(reserved)
    }
}

impl Default for TokenBudget {
    fn default() -> Self {
        Self::new(32_000)
    }
}

// ---------------------------------------------------------------------------
// Task types
// ---------------------------------------------------------------------------

/// Phases of the task lifecycle, unifying the phase models from all 5 proposals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskPhase {
    /// Retrieve relevant memories, classify task context.
    Classify,
    /// Design approach and plan.
    Plan,
    /// Generate patches and write code.
    Implement,
    /// Validate correctness, consistency, and quality.
    Validate,
    /// Classify the new work as future memory.
    Catalog,
    /// Create PR, post coordination messages.
    Coordinate,
}

// ---------------------------------------------------------------------------
// Scoring types (from consensus formula in PROPOSAL.md)
// ---------------------------------------------------------------------------

/// Weights for the unified retrieval scoring formula.
///
/// ```text
/// score = motif_resonance      * w.motif_resonance
///       + call_number_proximity * w.call_number_proximity
///       + see_also_distance     * w.see_also_distance
///       + survival_probability  * w.survival_probability
///       + freshness             * w.freshness
///       + tension_boost         * w.tension_boost
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelevanceWeights {
    /// Weight for motif resonance (from 084).
    pub motif_resonance: f64,
    /// Weight for call number proximity (from 145).
    pub call_number_proximity: f64,
    /// Weight for see-also graph distance (from 145).
    pub see_also_distance: f64,
    /// Weight for survival probability (from 093).
    pub survival_probability: f64,
    /// Weight for freshness/recency.
    pub freshness: f64,
    /// Weight for tension urgency boost.
    pub tension_boost: f64,
}

impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            motif_resonance: 0.25,
            call_number_proximity: 0.20,
            see_also_distance: 0.20,
            survival_probability: 0.15,
            freshness: 0.10,
            tension_boost: 0.10,
        }
    }
}

/// A memory entry with its computed relevance score and breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoredMemory {
    /// The memory entry.
    pub entry: MemoryEntry,
    /// The composite relevance score (0.0 to 1.0).
    pub score: f64,
    /// Per-component score breakdown.
    pub breakdown: ScoreBreakdown,
}

/// Per-component score breakdown for a retrieval result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoreBreakdown {
    /// Motif resonance component.
    pub motif_resonance: f64,
    /// Call number proximity component.
    pub call_number_proximity: f64,
    /// See-also graph distance component.
    pub see_also_distance: f64,
    /// Survival probability component.
    pub survival_probability: f64,
    /// Freshness/recency component.
    pub freshness: f64,
    /// Tension urgency boost component.
    pub tension_boost: f64,
}

// ---------------------------------------------------------------------------
// Core traits
// ---------------------------------------------------------------------------

/// Storage backend for memory entries.
pub trait MemoryStore: Send + Sync {
    /// Store a memory entry. Overwrites if an entry with the same ID exists.
    fn store(&self, entry: &MemoryEntry) -> anyhow::Result<()>;

    /// Load a memory entry by ID.
    fn load(&self, id: &EntryId) -> anyhow::Result<Option<MemoryEntry>>;

    /// List all entry IDs, optionally filtered by state.
    fn list(&self, state: Option<MemoryState>) -> anyhow::Result<Vec<EntryId>>;

    /// Transition an entry to a new lifecycle state.
    fn transition(&self, id: &EntryId, new_state: MemoryState) -> anyhow::Result<()>;

    /// Delete an entry permanently.
    fn delete(&self, id: &EntryId) -> anyhow::Result<()>;
}

/// Retrieval engine for scoring and ranking memory entries.
pub trait MemoryRetriever: Send + Sync {
    /// Retrieve the top-scoring entries for a query.
    fn retrieve(
        &self,
        query: &str,
        max_results: usize,
        weights: &RelevanceWeights,
    ) -> anyhow::Result<Vec<ScoredMemory>>;
}

/// Adapter for interacting with a code forge (GitHub, GitLab, etc.).
pub trait ForgeAdapter: Send + Sync {
    /// Create a pull request.
    fn create_pr(
        &self,
        repo: &RepoRef,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<PrRef>;

    /// Post a comment on a pull request.
    fn comment(&self, pr: &PrRef, body: &str) -> anyhow::Result<()>;

    /// List comments on a pull request.
    fn list_comments(&self, pr: &PrRef) -> anyhow::Result<Vec<String>>;

    /// Get the status of a pull request.
    fn pr_status(&self, pr: &PrRef) -> anyhow::Result<PrStatus>;

    /// Add a label to a pull request.
    fn add_label(&self, pr: &PrRef, label: &str) -> anyhow::Result<()>;

    /// List pull requests matching the given labels.
    fn list_prs(&self, repo: &RepoRef, labels: &[&str]) -> anyhow::Result<Vec<PrRef>>;

    /// Return the forge type this adapter handles.
    fn forge_type(&self) -> ForgeType;
}

/// Signer and verifier for agent commits.
pub trait CommitSigner: Send + Sync {
    /// Sign a message, returning the signature bytes.
    fn sign(&self, message: &[u8]) -> anyhow::Result<Vec<u8>>;

    /// Verify a signature against a message and agent identity.
    fn verify(
        &self,
        message: &[u8],
        signature: &[u8],
        agent: &AgentId,
    ) -> anyhow::Result<bool>;
}
