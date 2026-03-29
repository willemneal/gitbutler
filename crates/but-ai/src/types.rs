//! Shared types for the card-catalog memory architecture.
//!
//! Every type in this module corresponds to a library science concept:
//! catalog entries, call numbers, classifications, cross-references,
//! and circulation records. These are the index cards that every
//! module reads and writes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

/// Unique identifier for a catalog item (memory entry).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct ItemId(pub String);

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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

/// Reference to a repository (collection) in a forge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CollectionRef {
    /// Full collection identifier, e.g. `github.com/org/repo`.
    pub uri: String,
}

impl std::fmt::Display for CollectionRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.uri)
    }
}

/// Identifier for a loan (pull request) within a collection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct LoanId(pub u64);

/// Identifier for a note (comment) on a loan.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct NoteId(pub u64);

// ---------------------------------------------------------------------------
// Call Number
// ---------------------------------------------------------------------------

/// A hierarchical call number that positions a memory in the knowledge
/// structure. Segments are separated by dots: `ARCH.AUTH.MIDDLEWARE`.
///
/// The call number is analogous to a Library of Congress call number —
/// it encodes both subject and relative position within that subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CallNumber {
    /// The ordered segments, e.g. `["ARCH", "AUTH", "MIDDLEWARE"]`.
    pub segments: Vec<String>,
}

impl CallNumber {
    /// Create a call number from dot-separated string.
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

    /// Return the top-level category (first segment), if any.
    pub fn top_level(&self) -> Option<&str> {
        self.segments.first().map(|s| s.as_str())
    }
}

impl std::fmt::Display for CallNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string_repr())
    }
}

// ---------------------------------------------------------------------------
// Classification Systems
// ---------------------------------------------------------------------------

/// The five simultaneous classification systems applied to every catalog entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Classification {
    /// Subject headings — topical descriptors from the controlled vocabulary.
    pub subject_headings: Vec<String>,
    /// Hierarchical call number positioning the memory in the knowledge tree.
    pub call_number: CallNumber,
    /// Provenance — where the memory originated.
    pub source: SourceClassification,
    /// Temporal metadata — when the memory was created, accessed, validated.
    pub temporal: TemporalClassification,
}

/// Source (provenance) classification — where a memory came from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SourceClassification {
    /// The task that produced this memory.
    pub task: Option<TaskId>,
    /// The agent that created it.
    pub agent: Option<AgentId>,
    /// The branch it was created on.
    pub branch: Option<String>,
    /// The tool or operation that produced the observation.
    pub tool: Option<String>,
}

/// Temporal classification — when events occurred.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemporalClassification {
    /// ISO-8601 creation timestamp.
    pub created: String,
    /// ISO-8601 last-accessed timestamp.
    pub last_accessed: String,
    /// ISO-8601 last-validated timestamp.
    pub last_validated: String,
}

// ---------------------------------------------------------------------------
// See-Also Cross-References
// ---------------------------------------------------------------------------

/// The type of relationship between two catalog entries.
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
    Supersedes,
    /// The source is a narrower term of the target.
    NarrowerTerm,
    /// The source is a broader term of the target.
    BroaderTerm,
}

/// A single "see also" cross-reference link.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SeeAlsoLink {
    /// The item this link points to.
    pub target: ItemId,
    /// The kind of relationship.
    pub relationship: Relationship,
    /// Human-readable note explaining the connection.
    pub note: String,
}

// ---------------------------------------------------------------------------
// Circulation
// ---------------------------------------------------------------------------

/// Circulation record tracking how a catalog entry has been used.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CirculationRecord {
    /// Total number of times this item has been checked out (accessed).
    pub total_checkouts: u64,
    /// ISO-8601 timestamp of the most recent checkout.
    pub last_checkout: Option<String>,
    /// Contexts (task descriptions, branch names) in which the item was used.
    pub checkout_contexts: Vec<String>,
}

impl Default for CirculationRecord {
    fn default() -> Self {
        Self {
            total_checkouts: 0,
            last_checkout: None,
            checkout_contexts: Vec::new(),
        }
    }
}

