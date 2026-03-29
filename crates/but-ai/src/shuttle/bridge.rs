//! LoomBridge trait and shuttle message types.
//!
//! The bridge maps forge operations (PR comments, labels) to loom concepts
//! (shuttle messages, coordination patterns). Each forge backend implements
//! `LoomBridge` to translate between the two worlds.

use crate::types::{AgentId, MessageId, PrId, RepoRef, WeavePattern};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The type of a shuttle message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShuttleMessageType {
    Task,
    Status,
    Dependency,
    Handoff,
    Budget,
}

/// Origin or destination of a shuttle message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LoomEndpoint {
    pub agent: AgentId,
    pub org: String,
    pub repo: RepoRef,
}

/// Thread context carried by a shuttle message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShuttleThread {
    pub task_ref: String,
    pub status: String,
    pub dependencies: Vec<String>,
    pub budget_remaining: u64,
    pub pattern_complexity: WeavePattern,
}

/// A message carried by the shuttle between looms (repos).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShuttleMessage {
    pub schema: String,
    pub message_type: ShuttleMessageType,
    pub from_loom: LoomEndpoint,
    pub to_loom: LoomEndpoint,
    pub thread: ShuttleThread,
    pub timestamp: String,
}

impl ShuttleMessage {
    /// Schema version for shuttle messages.
    pub const SCHEMA_VERSION: &'static str = "but-ai/shuttle/v1";

    /// Serialize this message to the markdown code-fence format for PR comments.
    pub fn to_pr_comment(&self) -> anyhow::Result<String> {
        let json = serde_json::to_string_pretty(self)?;
        Ok(format!("```but-ai-shuttle\n{}\n```", json))
    }

    /// Parse a shuttle message from a PR comment body.
    pub fn from_pr_comment(comment: &str) -> anyhow::Result<Self> {
        let start = comment
            .find("```but-ai-shuttle\n")
            .ok_or_else(|| anyhow::anyhow!("No shuttle message found in comment"))?;
        let json_start = start + "```but-ai-shuttle\n".len();
        let end = comment[json_start..]
            .find("```")
            .ok_or_else(|| anyhow::anyhow!("Unterminated shuttle message"))?;
        let json_str = &comment[json_start..json_start + end];
        Ok(serde_json::from_str(json_str)?)
    }
}

/// Status of a fabric (PR) being tracked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FabricStatus {
    Weaving,
    Inspecting,
    Complete,
    Merged,
    Closed,
}

/// Coordination pattern attached to a PR for cross-repo tracking.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoordinationPattern {
    pub dependencies: Vec<PrId>,
    pub dependents: Vec<PrId>,
    pub weave_pattern: WeavePattern,
    pub expected_completion: Option<String>,
}

/// The loom-to-loom bridge trait. Maps forge operations to textile concepts.
///
/// Implementations wrap forge API calls. `InMemoryBridge` is provided for testing;
/// production implementations (GitHub, GitLab, etc.) translate each method to
/// the corresponding forge API endpoint.
pub trait LoomBridge: Send + Sync {
    /// Send a shuttle message to a target repo (creates a PR comment).
    fn send_shuttle(
        &self,
        target: &RepoRef,
        message: &ShuttleMessage,
    ) -> anyhow::Result<MessageId>;

    /// Receive shuttle messages since a given timestamp (reads PR comments).
    fn receive_shuttles(&self, since: &str) -> anyhow::Result<Vec<ShuttleMessage>>;

    /// Track the status of a fabric (PR).
    fn track_fabric(&self, pr: &PrId) -> anyhow::Result<FabricStatus>;

    /// Attach a coordination pattern to a PR (labels/metadata).
    fn attach_pattern(&self, pr: &PrId, pattern: &CoordinationPattern) -> anyhow::Result<()>;

    /// List connected looms (repos) for a given PR.
    fn list_connected_looms(&self, pr: &PrId) -> anyhow::Result<Vec<RepoRef>>;
}

