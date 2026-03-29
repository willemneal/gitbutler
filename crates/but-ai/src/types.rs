//! Shared types used across all `but-ai` modules.
//!
//! These are the tidal structures that every module references --
//! the manifests, the tides, the identities that flow through the protocol.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for an agent in the collective.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AgentId(pub String);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a manifest entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct EntryId(pub String);

impl std::fmt::Display for EntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a protocol message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Reference to a repository in cross-repo coordination.
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

impl RepoRef {
    pub fn new(owner: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            name: name.into(),
        }
    }
}

/// Reference to a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PrRef {
    pub repo: RepoRef,
    pub number: u64,
}

impl std::fmt::Display for PrRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.repo, self.number)
    }
}

// ---------------------------------------------------------------------------
// Tide -- the clock
// ---------------------------------------------------------------------------

/// The four phases of a tide cycle, each lasting ~90 minutes within a 6-hour cycle.
///
/// The tide is the collective's clock. Decisions that cannot reach consensus
/// within one tide are deferred to the next. This prevents infinite negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TidePhase {
    /// Water rising -- new tasks accepted, agents mobilize.
    Flood,
    /// Peak -- active execution, maximum parallelism.
    High,
    /// Water falling -- wrap up, produce partial results if needed.
    Ebb,
    /// Trough -- maintenance, memory expiration, consensus review.
    Low,
}

impl TidePhase {
    /// Returns the next phase in the cycle.
    pub fn next(self) -> Self {
        match self {
            Self::Flood => Self::High,
            Self::High => Self::Ebb,
            Self::Ebb => Self::Low,
            Self::Low => Self::Flood,
        }
    }

    /// Returns the duration of this phase in seconds (90 minutes = 5400s).
    pub fn duration_seconds(self) -> u64 {
        5400
    }

    /// Whether new tasks can be accepted in this phase.
    pub fn accepts_new_tasks(self) -> bool {
        matches!(self, Self::Flood | Self::High)
    }

    /// Whether consensus votes are collected in this phase.
    pub fn collects_votes(self) -> bool {
        matches!(self, Self::High | Self::Ebb)
    }
}

impl std::fmt::Display for TidePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flood => write!(f, "flood"),
            Self::High => write!(f, "high"),
            Self::Ebb => write!(f, "ebb"),
            Self::Low => write!(f, "low"),
        }
    }
}

/// A specific point in tidal time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TideMark {
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Current phase of the tide.
    pub phase: TidePhase,
    /// Cycle number since epoch (increments every 6 hours).
    pub cycle: u64,
    /// Seconds elapsed within the current phase (0..5400).
    pub phase_elapsed_seconds: u64,
}

// ---------------------------------------------------------------------------
// Manifest entry -- the memory
// ---------------------------------------------------------------------------

/// Category of a manifest entry, determining its TTL and purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ManifestCategory {
    /// Recurring code patterns, conventions, idioms. TTL: 30 days.
    Pattern,
    /// Specific facts about the codebase. TTL: 7 days.
    Fact,
    /// Architectural decisions and rationale. TTL: 90 days.
    Decision,
    /// Recent errors and resolutions. TTL: 2 days.
    Error,
    /// Agent identity records. TTL: never.
    Identity,
}

impl ManifestCategory {
    /// Default TTL for this category in seconds.
    pub fn default_ttl_seconds(self) -> Option<u64> {
        match self {
            Self::Pattern => Some(30 * 24 * 3600),   // 720h
            Self::Fact => Some(7 * 24 * 3600),        // 168h
            Self::Decision => Some(90 * 24 * 3600),   // 2160h
            Self::Error => Some(2 * 24 * 3600),       // 48h
            Self::Identity => None,                    // never expires
        }
    }
}

/// A single manifest entry -- the atomic unit of agent memory.
///
/// Modeled after a shipping manifest: a document that travels with cargo,
/// listing contents, origin, destination, and handling instructions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManifestEntry {
    /// SHA-256 hash of the content, used as the entry ID.
    pub id: EntryId,
    /// The agent that created this entry.
    pub agent: AgentId,
    /// Category determining TTL and retrieval behavior.
    pub category: ManifestCategory,
    /// ISO-8601 creation timestamp.
    pub created: String,
    /// TTL as a duration string (e.g. "720h"). None for identity entries.
    pub ttl: Option<String>,
    /// ISO-8601 expiration timestamp. None for identity entries.
    pub expires: Option<String>,
    /// Searchable tags for this entry.
    pub tags: Vec<String>,
    /// The actual memory content.
    pub content: String,
    /// SHA-256 hash of the embedding vector (embeddings stored separately).
    pub embedding_hash: Option<String>,
    /// Decay factor applied per hour to relevance score. Typically 0.95.
    pub relevance_decay: f64,
    /// Number of times this entry has been accessed.
    pub access_count: u64,
    /// ISO-8601 timestamp of last access.
    pub last_accessed: Option<String>,
    /// Tidal mark at creation (e.g. "high tide, Rotterdam, +0.3m").
    pub tide_created: String,
    /// Version counter for identity entries (incremented on update).
    pub version: u64,
    /// How many distinct agents have referenced this entry.
    pub consensus_citations: u64,
}

impl ManifestEntry {
    /// Whether this entry has expired relative to the given ISO-8601 timestamp.
    ///
    /// Identity entries never expire. For other categories, compares
    /// the `expires` field against the provided current time string.
    pub fn is_expired(&self, now: &str) -> bool {
        match &self.expires {
            None => false,
            Some(exp) => now > exp.as_str(),
        }
    }