/// A single circulation event — one checkout or check-in.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CirculationEvent {
    /// The item that was accessed.
    pub item_id: ItemId,
    /// The agent that accessed it.
    pub agent: AgentId,
    /// ISO-8601 timestamp of the event.
    pub timestamp: String,
    /// The context (task, branch, etc.) of the access.
    pub context: String,
    /// Whether this was a checkout (read) or check-in (update).
    pub event_type: CirculationEventType,
}

/// Whether circulation is a checkout or a check-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CirculationEventType {
    /// The item was read/retrieved.
    Checkout,
    /// The item was updated/returned with modifications.
    Checkin,
}

// ---------------------------------------------------------------------------
// Catalog Entry — the card in the card catalog
// ---------------------------------------------------------------------------

/// A single entry in the card catalog — the atomic unit of memory.
///
/// Each entry is classified by five systems simultaneously, linked to
/// other entries via "see also" references, and tracked by circulation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CatalogEntry {
    /// Unique identifier for this item.
    pub item_id: ItemId,
    /// The memory content — what the agent learned.
    pub content: String,
    /// Multi-system classification.
    pub classification: Classification,
    /// Cross-reference links to related entries.
    pub see_also: Vec<SeeAlsoLink>,
    /// Circulation tracking data.
    pub circulation: CirculationRecord,
    /// Time-to-live as a duration string, e.g. `"30d"`.
    pub ttl: String,
    /// Confidence score (0.0 to 1.0) in the accuracy of this memory.
    pub confidence: f64,
    /// Whether this entry has been deaccessioned (archived).
    pub deaccessioned: bool,
}

// ---------------------------------------------------------------------------
// Agent Identity
// ---------------------------------------------------------------------------

/// Agent identity — the library card that identifies who is operating.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentIdentity {
    /// Agent name.
    pub name: String,
    /// Organization the agent belongs to.
    pub organization: String,
    /// Functional role.
    pub role: String,
    /// What the agent can do.
    pub capabilities: Vec<String>,
    /// Authorization scope defining branch patterns, line limits, etc.
    pub authorization: AuthorizationScope,
    /// OpenWallet key reference for signing.
    pub openwallet_key_ref: Option<String>,
    /// ISO-8601 creation timestamp.
    pub created: String,
    /// Key rotation policy, e.g. `"90d"`.
    pub key_rotation_policy: String,
}

/// Authorization scope constraining what an agent may do.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AuthorizationScope {
    /// Branch name patterns the agent may commit to.
    pub branches: Vec<String>,
    /// Maximum number of lines in a single patch.
    pub max_patch_lines: Option<u32>,
    /// Collections (repos) the agent may operate on. `["*"]` means all.
    pub collections: Vec<String>,
}

// ---------------------------------------------------------------------------
// Catalog Configuration
// ---------------------------------------------------------------------------

/// Configuration for the card-catalog memory system, read from git config.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CatalogConfig {
    /// Git ref prefix for catalog storage.
    pub catalog_branch: String,
    /// Per-task token budget.
    pub token_budget: u64,
    /// Maximum subject headings per entry.
    pub max_subject_headings: usize,
    /// Maximum "see also" links per entry.
    pub max_see_also: usize,
    /// Enable circulation tracking.
    pub circulation_tracking: bool,
    /// Default TTL before deaccession review.
    pub deaccession_age: String,
    /// Maximum depth of call number hierarchy.
    pub call_number_depth: usize,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            catalog_branch: "refs/catalog/".to_string(),
            token_budget: 40_000,
            max_subject_headings: 3,
            max_see_also: 5,
            circulation_tracking: true,
            deaccession_age: "30d".to_string(),
            call_number_depth: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Token Budget
// ---------------------------------------------------------------------------

/// Token budget tracking with mandatory reserves for cataloging and
/// circulation — because an unclassified result is a lost result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CirculationBudget {
    /// Total tokens available for the task.
    pub total: u64,
    /// Tokens consumed so far.
    pub used: u64,
    /// Tokens reserved for post-task cataloging (never skipped).
    pub catalog_reserve: u64,
    /// Tokens reserved for PR creation and coordination (never skipped).
    pub circulation_reserve: u64,
}

impl CirculationBudget {
    /// Create a new budget with standard reserves.
    pub fn new(total: u64) -> Self {
        Self {
            total,
            used: 0,
            catalog_reserve: 1_500,
            circulation_reserve: 2_000,
        }
    }

