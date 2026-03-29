//! Shared types used across all `but-ai` modules.
//!
//! These are the narrative building blocks that every module references:
//! chapters, motifs, tensions, arcs, colophons, and the workshop roles
//! that bring them together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for an agent in the workshop.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AgentId(pub String);

/// Unique identifier for a task (maps to a chapter).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TaskId(pub String);

/// Unique identifier for a chapter in the narrative.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ChapterId(pub u64);

/// Unique identifier for a motif (recurring theme).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct MotifId(pub String);

/// Unique identifier for a tension (contradiction/conflict).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TensionId(pub String);

/// Unique identifier for a story arc.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ArcId(pub String);

/// Unique identifier for a correspondence letter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct LetterId(pub String);

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

// ---------------------------------------------------------------------------
// Chapter -- the atomic unit of narrative memory
// ---------------------------------------------------------------------------

/// The setting of a chapter: the codebase state when the chapter was written.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChapterSetting {
    pub branch: String,
    pub workspace_state_hash: String,
    pub timestamp: String,
}

/// The plot of a chapter: what happened, how it was approached, what resulted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChapterPlot {
    pub task: String,
    pub approach: String,
    pub outcome: String,
}

/// The colophon of a chapter: who was involved and what artifacts were produced.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChapterColophon {
    pub author: AgentId,
    pub editor: AgentId,
    pub continuity_checker: AgentId,
    pub patch_ref: String,
    pub commit_sha: Option<String>,
}

/// A single chapter in the ongoing story -- the atomic unit of narrative memory.
///
/// A chapter is not a key-value pair. It is a self-contained narrative unit with
/// a setting, characters, plot, motifs, and tensions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Chapter {
    pub chapter_number: u64,
    pub title: String,
    pub arc: ArcId,
    pub setting: ChapterSetting,
    /// The files and modules involved in this chapter.
    pub characters: Vec<String>,
    pub plot: ChapterPlot,
    /// Recurring themes that connect this chapter to others.
    pub motifs: Vec<MotifId>,
    /// Contradictions introduced by this chapter.
    pub tensions_introduced: Vec<TensionEntry>,
    /// Contradictions resolved by this chapter.
    pub tensions_resolved: Vec<TensionId>,
    pub colophon: ChapterColophon,
}

// ---------------------------------------------------------------------------
// Motif -- recurring themes that anchor retrieval
// ---------------------------------------------------------------------------

/// A single variation of a motif in a specific chapter.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MotifVariation {
    pub chapter: u64,
    pub form: String,
}

/// A recurring theme identified by the editor. When a theme appears in three
/// or more chapters, it becomes a motif -- a retrieval anchor that captures
/// thematic resonance beyond keyword matching.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Motif {
    pub motif_id: MotifId,
    pub description: String,
    pub first_appearance: u64,
    pub appearances: Vec<u64>,
    pub variations: Vec<MotifVariation>,
    pub related_motifs: Vec<MotifId>,
}

// ---------------------------------------------------------------------------
// Tension -- contradictions and unresolved issues
// ---------------------------------------------------------------------------

/// Severity of a narrative tension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TensionSeverity {
    /// Informational only.
    Low,
    /// Should be addressed in a future chapter.
    Moderate,
    /// Blocks further work in this arc.
    High,
    /// Escalated: unresolved for longer than the escalation threshold.
    Critical,
}

/// The lifecycle state of a tension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TensionState {
    /// Newly identified, awaiting resolution.
    Active,
    /// Escalated due to prolonged non-resolution.
    Escalated,
    /// Resolved by a subsequent chapter.
    Resolved,
}

/// A specific contradiction or unresolved issue in the narrative.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TensionEntry {
    pub id: TensionId,
    pub description: String,
    pub severity: TensionSeverity,
    pub suggested_resolution: Option<String>,
}

/// A tracked tension with full lifecycle metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tension {
    pub id: TensionId,
    pub description: String,
    pub severity: TensionSeverity,
    pub state: TensionState,
    pub introduced_in: u64,
    pub resolved_in: Option<u64>,
    pub created_at: String,
    pub escalated_at: Option<String>,
    pub suggested_resolution: Option<String>,
}

