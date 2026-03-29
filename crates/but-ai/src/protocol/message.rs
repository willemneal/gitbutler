//! Structured PR comment messages for inter-agent coordination.
//!
//! Every agent-to-agent message is a PR comment with a structured header
//! encoded as HTML comments (machine-parseable) and Markdown body (human-readable).
//! This dual encoding ensures both machines and humans can read the protocol.

use crate::types::{
    AgentId, MessageId, MessageTarget, MessageType, PrRef, TaskStatus, TideMark, TokenUsage,
};

/// A structured protocol message, carried as a PR comment.
#[derive(Debug, Clone)]
pub struct ProtocolMessage {
    pub id: MessageId,
    pub message_type: MessageType,
    pub from: AgentId,
    pub to: MessageTarget,
    pub tide_mark: TideMark,
    pub body: MessageBody,
}

/// The body of a protocol message, varying by message type.
#[derive(Debug, Clone)]
pub enum MessageBody {
    TaskAssignment(TaskAssignmentBody),
    StatusReport(StatusReportBody),
    DependencyDeclaration(DependencyDeclarationBody),
    PatchHandoff(PatchHandoffBody),
    BudgetReport(BudgetReportBody),
    ConsensusRequest(ConsensusRequestBody),
    ConsensusVote(ConsensusVoteBody),
}

/// Body for a task assignment message.
#[derive(Debug, Clone)]
pub struct TaskAssignmentBody {
    pub task_description: String,
    pub branch: String,
    pub dependencies: Vec<PrRef>,
    pub estimated_tokens: Option<u64>,
}

/// Body for a status report message.
#[derive(Debug, Clone)]
pub struct StatusReportBody {
    pub status: TaskStatus,
    pub summary: String,
    pub tokens_used: TokenUsage,
    pub tokens_budget: u64,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub dependencies_on: Vec<PrRef>,
    pub blocks: Vec<PrRef>,
}

/// Body for a dependency declaration message.
#[derive(Debug, Clone)]
pub struct DependencyDeclarationBody {
    pub depends_on: Vec<PrRef>,
    pub blocks: Vec<PrRef>,
    pub reason: String,
}

/// Body for a patch handoff message.
#[derive(Debug, Clone)]
pub struct PatchHandoffBody {
    pub patch_content: String,
    pub commit_message: String,
    pub partial: bool,
    pub skipped_steps: Vec<String>,
}

/// Body for a budget report message.
#[derive(Debug, Clone)]
pub struct BudgetReportBody {
    pub budget_total: u64,
    pub used: TokenUsage,
    pub remaining: u64,
    pub phase: String,
    pub checkpoint: String,
}

/// Body for a consensus request message.
#[derive(Debug, Clone)]
pub struct ConsensusRequestBody {
    pub proposal: String,
    pub options: Vec<String>,
    pub deadline_cycle: u64,
}

/// Body for a consensus vote message.
#[derive(Debug, Clone)]
pub struct ConsensusVoteBody {
    pub request_id: MessageId,
    pub approve: bool,
    pub reason: Option<String>,
}