    /// Tokens available for the current work phase (excludes reserves).
    pub fn available(&self) -> u64 {
        let reserved = self.catalog_reserve + self.circulation_reserve;
        self.total.saturating_sub(self.used).saturating_sub(reserved)
    }

    /// Record token usage. Returns an error if the budget is exhausted.
    pub fn spend(&mut self, tokens: u64) -> anyhow::Result<()> {
        if self.used + tokens > self.total {
            anyhow::bail!(
                "token budget exhausted: used={}, spending={}, total={}",
                self.used,
                tokens,
                self.total
            );
        }
        self.used += tokens;
        Ok(())
    }

    /// Whether the working budget is exhausted (only reserves remain).
    pub fn is_work_exhausted(&self) -> bool {
        self.available() == 0
    }

    /// Fraction of total budget consumed.
    pub fn utilization(&self) -> f64 {
        if self.total == 0 {
            return 1.0;
        }
        self.used as f64 / self.total as f64
    }
}

impl Default for CirculationBudget {
    fn default() -> Self {
        Self::new(40_000)
    }
}

// ---------------------------------------------------------------------------
// Task Lifecycle Phases
// ---------------------------------------------------------------------------

/// The five phases of the library acquisition lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum LifecyclePhase {
    /// Receive task description, branch metadata, PR context.
    Acquire,
    /// Retrieve relevant memories, classify task context.
    Classify,
    /// Produce INDEX.patch + COMMIT.msg.
    Shelve,
    /// Classify the new work as future memory.
    Catalog,
    /// Create PR, post coordination, track circulation.
    Circulate,
}

/// Progress report during a task lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LifecycleProgress {
    /// Current phase.
    pub phase: LifecyclePhase,
    /// Active agent.
    pub agent: AgentId,
    /// Token budget status.
    pub budget: CirculationBudget,
    /// Number of catalog entries retrieved so far.
    pub entries_retrieved: u32,
    /// Number of new catalog entries created.
    pub entries_created: u32,
}

// ---------------------------------------------------------------------------
// Shelf Output
// ---------------------------------------------------------------------------

/// The output of a shelving operation — a patch with catalog metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShelvedPatch {
    /// The unified diff content (INDEX.patch).
    pub patch: String,
    /// The commit message (COMMIT.msg).
    pub commit_message: String,
    /// Catalog metadata attached to the patch for classification.
    pub metadata: PatchMetadata,
}

/// Metadata attached to a shelved patch.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatchMetadata {
    /// Call number classifying the work performed.
    pub call_number: CallNumber,
    /// Subject headings describing the patch.
    pub subject_headings: Vec<String>,
    /// Agent that produced the patch.
    pub agent: AgentId,
    /// Tokens used during patch generation.
    pub tokens_used: u64,
}

// ---------------------------------------------------------------------------
// Controlled Vocabulary
// ---------------------------------------------------------------------------

/// A mapping from variant terms to canonical terms.
///
/// Without controlled vocabulary, the same concept gets classified under
/// "authentication", "auth", "login", "sign-in" — and searches for any
/// one term miss the others.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ControlledVocabulary {
    /// Mapping of variant term -> canonical term.
    pub mappings: Vec<VocabularyMapping>,
}

/// A single vocabulary mapping: a variant term and its canonical form.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VocabularyMapping {
    /// The variant or alias, e.g. "auth".
    pub variant: String,
    /// The canonical term, e.g. "authentication".
    pub canonical: String,
}

// ---------------------------------------------------------------------------
// Loan / Coordination Protocol
// ---------------------------------------------------------------------------

/// A structured coordination message following the catalog protocol.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CatalogMessage {
    /// Protocol version.
    pub protocol: String,
    /// Message type.
    pub message_type: CatalogMessageType,
    /// Sending agent.
    pub agent: AgentId,
    /// Collection (repo) the message originates from.
    pub collection: CollectionRef,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Call number classifying the message.
    pub call_number: CallNumber,
    /// Type-specific payload.
    pub payload: serde_json::Value,
}

/// The type of coordination message in the catalog protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CatalogMessageType {
    /// Request an item from another collection (cross-repo dependency).
    LoanRequest,
    /// Signal that a borrowed item has been processed.
    Return,
    /// Request priority on a pending item.
    Hold,
    /// Provide context without requesting action.
    Reference,
    /// Token budget status report.
    BudgetReport,
}

