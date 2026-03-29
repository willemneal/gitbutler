//! Commit message composition for the fabric output module.
//!
//! Produces COMMIT.msg content with structured loom metadata trailers
//! following the thread-count convention: warp:N, weft:M.

use crate::types::FabricMetadata;

/// Loom trailer data for a commit message.
#[derive(Debug, Clone)]
pub struct LoomTrailer {
    pub thread_count: String,
    pub weave_pattern: String,
    pub inspected_by: Option<String>,
}

impl LoomTrailer {
    pub fn from_metadata(metadata: &FabricMetadata) -> Self {
        Self {
            thread_count: metadata.thread_count.clone(),
            weave_pattern: format!("{:?}", metadata.weave_pattern).to_lowercase(),
            inspected_by: metadata.inspected_by.as_ref().map(|a| a.0.clone()),
        }
    }

    /// Format as git commit trailer lines.
    pub fn to_trailer_lines(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Thread-count: {}", self.thread_count));
        lines.push(format!("Weave-pattern: {}", self.weave_pattern));
        if let Some(ref inspector) = self.inspected_by {
            lines.push(format!("Inspected-by: {}", inspector));
        }
        lines.join("\n")
    }

    /// Parse thread count into (warp, weft) counts.
    ///
    /// Expects format "NW/MF" where N is warp count and M is weft count.
    pub fn parse_thread_count(&self) -> Option<(u32, u32)> {
        let parts: Vec<&str> = self.thread_count.split('/').collect();
        if parts.len() != 2 {
            return None;
        }
        let warp = parts[0].strip_suffix('W')?.parse().ok()?;
        let weft = parts[1].strip_suffix('F')?.parse().ok()?;
        Some((warp, weft))
    }
}

/// Builder for commit messages with loom metadata.
///
/// Composes a commit message with a title line, optional body paragraphs,
/// and structured loom trailers (thread-count, weave-pattern, inspected-by).
///
/// The resulting format:
/// ```text
/// <title>
///
/// <body paragraph(s)>
///
/// Thread-count: 3W/5F
/// Weave-pattern: twill
/// Inspected-by: lindqvist
/// ```
pub struct CommitMessageBuilder {
    description: Option<String>,
    body: Vec<String>,
    trailer: Option<LoomTrailer>,
    extra_trailers: Vec<(String, String)>,
}

impl CommitMessageBuilder {
    pub fn new() -> Self {
        Self {
            description: None,
            body: Vec::new(),
            trailer: None,
            extra_trailers: Vec::new(),
        }
    }

    /// Set the commit message title/description (first line).
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    /// Add a body paragraph to the commit message.
    pub fn with_body(mut self, paragraph: &str) -> Self {
        self.body.push(paragraph.to_string());
        self
    }

    /// Set the loom trailer.
    pub fn with_trailer(mut self, trailer: LoomTrailer) -> Self {
        self.trailer = Some(trailer);
        self
    }

    /// Add an extra key-value trailer line (e.g. "Co-authored-by").
    pub fn with_extra_trailer(mut self, key: &str, value: &str) -> Self {
        self.extra_trailers.push((key.to_string(), value.to_string()));
        self
    }

    /// Build the final commit message with title, body, and loom trailers.
    pub fn build(self) -> anyhow::Result<String> {
        let desc = self
            .description
            .ok_or_else(|| anyhow::anyhow!("Commit message requires a description"))?;

        let mut message = desc;

        // Append body paragraphs.
        if !self.body.is_empty() {
            message.push_str("\n\n");
            message.push_str(&self.body.join("\n\n"));
        }

        // Append trailers section (blank line separator before trailers).
        let has_trailers = self.trailer.is_some() || !self.extra_trailers.is_empty();
        if has_trailers {
            message.push_str("\n\n");

            let mut trailer_lines = Vec::new();
            if let Some(trailer) = self.trailer {
                trailer_lines.push(trailer.to_trailer_lines());
            }
            for (key, value) in &self.extra_trailers {
                trailer_lines.push(format!("{key}: {value}"));
            }
            message.push_str(&trailer_lines.join("\n"));
        }

        Ok(message)
    }
}

impl Default for CommitMessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compose a commit message from structured parts using the org convention.
///
/// This is a convenience function that builds a full COMMIT.msg with
/// thread-count metadata in the `warp:N weft:M` annotation style.
pub fn compose_commit_message(
    title: &str,
    body: Option<&str>,
    warp_count: u32,
    weft_count: u32,
    pattern: &str,
    inspector: Option<&str>,
) -> String {
    let mut msg = title.to_string();

    if let Some(body_text) = body {
        msg.push_str("\n\n");
        msg.push_str(body_text);
    }

    msg.push_str("\n\n");
    msg.push_str(&format!("Thread-count: {warp_count}W/{weft_count}F"));
    msg.push_str(&format!("\nWeave-pattern: {pattern}"));

    if let Some(inspector_name) = inspector {
        msg.push_str(&format!("\nInspected-by: {inspector_name}"));
    }

    msg
}