impl ProtocolMessage {
    /// Render this message as a PR comment string with HTML comment headers
    /// and Markdown body.
    pub fn render(&self) -> String {
        let mut output = String::new();

        // Machine-readable HTML comment headers
        output.push_str("<!-- but-ai:message -->\n");
        output.push_str(&format!(
            "<!-- type: {} -->\n",
            self.message_type_str()
        ));
        output.push_str(&format!("<!-- from: {} -->\n", self.from));
        output.push_str(&format!("<!-- to: {} -->\n", self.target_str()));
        output.push_str(&format!(
            "<!-- timestamp: {} -->\n",
            self.tide_mark.timestamp
        ));
        output.push_str(&format!("<!-- tide: {} -->\n", self.tide_mark.phase));
        output.push('\n');

        // Human-readable Markdown body
        match &self.body {
            MessageBody::TaskAssignment(body) => {
                output.push_str(&format!(
                    "## [TASK-ASSIGNMENT] {}\n\n",
                    truncate(&body.task_description, 60)
                ));
                output.push_str(&format!("**Branch:** `{}`\n", body.branch));
                if !body.dependencies.is_empty() {
                    output.push_str("\n### Dependencies\n");
                    for dep in &body.dependencies {
                        output.push_str(&format!("- {}\n", dep));
                    }
                }
                if let Some(est) = body.estimated_tokens {
                    output.push_str(&format!(
                        "\n**Estimated tokens:** {}\n",
                        format_tokens(est)
                    ));
                }
            }
            MessageBody::StatusReport(body) => {
                output.push_str(&format!(
                    "## [STATUS-REPORT] {}\n\n",
                    truncate(&body.summary, 60)
                ));
                output.push_str(&format!("**Status:** {:?}\n", body.status));
                output.push_str(&format!("**Agent:** {}\n", self.from));
                output.push_str(&format!(
                    "**Tokens used:** {} / {}\n",
                    format_tokens(body.tokens_used.total()),
                    format_tokens(body.tokens_budget)
                ));
                output.push_str(&format!(
                    "\n### Summary\n{}\n{} files changed, {} insertions, {} deletions.\n",
                    body.summary, body.files_changed, body.insertions, body.deletions
                ));
                if !body.dependencies_on.is_empty() || !body.blocks.is_empty() {
                    output.push_str("\n### Dependencies\n");
                    for dep in &body.dependencies_on {
                        output.push_str(&format!("- Depends on: {}\n", dep));
                    }
                    for blk in &body.blocks {
                        output.push_str(&format!("- Blocks: {}\n", blk));
                    }
                }
            }
            MessageBody::DependencyDeclaration(body) => {
                output.push_str("## [DEPENDENCY-DECLARATION]\n\n");
                output.push_str(&format!("**Reason:** {}\n", body.reason));
                for dep in &body.depends_on {
                    output.push_str(&format!("- Depends on: {}\n", dep));
                }
                for blk in &body.blocks {
                    output.push_str(&format!("- Blocks: {}\n", blk));
                }
            }
            MessageBody::PatchHandoff(body) => {
                output.push_str("## [PATCH-HANDOFF]\n\n");
                if body.partial {
                    output.push_str("**PARTIAL** -- not all steps completed.\n\n");
                    if !body.skipped_steps.is_empty() {
                        output.push_str("### Skipped Steps\n");
                        for step in &body.skipped_steps {
                            output.push_str(&format!("- {}\n", step));
                        }
                        output.push('\n');
                    }
                }
                output.push_str(&format!(
                    "### Commit Message\n```\n{}\n```\n",
                    body.commit_message
                ));
                output.push_str(&format!(
                    "\n### Patch\n```diff\n{}\n```\n",
                    body.patch_content
                ));
            }
            MessageBody::BudgetReport(body) => {
                output.push_str("## [BUDGET-REPORT]\n\n");
                output.push_str(&format!(
                    "**Budget:** {} / {} (remaining: {})\n",
                    format_tokens(body.used.total()),
                    format_tokens(body.budget_total),
                    format_tokens(body.remaining)
                ));
                output.push_str(&format!("**Phase:** {}\n", body.phase));
                output.push_str(&format!("**Checkpoint:** {}\n", body.checkpoint));
            }
            MessageBody::ConsensusRequest(body) => {
                output.push_str("## [CONSENSUS-REQUEST]\n\n");
                output.push_str(&format!("**Proposal:** {}\n\n", body.proposal));
                if !body.options.is_empty() {
                    output.push_str("### Options\n");
                    for (i, opt) in body.options.iter().enumerate() {
                        output.push_str(&format!("{}. {}\n", i + 1, opt));
                    }
                }
                output.push_str(&format!(
                    "\n**Deadline:** cycle {}\n",
                    body.deadline_cycle
                ));
            }
            MessageBody::ConsensusVote(body) => {
                let verdict = if body.approve { "APPROVE" } else { "REJECT" };
                output.push_str(&format!("## [CONSENSUS-VOTE] {}\n\n", verdict));
                output.push_str(&format!("**In response to:** {}\n", body.request_id));
                if let Some(reason) = &body.reason {
                    output.push_str(&format!("**Reason:** {}\n", reason));
                }
            }
        }

        output
    }

