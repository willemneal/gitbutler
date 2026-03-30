//! PR comment schema for coordination messages.
//!
//! Messages are serialized as JSON inside markdown code fences tagged
//! `but-ai-message`. This dual encoding ensures both machines and humans
//! can read the protocol in PR comment threads.

use crate::types::{CoordinationMessage, MessageType};

/// Code fence tag used to identify coordination messages in PR comments.
const FENCE_TAG: &str = "but-ai-message";

/// Schema version for coordination messages.
pub const SCHEMA_VERSION: &str = "but-ai/coordination/v1";

/// Render a `CoordinationMessage` as a markdown PR comment with a JSON
/// code fence.
///
/// Format:
/// ````markdown
/// ```but-ai-message
/// { ... json ... }
/// ```
/// ````
pub fn render(msg: &CoordinationMessage) -> anyhow::Result<String> {
    let json = serde_json::to_string_pretty(msg)?;
    Ok(format!("```{FENCE_TAG}\n{json}\n```"))
}

/// Parse all `CoordinationMessage`s from a PR comment body.
///
/// A single comment may contain multiple code-fenced messages (though
/// typically there is only one). Non-message content is ignored.
pub fn parse(comment: &str) -> Vec<anyhow::Result<CoordinationMessage>> {
    let fence_open = format!("```{FENCE_TAG}");
    let mut results = Vec::new();
    let mut search_from = 0;

    while let Some(start) = comment[search_from..].find(&fence_open) {
        let abs_start = search_from + start;
        let json_start = abs_start + fence_open.len();

        // Skip optional newline after the fence tag
        let json_start = if comment[json_start..].starts_with('\n') {
            json_start + 1
        } else {
            json_start
        };

        if let Some(end_offset) = comment[json_start..].find("```") {
            let json_str = &comment[json_start..json_start + end_offset];
            results.push(
                serde_json::from_str::<CoordinationMessage>(json_str.trim())
                    .map_err(|e| anyhow::anyhow!("Failed to parse coordination message: {e}")),
            );
            search_from = json_start + end_offset + 3;
        } else {
            results.push(Err(anyhow::anyhow!(
                "Unterminated code fence for {FENCE_TAG}"
            )));
            break;
        }
    }

    results
}

/// Parse the first `CoordinationMessage` from a PR comment body, if any.
pub fn parse_first(comment: &str) -> Option<anyhow::Result<CoordinationMessage>> {
    parse(comment).into_iter().next()
}

/// Check whether a PR comment body contains at least one coordination message.
pub fn is_coordination_comment(comment: &str) -> bool {
    comment.contains(&format!("```{FENCE_TAG}"))
}

/// Extract the message type from a coordination message without full deserialization.
/// Returns `None` if the comment is not a coordination message.
pub fn peek_message_type(comment: &str) -> Option<MessageType> {
    let msg = parse_first(comment)?.ok()?;
    Some(msg.message_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentId, MessageType};

    fn sample_message() -> CoordinationMessage {
        CoordinationMessage {
            schema: SCHEMA_VERSION.to_string(),
            message_type: MessageType::StatusReport,
            from: AgentId("dara".to_string()),
            to: Some(AgentId("ines".to_string())),
            payload: serde_json::json!({
                "status": "in_progress",
                "files_changed": 3
            }),
            timestamp: "2026-03-29T14:00:00Z".to_string(),
        }
    }

    #[test]
    fn render_and_parse_round_trip() {
        let msg = sample_message();
        let rendered = render(&msg).unwrap();

        assert!(rendered.starts_with("```but-ai-message\n"));
        assert!(rendered.ends_with("\n```"));

        let parsed = parse_first(&rendered).unwrap().unwrap();
        assert_eq!(parsed.message_type, MessageType::StatusReport);
        assert_eq!(parsed.from.0, "dara");
        assert_eq!(parsed.to.unwrap().0, "ines");
        assert_eq!(parsed.timestamp, "2026-03-29T14:00:00Z");
    }

    #[test]
    fn non_message_comment_returns_empty() {
        assert!(parse("just a regular PR comment").is_empty());
        assert!(!is_coordination_comment("just a regular comment"));
    }

    #[test]
    fn multiple_messages_in_one_comment() {
        let msg1 = sample_message();
        let msg2 = CoordinationMessage {
            message_type: MessageType::BudgetReport,
            ..sample_message()
        };
        let comment = format!("{}\n\nSome text\n\n{}", render(&msg1).unwrap(), render(&msg2).unwrap());

        let parsed = parse(&comment);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0].as_ref().unwrap().message_type,
            MessageType::StatusReport
        );
        assert_eq!(
            parsed[1].as_ref().unwrap().message_type,
            MessageType::BudgetReport
        );
    }

    #[test]
    fn is_coordination_comment_detects_fence() {
        let rendered = render(&sample_message()).unwrap();
        assert!(is_coordination_comment(&rendered));
        assert!(!is_coordination_comment("no message here"));
    }

    #[test]
    fn peek_message_type_works() {
        let rendered = render(&sample_message()).unwrap();
        assert_eq!(
            peek_message_type(&rendered),
            Some(MessageType::StatusReport)
        );
        assert_eq!(peek_message_type("not a message"), None);
    }
}