    /// Increment the access counter and update last_accessed.
    pub fn record_access(&mut self, now: String) {
        self.access_count += 1;
        self.last_accessed = Some(now);
    }
}

// ---------------------------------------------------------------------------
// Consensus
// ---------------------------------------------------------------------------

/// Weight assigned to a consensus vote based on the voting agent's track record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsensusWeight {
    /// The voting agent.
    pub agent: AgentId,
    /// Weight between 0.0 and 1.0. All agents start at 1.0 (equal peers).
    pub weight: f64,
    /// Number of successful consensus participations.
    pub participation_count: u64,
}

/// The outcome of a consensus round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConsensusOutcome {
    /// Quorum reached, decision approved.
    Approved,
    /// Quorum reached, decision rejected.
    Rejected,
    /// Not enough votes within the tide cycle.
    Deferred,
    /// Tie (e.g. 2-2 with 1 abstention).
    Deadlocked,
}

/// A single vote cast in a consensus round.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Vote {
    pub agent: AgentId,
    pub approve: bool,
    pub weight: f64,
    pub reason: Option<String>,
    pub tide_mark: TideMark,
}

// ---------------------------------------------------------------------------
// Protocol messages
// ---------------------------------------------------------------------------

/// Types of structured PR comment messages between agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MessageType {
    TaskAssignment,
    StatusReport,
    DependencyDeclaration,
    PatchHandoff,
    BudgetReport,
    ConsensusRequest,
    ConsensusVote,
}

/// Status of a task as reported by an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task accepted, work not started.
    Accepted,
    /// Actively working on the task.
    InProgress,
    /// Work completed, patch produced.
    Completed,
    /// Work partially completed (budget exhaustion or tide deadline).
    Partial,
    /// Task blocked on a dependency.
    Blocked,
    /// Task failed.
    Failed,
}

/// The target of a protocol message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageTarget {
    /// A specific agent.
    Agent(AgentId),
    /// All agents in the collective.
    Broadcast,
}

// ---------------------------------------------------------------------------
// Agent identity
// ---------------------------------------------------------------------------

/// Capabilities that an agent can declare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCapability {
    PatchGeneration,
    DiffAnalysis,
    CodeReview,
    MemoryManagement,
    ProtocolCoordination,
    SecurityAudit,
    BudgetTracking,
    ProviderAbstraction,
    /// A custom capability not in the standard set.
    Custom(String),
}

/// Authorization scope for an agent -- what branches, repos, and patch sizes are allowed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuthorizationScope {
    /// Glob patterns for allowed branches (e.g. "feat/*", "fix/*").
    pub branches: Vec<String>,
    /// Repository references the agent is authorized for.
    pub repos: Vec<String>,
    /// Maximum number of lines in a single patch.
    pub max_patch_lines: Option<u32>,
}

/// Full agent identity record, stored in `refs/but-ai/memory/<agent-id>/identity/self`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentIdentity {
    pub name: AgentId,
    pub organization: String,
    pub capabilities: Vec<AgentCapability>,
    pub authorization_scope: AuthorizationScope,
    pub signing_key_fingerprint: Option<String>,
    /// ISO-8601 creation timestamp.
    pub created: String,
    /// Version counter, incremented on each update.
    pub version: u64,
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

/// Status of a signing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KeyStatus {
    /// Key is active and valid.
    Active,
    /// Key was rotated on schedule; old commits still trusted.
    Retired,
    /// Key may have been compromised; all signed commits suspect.
    Compromised,
}

/// Result of verifying a signed commit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationResult {
    pub valid: bool,
    pub agent: Option<AgentId>,
    pub organization: Option<String>,
    pub key_status: Option<KeyStatus>,
    pub authorized: bool,
    pub denial_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

/// Token usage breakdown for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input + self.output
    }

    pub fn zero() -> Self {
        Self {
            input: 0,
            output: 0,
        }
    }
}

/// Provider type for LLM calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Ollama,
    LMStudio,
    /// External provider discovered via PATH shim.
    Shim(String),
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Top-level configuration for the but-ai plugin, read from git config keys.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TidalConfig {
    /// Maximum tokens per task.
    pub token_budget: u64,
    /// Default memory entry TTL (e.g. "720h").
    pub default_memory_ttl: String,
    /// Maximum memory entries injected per retrieval.
    pub max_memory_entries: usize,
    /// Minimum agents required for consensus quorum.
    pub consensus_quorum: usize,
    /// Hours per tide cycle.
    pub tide_cycle_hours: u64,
    /// Git ref namespace for memory.
    pub memory_ref_prefix: String,
    /// Git ref namespace for fleet manifests.
    pub fleet_ref_prefix: String,
    /// Forge type (github, gitlab, etc.).
    pub forge_type: String,
    /// Maximum PR comments per coordination event.
    pub max_coordination_comments: usize,
    /// Relevance score floor -- entries below this are excluded.
    pub relevance_floor: f64,
}

impl Default for TidalConfig {
    fn default() -> Self {
        Self {
            token_budget: 50_000,
            default_memory_ttl: "720h".to_string(),
            max_memory_entries: 5,
            consensus_quorum: 3,
            tide_cycle_hours: 6,
            memory_ref_prefix: "refs/but-ai/memory".to_string(),
            fleet_ref_prefix: "refs/but-ai/fleet".to_string(),
            forge_type: "github".to_string(),
            max_coordination_comments: 5,
            relevance_floor: 0.3,
        }
    }
}