// ---------------------------------------------------------------------------
// Holds Queue
// ---------------------------------------------------------------------------

/// An item in the holds queue — a task waiting for a dependency.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hold {
    /// Unique identifier for the hold.
    pub hold_id: String,
    /// The item being waited on.
    pub item_id: ItemId,
    /// The collection that owns the item.
    pub collection: CollectionRef,
    /// The agent requesting the hold.
    pub requesting_agent: AgentId,
    /// Urgency level.
    pub urgency: HoldUrgency,
    /// Position in the hold queue.
    pub queue_position: u32,
    /// ISO-8601 timestamp when the hold was placed.
    pub placed_at: String,
    /// ISO-8601 due date, if any.
    pub due_date: Option<String>,
    /// Whether the hold has been satisfied.
    pub satisfied: bool,
}

/// Urgency level for a hold request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HoldUrgency {
    /// Normal priority.
    Normal,
    /// Expedited — blocking other work.
    Rush,
    /// Interlibrary loan — cross-repo dependency.
    InterlibraryLoan,
}

// ---------------------------------------------------------------------------
// Retrieval Scoring
// ---------------------------------------------------------------------------

/// Weights for the five retrieval scoring dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetrievalWeights {
    /// Weight for subject heading match.
    pub subject_match: f64,
    /// Weight for call number proximity.
    pub call_number_proximity: f64,
    /// Weight for "see also" graph distance.
    pub see_also_distance: f64,
    /// Weight for circulation frequency.
    pub circulation_frequency: f64,
    /// Weight for freshness (recency of validation).
    pub freshness: f64,
}

impl Default for RetrievalWeights {
    fn default() -> Self {
        Self {
            subject_match: 0.35,
            call_number_proximity: 0.25,
            see_also_distance: 0.20,
            circulation_frequency: 0.10,
            freshness: 0.10,
        }
    }
}

/// A scored catalog entry returned from a retrieval query.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoredEntry {
    /// The catalog entry.
    pub entry: CatalogEntry,
    /// The composite relevance score (0.0 to 1.0).
    pub score: f64,
    /// Breakdown of how the score was computed.
    pub score_breakdown: ScoreBreakdown,
}

/// Per-dimension score breakdown for a retrieval result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScoreBreakdown {
    pub subject_match: f64,
    pub call_number_proximity: f64,
    pub see_also_distance: f64,
    pub circulation_frequency: f64,
    pub freshness: f64,
}

// ---------------------------------------------------------------------------
// Reference Shelf (finding aid)
// ---------------------------------------------------------------------------

/// A curated set of memories provided to an agent before task execution.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReferenceShelf {
    /// The scored entries selected for this task.
    pub entries: Vec<ScoredEntry>,
    /// Natural-language finding aid summarizing the reference shelf.
    pub finding_aid: String,
}

// ---------------------------------------------------------------------------
// Loan Info (for forge adapter)
// ---------------------------------------------------------------------------

/// Summary information about a loan (PR).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoanInfo {
    pub loan_id: LoanId,
    pub collection: CollectionRef,
    pub title: String,
    pub state: LoanState,
    pub labels: Vec<String>,
}

/// State of a loan (PR).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoanState {
    Open,
    Closed,
    Merged,
}

/// Brief summary of a loan for search results.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoanSummary {
    pub loan_id: LoanId,
    pub title: String,
    pub state: LoanState,
}

/// A note (comment) on a loan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Note {
    pub note_id: NoteId,
    pub author: String,
    pub body: String,
    pub created_at: String,
}

/// Request to create a new loan (PR) or interlibrary loan.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoanRequest {
    /// Title for the loan.
    pub title: String,
    /// Body/description.
    pub body: String,
    /// The requesting collection (for interlibrary loans).
    pub requesting_collection: Option<CollectionRef>,
    /// Call number classifying the loan.
    pub call_number: CallNumber,
    /// Reason for the loan.
    pub reason: String,
    /// ISO-8601 due date, if any.
    pub due_date: Option<String>,
    /// "See also" references.
    pub see_also: Vec<CallNumber>,
}