    /// Parse a PR comment string back into a ProtocolMessage.
    /// Returns `None` if the comment does not contain the `but-ai:message` marker.
    pub fn parse(comment: &str) -> Option<ParsedHeader> {
        if !comment.contains("<!-- but-ai:message -->") {
            return None;
        }

        let mut header = ParsedHeader::default();

        for line in comment.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("<!-- type: ") {
                if let Some(val) = rest.strip_suffix(" -->") {
                    header.message_type = Some(val.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("<!-- from: ") {
                if let Some(val) = rest.strip_suffix(" -->") {
                    header.from = Some(AgentId(val.to_string()));
                }
            } else if let Some(rest) = line.strip_prefix("<!-- to: ") {
                if let Some(val) = rest.strip_suffix(" -->") {
                    header.to = Some(val.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("<!-- timestamp: ") {
                if let Some(val) = rest.strip_suffix(" -->") {
                    header.timestamp = Some(val.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("<!-- tide: ") {
                if let Some(val) = rest.strip_suffix(" -->") {
                    header.tide = Some(val.to_string());
                }
            }
        }

        Some(header)
    }

    fn message_type_str(&self) -> &'static str {
        match self.body {
            MessageBody::TaskAssignment(_) => "task-assignment",
            MessageBody::StatusReport(_) => "status-report",
            MessageBody::DependencyDeclaration(_) => "dependency-declaration",
            MessageBody::PatchHandoff(_) => "patch-handoff",
            MessageBody::BudgetReport(_) => "budget-report",
            MessageBody::ConsensusRequest(_) => "consensus-request",
            MessageBody::ConsensusVote(_) => "consensus-vote",
        }
    }

    fn target_str(&self) -> String {
        match &self.to {
            MessageTarget::Agent(id) => id.to_string(),
            MessageTarget::Broadcast => "broadcast".to_string(),
        }
    }
}

/// Parsed header fields from a PR comment. Not all fields are always present.
#[derive(Debug, Clone, Default)]
pub struct ParsedHeader {
    pub message_type: Option<String>,
    pub from: Option<AgentId>,
    pub to: Option<String>,
    pub timestamp: Option<String>,
    pub tide: Option<String>,
}

impl ParsedHeader {
    /// Whether this header indicates a but-ai protocol message.
    pub fn is_protocol_message(&self) -> bool {
        self.message_type.is_some() && self.from.is_some()
    }
}

/// Truncate a string to at most `max` characters, adding "..." if truncated.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.min(s.len())])
    }
}

/// Format a token count with commas for readability.
fn format_tokens(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TidePhase;

    #[test]
    fn render_and_parse_round_trip() {
        let msg = ProtocolMessage {
            id: MessageId("msg-001".to_string()),
            message_type: MessageType::StatusReport,
            from: AgentId("dara".to_string()),
            to: MessageTarget::Broadcast,
            tide_mark: TideMark {
                timestamp: "2026-03-28T14:00:00Z".to_string(),
                phase: TidePhase::High,
                cycle: 1234,
                phase_elapsed_seconds: 100,
            },
            body: MessageBody::StatusReport(StatusReportBody {
                status: crate::types::TaskStatus::Completed,
                summary: "Auth module refactored".to_string(),
                tokens_used: TokenUsage {
                    input: 12000,
                    output: 400,
                },
                tokens_budget: 50000,
                files_changed: 3,
                insertions: 142,
                deletions: 87,
                dependencies_on: vec![],
                blocks: vec![],
            }),
        };

        let rendered = msg.render();
        assert!(rendered.contains("<!-- but-ai:message -->"));
        assert!(rendered.contains("<!-- type: status-report -->"));
        assert!(rendered.contains("<!-- from: dara -->"));

        let parsed = ProtocolMessage::parse(&rendered).unwrap();
        assert_eq!(parsed.message_type.as_deref(), Some("status-report"));
        assert_eq!(parsed.from.as_ref().unwrap().0, "dara");
        assert_eq!(parsed.tide.as_deref(), Some("high"));
    }

    #[test]
    fn non_protocol_comment_returns_none() {
        assert!(ProtocolMessage::parse("just a regular comment").is_none());
    }

    #[test]
    fn format_tokens_with_commas() {
        assert_eq!(format_tokens(1234), "1,234");
        assert_eq!(format_tokens(50000), "50,000");
        assert_eq!(format_tokens(100), "100");
    }
}
