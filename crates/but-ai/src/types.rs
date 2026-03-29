//! Shared types used across all `but-ai` modules.
//!
//! These are the structural threads that every module references.
//! They must exist before any module can compile.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Unique identifier for an agent in the loom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AgentId(pub String);

/// Unique identifier for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TaskId(pub String);

/// Unique identifier for a thread (memory entry).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ThreadId(pub String);

/// Unique identifier for a shuttle message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MessageId(pub String);

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

/// Reference to a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PrId {
    pub repo: RepoRef,
    pub number: u64,
}

/// The three weave patterns, each determining how warp and weft interact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WeavePattern {
    /// Every warp thread interacts with every weft thread.
    /// Dense, balanced memory retrieval for unfamiliar tasks.
    Plain,
    /// Weft threads skip some warp threads, creating diagonal lines.
    /// Lighter, faster retrieval for familiar tasks.
    Twill,
    /// Long floats where weft rides over many warp threads.
    /// Surface-level retrieval for quick, routine tasks.
    Satin,
}

impl Default for WeavePattern {
    fn default() -> Self {
        Self::Twill
    }
}

/// The type of a memory thread -- either structural (warp) or task-specific (weft).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThreadType {
    Warp,
    Weft,
}

/// Color classification for memory threads, encoding their functional role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThreadColor {
    /// Architectural knowledge (module patterns, design decisions).
    Structural,
    /// Coding conventions (naming, formatting, style).
    Convention,
    /// Team preferences and workflow rules.
    Preference,
    /// Cross-session learning from past tasks.
    Learned,
    /// Task-specific observation.
    Observation,
    /// Task-specific plan or intent.
    Plan,
    /// Coordination context from other repos.
    Coordination,
}

/// A single memory thread -- the atomic unit of woven memory.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Thread {
    pub id: ThreadId,
    pub thread_type: ThreadType,
    /// Structural importance (0.0 to 1.0). High-tension threads are critical.
    pub tension: f64,
    pub content: String,
    pub created_at: String,
    pub last_interlaced: String,
    pub interlacement_count: u64,
    /// Time-to-live in seconds. Warp defaults to 2592000 (30 days).
    pub ttl_seconds: u64,
    pub color: ThreadColor,
    /// Position in the warp (for warp threads) or pick number (for weft threads).
    pub position: u32,
    /// IDs of threads this thread has been interlaced with.
    pub connected_threads: Vec<ThreadId>,
    /// Source commit hash, if thread originated from a code change.
    pub source_commit: Option<String>,
}

/// Loom configuration read from git config.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoomConfig {
    pub default_pattern: WeavePattern,
    pub max_warp_threads: usize,
    pub max_weft_threads: usize,
    pub warp_ttl_seconds: u64,
    pub promotion_threshold: u32,
    pub tension_decay: f64,
    pub token_budget: u64,
    pub pattern_downgrade_threshold: f64,
    pub halt_threshold: f64,
}

impl Default for LoomConfig {
    fn default() -> Self {
        Self {
            default_pattern: WeavePattern::Twill,
            max_warp_threads: 200,
            max_weft_threads: 100,
            warp_ttl_seconds: 2_592_000,
            promotion_threshold: 3,
            tension_decay: 0.95,
            token_budget: 50_000,
            pattern_downgrade_threshold: 0.80,
            halt_threshold: 0.95,
        }
    }
}

/// Agent identity -- the maker's mark.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MakersMark {
    pub agent_id: AgentId,
    pub org_id: String,
    pub loom_position: LoomPosition,
    pub capabilities: Vec<String>,
    pub authorization: Authorization,
    pub signing_key: Option<String>,
    pub created_at: String,
}

/// The agent's position in the loom, determining their role and access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LoomPosition {
    /// Architectural authority (Tanaka).
    Warp,
    /// Patch production authority (Marchetti).
    Weft,
    /// Memory management authority (Osei).
    Heddle,
    /// Validation authority (Lindqvist).
    Selvedge,
    /// Coordination authority (Nakamura).
    Shuttle,
}

/// Authorization constraints for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Authorization {
    pub branch_patterns: Vec<String>,
    pub max_patch_lines: Option<u32>,
    pub repos: Vec<String>,
}

/// Phases of the weaving lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum WeavePhase {
    /// Tanaka establishes foundational context.
    Warp,
    /// Osei selects the weave pattern.
    Thread,
    /// Marchetti runs the weft through the warp.
    Shuttle,
    /// Marchetti compacts the weft against existing fabric.
    Beat,
    /// Lindqvist validates boundaries and consistency.
    Inspect,
    /// Final patch and commit message produced.
    Cut,
}

/// Progress report during a weaving operation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WeaveProgress {
    pub phase: WeavePhase,
    pub agent: AgentId,
    pub loom_position: LoomPosition,
    pub pick_number: u32,
    pub picks_estimated: u32,
    pub tokens_used: u64,
    pub tokens_budget: u64,
    pub weave_pattern: WeavePattern,
    /// Fabric integrity score (0.0 to 1.0).
    pub fabric_integrity: f64,
}

/// The output of a completed weaving operation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Fabric {
    /// The unified diff (INDEX.patch content).
    pub patch: String,
    /// The commit message (COMMIT.msg content).
    pub commit_message: String,
    /// Loom metadata for the commit trailer.
    pub metadata: FabricMetadata,
}

/// Metadata attached to a fabric (commit trailer).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FabricMetadata {
    /// e.g. "3W/5F" (3 warp concerns, 5 weft operations).
    pub thread_count: String,
    pub weave_pattern: WeavePattern,
    pub inspected_by: Option<AgentId>,
}