// ---------------------------------------------------------------------------
// Arc -- thematic groupings of related chapters
// ---------------------------------------------------------------------------

/// The state of a story arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArcState {
    /// Actively receiving new chapters.
    Active,
    /// No new chapters for longer than the dormancy threshold.
    Dormant,
    /// Summarized and archived, individual chapters removed.
    Archived,
}

/// A story arc -- a thematic grouping of related chapters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Arc {
    pub arc_id: ArcId,
    pub title: String,
    pub state: ArcState,
    pub chapters: Vec<u64>,
    pub motifs: Vec<MotifId>,
    /// Active tensions within this arc.
    pub active_tensions: Vec<TensionId>,
    pub last_chapter_at: String,
    pub created_at: String,
}

/// A compressed summary of a dormant or archived arc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArcSummary {
    pub arc_id: ArcId,
    pub title: String,
    pub chapter_count: u64,
    pub motifs: Vec<MotifId>,
    pub active_tensions: Vec<TensionId>,
    pub summary_text: String,
    pub summarized_at: String,
}

// ---------------------------------------------------------------------------
// Workshop roles
// ---------------------------------------------------------------------------

/// The literary role of an agent in the workshop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorkshopRole {
    /// Orozco: writes the text (patch generation).
    Author,
    /// Brenner: maintains narrative coherence (memory curation).
    Editor,
    /// Sato: catches contradictions (consistency checking).
    ContinuityChecker,
    /// Hartmann: manages cross-repo distribution (publishing).
    Publisher,
}

/// Phases of the chapter lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChapterPhase {
    /// Brenner reads the task, identifies thematic connections.
    Premise,
    /// Orozco plans the chapter structure.
    Outline,
    /// Orozco writes the first draft.
    Draft,
    /// Orozco refines based on self-review.
    Revision,
    /// Sato checks for contradictions with existing chapters.
    Continuity,
    /// Brenner annotates with narrative metadata.
    Editing,
    /// INDEX.patch + COMMIT.msg produced; chapter complete.
    Publication,
}

// ---------------------------------------------------------------------------
// Relevance scoring
// ---------------------------------------------------------------------------

/// Relevance score components for narrative memory retrieval.
///
/// ```text
/// score = 0.30 * motif_resonance
///       + 0.25 * embedding_similarity
///       + 0.20 * arc_relevance
///       + 0.15 * recency
///       + 0.10 * tension_urgency
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelevanceScore {
    pub motif_resonance: f64,
    pub embedding_similarity: f64,
    pub arc_relevance: f64,
    pub recency: f64,
    pub tension_urgency: f64,
}

impl RelevanceScore {
    /// Compute the weighted total relevance score.
    pub fn total(&self) -> f64 {
        0.30 * self.motif_resonance
            + 0.25 * self.embedding_similarity
            + 0.20 * self.arc_relevance
            + 0.15 * self.recency
            + 0.10 * self.tension_urgency
    }

    /// Create a zero-valued score.
    pub fn zero() -> Self {
        Self {
            motif_resonance: 0.0,
            embedding_similarity: 0.0,
            arc_relevance: 0.0,
            recency: 0.0,
            tension_urgency: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Progress reporting
// ---------------------------------------------------------------------------

/// Progress report during a chapter lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChapterProgress {
    pub phase: ChapterPhase,
    pub agent: AgentId,
    pub role: WorkshopRole,
    pub chapter: u64,
    pub arc: ArcId,
    pub draft_number: u32,
    pub tokens_used: u64,
    pub tokens_budget: u64,
    /// List of currently active motifs.
    pub motifs_active: Vec<MotifId>,
    pub tensions_unresolved: u32,
    /// Narrative coherence score (0.0 to 1.0).
    pub narrative_coherence: f64,
}

// ---------------------------------------------------------------------------
// Output artifacts
// ---------------------------------------------------------------------------

/// The output of a completed chapter: INDEX.patch + COMMIT.msg.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Manuscript {
    /// The unified diff (INDEX.patch content).
    pub patch: String,
    /// The commit message (COMMIT.msg content).
    pub commit_message: String,
    /// Narrative metadata for the commit.
    pub colophon: ManuscriptColophon,
}

/// Metadata attached to a manuscript (commit trailers).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManuscriptColophon {
    pub chapter_number: u64,
    pub arc: ArcId,
    pub motifs: Vec<MotifId>,
    pub tensions_introduced: Vec<TensionId>,
    pub continuity_verified_by: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// Correspondence (cross-repo)
// ---------------------------------------------------------------------------

/// Type of a correspondence letter between communes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LetterType {
    /// A new task commission.
    Commission,
    /// Progress update on an existing task.
    Progress,
    /// Dependency notification.
    Dependency,
    /// A completed manuscript for review.
    Manuscript,
    /// Token budget status.
    Budget,
}