/// In-memory bridge for testing. Stores messages and fabric statuses in vectors
/// behind interior mutability so tests can inspect state after operations.
pub struct InMemoryBridge {
    sent: std::sync::Mutex<Vec<(RepoRef, ShuttleMessage)>>,
    inbox: std::sync::Mutex<Vec<ShuttleMessage>>,
    fabric_statuses: std::sync::Mutex<Vec<(PrId, FabricStatus)>>,
    patterns: std::sync::Mutex<Vec<(PrId, CoordinationPattern)>>,
    connected_looms: std::sync::Mutex<Vec<(PrId, Vec<RepoRef>)>>,
    next_message_id: std::sync::Mutex<u64>,
}

impl InMemoryBridge {
    pub fn new() -> Self {
        Self {
            sent: std::sync::Mutex::new(Vec::new()),
            inbox: std::sync::Mutex::new(Vec::new()),
            fabric_statuses: std::sync::Mutex::new(Vec::new()),
            patterns: std::sync::Mutex::new(Vec::new()),
            connected_looms: std::sync::Mutex::new(Vec::new()),
            next_message_id: std::sync::Mutex::new(1),
        }
    }

    /// Enqueue a message into the inbox for `receive_shuttles` to return.
    pub fn enqueue_message(&self, msg: ShuttleMessage) {
        self.inbox.lock().unwrap().push(msg);
    }

    /// Set the tracked status for a PR.
    pub fn set_fabric_status(&self, pr: PrId, status: FabricStatus) {
        let mut statuses = self.fabric_statuses.lock().unwrap();
        if let Some(existing) = statuses.iter_mut().find(|(p, _)| p == &pr) {
            existing.1 = status;
        } else {
            statuses.push((pr, status));
        }
    }

    /// Set connected looms for a PR.
    pub fn set_connected_looms(&self, pr: PrId, repos: Vec<RepoRef>) {
        let mut looms = self.connected_looms.lock().unwrap();
        if let Some(existing) = looms.iter_mut().find(|(p, _)| p == &pr) {
            existing.1 = repos;
        } else {
            looms.push((pr, repos));
        }
    }

    /// Get all sent messages (for test assertions).
    pub fn sent_messages(&self) -> Vec<(RepoRef, ShuttleMessage)> {
        self.sent.lock().unwrap().clone()
    }

    /// Get all attached patterns (for test assertions).
    pub fn attached_patterns(&self) -> Vec<(PrId, CoordinationPattern)> {
        self.patterns.lock().unwrap().clone()
    }
}

impl LoomBridge for InMemoryBridge {
    fn send_shuttle(
        &self,
        target: &RepoRef,
        message: &ShuttleMessage,
    ) -> anyhow::Result<MessageId> {
        self.sent
            .lock()
            .unwrap()
            .push((target.clone(), message.clone()));
        let mut id = self.next_message_id.lock().unwrap();
        let msg_id = MessageId(format!("msg-{}", *id));
        *id += 1;
        Ok(msg_id)
    }

    fn receive_shuttles(&self, since: &str) -> anyhow::Result<Vec<ShuttleMessage>> {
        let inbox = self.inbox.lock().unwrap();
        let messages: Vec<ShuttleMessage> = inbox
            .iter()
            .filter(|msg| msg.timestamp.as_str() >= since)
            .cloned()
            .collect();
        Ok(messages)
    }

    fn track_fabric(&self, pr: &PrId) -> anyhow::Result<FabricStatus> {
        let statuses = self.fabric_statuses.lock().unwrap();
        statuses
            .iter()
            .find(|(p, _)| p == pr)
            .map(|(_, s)| s.clone())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No fabric status tracked for PR {}#{}",
                    pr.repo,
                    pr.number
                )
            })
    }

    fn attach_pattern(&self, pr: &PrId, pattern: &CoordinationPattern) -> anyhow::Result<()> {
        self.patterns
            .lock()
            .unwrap()
            .push((pr.clone(), pattern.clone()));
        Ok(())
    }

    fn list_connected_looms(&self, pr: &PrId) -> anyhow::Result<Vec<RepoRef>> {
        let looms = self.connected_looms.lock().unwrap();
        Ok(looms
            .iter()
            .find(|(p, _)| p == pr)
            .map(|(_, repos)| repos.clone())
            .unwrap_or_default())
    }
}
