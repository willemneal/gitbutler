//! Patch generation with catalog metadata.
//!
//! The Shelver produces `INDEX.patch` + `COMMIT.msg` artifacts.
//! Every patch carries catalog metadata: the call number of the work
//! performed, subject headings, and the agent that produced it.

use crate::types::{AgentId, CallNumber, PatchMetadata, ShelvedPatch};

/// Build a shelved patch from raw diff content and a commit message.
///
/// Attaches catalog metadata so the patch can be classified when
/// it is committed.
pub fn build_patch(
    patch_content: String,
    commit_message: String,
    call_number: CallNumber,
    subject_headings: Vec<String>,
    agent: AgentId,
    tokens_used: u64,
) -> ShelvedPatch {
    ShelvedPatch {
        patch: patch_content,
        commit_message,
        metadata: PatchMetadata {
            call_number,
            subject_headings,
            agent,
            tokens_used,
        },
    }
}

/// Generate a structured commit message in card-catalog style.
///
/// Format:
/// ```text
/// <subject line: what changed>
///
/// <body: why it changed>
///
/// Call-Number: ARCH.AUTH.MIDDLEWARE
/// Subject: authentication, jwt, middleware
/// Agent: shelver@shelfos
/// ```
pub fn format_commit_message(
    subject: &str,
    body: &str,
    call_number: &CallNumber,
    subject_headings: &[String],
    agent: &AgentId,
) -> String {
    let mut msg = String::new();

    // Subject line.
    msg.push_str(subject);
    msg.push_str("\n\n");

    // Body.
    if !body.is_empty() {
        msg.push_str(body);
        msg.push_str("\n\n");
    }

    // Catalog trailers.
    msg.push_str(&format!("Call-Number: {}\n", call_number));
    if !subject_headings.is_empty() {
        msg.push_str(&format!("Subject: {}\n", subject_headings.join(", ")));
    }
    msg.push_str(&format!("Agent: {}\n", agent));

    msg
}

/// Validate a patch: check that it is non-empty and looks like a unified diff.
pub fn validate_patch(patch: &str) -> anyhow::Result<PatchValidation> {
    if patch.is_empty() {
        anyhow::bail!("patch is empty");
    }

    let lines: Vec<&str> = patch.lines().collect();
    let additions = lines.iter().filter(|l| l.starts_with('+')).count();
    let deletions = lines.iter().filter(|l| l.starts_with('-')).count();
    let hunks = lines.iter().filter(|l| l.starts_with("@@")).count();

    if hunks == 0 {
        anyhow::bail!("patch contains no diff hunks");
    }

    Ok(PatchValidation {
        lines: lines.len(),
        additions,
        deletions,
        hunks,
    })
}

/// Summary statistics from patch validation.
#[derive(Debug, Clone)]
pub struct PatchValidation {
    /// Total lines in the patch.
    pub lines: usize,
    /// Number of addition lines.
    pub additions: usize,
    /// Number of deletion lines.
    pub deletions: usize,
    /// Number of diff hunks.
    pub hunks: usize,
}

impl PatchValidation {
    /// Net change in lines (additions - deletions).
    pub fn net_change(&self) -> isize {
        self.additions as isize - self.deletions as isize
    }
}

/// Estimate the token cost of a patch based on line count.
///
/// Rough heuristic: ~4 tokens per line for code, ~2 tokens per line
/// for context. We assume 60% of patch lines are context.
pub fn estimate_patch_tokens(patch: &str) -> u64 {
    let total_lines = patch.lines().count() as u64;
    let code_lines = (total_lines as f64 * 0.4).ceil() as u64;
    let context_lines = total_lines - code_lines;
    code_lines * 4 + context_lines * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PATCH: &str = "\
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,5 @@
 fn main() {
+    println!(\"hello\");
+    println!(\"world\");
 }
";

    #[test]
    fn validate_good_patch() {
        let result = validate_patch(SAMPLE_PATCH).unwrap();
        assert_eq!(result.hunks, 1);
        assert_eq!(result.additions, 2);
        assert_eq!(result.net_change(), 2);
    }

    #[test]
    fn validate_empty_patch() {
        assert!(validate_patch("").is_err());
    }

    #[test]
    fn validate_no_hunks() {
        assert!(validate_patch("just some text").is_err());
    }

    #[test]
    fn commit_message_format() {
        let msg = format_commit_message(
            "Add JWT token validation middleware",
            "Validates RS256 tokens with 15-minute expiry.",
            &CallNumber::parse("ARCH.AUTH.MIDDLEWARE"),
            &["authentication".into(), "jwt".into()],
            &AgentId("shelver".into()),
        );
        assert!(msg.contains("Call-Number: ARCH.AUTH.MIDDLEWARE"));
        assert!(msg.contains("Subject: authentication, jwt"));
        assert!(msg.contains("Agent: shelver"));
    }
}