/// The sender or receiver of a correspondence letter.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Correspondent {
    pub author: AgentId,
    pub commune: String,
    pub repo: RepoRef,
}

/// Content of a correspondence letter.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LetterContent {
    pub subject: String,
    pub arc: Option<ArcId>,
    pub chapter_ref: Option<String>,
    pub status: Option<String>,
    pub dependencies: Vec<String>,
    pub budget: Option<BudgetStatus>,
    pub motifs: Vec<MotifId>,
}

/// Token budget status for correspondence.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BudgetStatus {
    pub used: u64,
    pub total: u64,
}

/// A correspondence letter exchanged via PR comments.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Letter {
    pub schema: String,
    pub letter_type: LetterType,
    pub from: Correspondent,
    pub to: Correspondent,
    pub content: LetterContent,
    pub timestamp: String,
}

// ---------------------------------------------------------------------------
// Agent identity (colophon)
// ---------------------------------------------------------------------------

/// Agent identity, modeled as a colophon -- the signed document found
/// in hand-printed books that identifies the maker.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Colophon {
    pub agent_id: AgentId,
    pub commune: String,
    pub role: WorkshopRole,
    pub capabilities: Vec<String>,
    pub style: String,
    pub authorization: AgentAuthorization,
    pub signing_key: Option<String>,
    pub first_chapter: Option<u64>,
    pub chapters_authored: u64,
    pub created_at: String,
}

/// Authorization constraints for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentAuthorization {
    pub branch_patterns: Vec<String>,
    pub max_chapter_size: Option<u32>,
    pub repos: Vec<String>,
}

/// Key lifecycle events for agent signing keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum KeyEvent {
    /// Key provisioned (author signs first book contract).
    Provisioned,
    /// Key rotated (author adopts a new pen name).
    Rotated,
    /// Key compromised (forged manuscripts discovered).
    Compromised,
    /// Key decommissioned (author retires).
    Decommissioned,
}

/// A record in the key lifecycle log.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KeyLifecycleEntry {
    pub event: KeyEvent,
    pub key_id: String,
    pub timestamp: String,
    pub succeeded_by: Option<String>,
    pub forgery_detected_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Narrative engine configuration, read from git config.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NarrativeConfig {
    /// Base ref for narrative storage.
    pub story_branch: String,
    /// Seconds of inactivity before an arc goes dormant.
    pub arc_dormancy_seconds: u64,
    /// Minimum appearances for motif emergence.
    pub motif_threshold: u32,
    /// Seconds before unresolved tension is escalated.
    pub tension_escalation_seconds: u64,
    /// Maximum active chapters before forced compaction.
    pub max_chapters: u64,
    /// Maximum tokens per arc summary.
    pub summary_max_tokens: u64,
    /// Total token budget per task.
    pub token_budget: u64,
    /// Budget fraction triggering minimal annotations (flash fiction mode).
    pub flash_fiction_threshold: f64,
    /// Budget fraction triggering single-draft mode.
    pub single_draft_threshold: f64,
    /// Budget fraction triggering partial output (cliffhanger).
    pub cliffhanger_threshold: f64,
}

impl Default for NarrativeConfig {
    fn default() -> Self {
        Self {
            story_branch: "refs/but-ai/story".to_string(),
            arc_dormancy_seconds: 2_592_000,  // 30 days
            motif_threshold: 3,
            tension_escalation_seconds: 1_209_600, // 14 days
            max_chapters: 500,
            summary_max_tokens: 500,
            token_budget: 45_000,
            flash_fiction_threshold: 0.75,
            single_draft_threshold: 0.85,
            cliffhanger_threshold: 0.95,
        }
    }
}
