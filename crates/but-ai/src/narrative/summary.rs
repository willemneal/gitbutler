//! Arc compaction and summarization.
//!
//! When an arc goes dormant or the context window needs compaction, the
//! summarizer produces a condensed representation that preserves:
//! - Active motifs
//! - Unresolved tensions
//! - Key decisions and content
//!
//! This is the narrative equivalent of garbage collection: details are lost
//! but thematic structure is preserved.

use crate::types::{MemoryEntry, MotifId, TensionId, TensionRole};

use super::arc::Arc;

/// Result of summarizing an arc.
#[derive(Debug, Clone)]
pub struct ArcSummary {
    /// Title of the summarized arc.
    pub title: String,
    /// Number of entries that were compacted.
    pub entry_count: usize,
    /// Motifs preserved from the arc.
    pub motifs: Vec<MotifId>,
    /// Tension IDs that remain unresolved.
    pub unresolved_tensions: Vec<TensionId>,
    /// Concatenated summary text.
    pub summary_text: String,
    /// Compaction ratio: summary length / total content length.
    pub compaction_ratio: f64,
}

/// Produces summaries of arcs for compaction.
pub struct Summarizer {
    /// Maximum character length for summary text.
    max_summary_chars: usize,
}

impl Summarizer {
    /// Create a new summarizer with the given character limit.
    pub fn new(max_summary_chars: usize) -> Self {
        Self { max_summary_chars }
    }

    /// Summarize an arc's entries into a compacted representation.
    ///
    /// Preserves active motifs, unresolved tensions, and key content.
    pub fn summarize(&self, arc: &Arc, entries: &[&MemoryEntry]) -> ArcSummary {
        let motifs = Self::collect_motifs(entries);
        let unresolved_tensions = Self::collect_unresolved_tensions(entries);
        let (summary_text, compaction_ratio) = self.compose_summary(arc, entries);

        ArcSummary {
            title: arc.title.clone(),
            entry_count: entries.len(),
            motifs,
            unresolved_tensions,
            summary_text,
            compaction_ratio,
        }
    }

    /// Collect all unique motifs from a set of entries.
    fn collect_motifs(entries: &[&MemoryEntry]) -> Vec<MotifId> {
        let mut motifs: Vec<MotifId> = entries
            .iter()
            .flat_map(|e| e.motifs.iter().cloned())
            .collect();
        motifs.sort_by(|a, b| a.0.cmp(&b.0));
        motifs.dedup();
        motifs
    }

    /// Collect tension IDs that were introduced but not resolved within
    /// the given entries.
    fn collect_unresolved_tensions(entries: &[&MemoryEntry]) -> Vec<TensionId> {
        let mut introduced = Vec::new();
        let mut resolved = Vec::new();

        for entry in entries {
            for tref in &entry.tension_refs {
                match tref.role {
                    TensionRole::Introduced => {
                        introduced.push(tref.tension_id.clone());
                    }
                    TensionRole::Resolved => {
                        resolved.push(tref.tension_id.clone());
                    }
                    TensionRole::Referenced => {}
                }
            }
        }

        introduced
            .into_iter()
            .filter(|t| !resolved.contains(t))
            .collect()
    }

    /// Compose summary text from arc entries, respecting the character limit.
    /// Returns (summary_text, compaction_ratio).
    fn compose_summary(&self, arc: &Arc, entries: &[&MemoryEntry]) -> (String, f64) {
        let mut parts = Vec::new();

        // Arc header.
        parts.push(format!(
            "Arc '{}' ({} entries).",
            arc.title,
            entries.len()
        ));

        // Total original content length for compaction ratio.
        let total_content_len: usize = entries.iter().map(|e| e.content.len()).sum();

        // Budget per entry (excluding header).
        let header_len: usize = parts.iter().map(|p| p.len()).sum();
        let remaining = self.max_summary_chars.saturating_sub(header_len);
        let budget_per_entry = if entries.is_empty() {
            remaining
        } else {
            remaining / entries.len()
        };

        for entry in entries {
            let content = &entry.content;
            let truncated = if content.len() > budget_per_entry {
                format!(
                    "[{}] {}...",
                    entry.id,
                    &content[..budget_per_entry.saturating_sub(10).min(content.len())]
                )
            } else {
                format!("[{}] {}", entry.id, content)
            };
            parts.push(truncated);
        }

        let full_text = parts.join(" ");

        // Truncate to limit.
        let summary = if full_text.len() > self.max_summary_chars {
            format!("{}...", &full_text[..self.max_summary_chars.saturating_sub(3)])
        } else {
            full_text
        };

        let compaction_ratio = if total_content_len == 0 {
            0.0
        } else {
            summary.len() as f64 / total_content_len as f64
        };

        (summary, compaction_ratio)
    }
}

impl Default for Summarizer {
    fn default() -> Self {
        // Default: ~2000 chars (~500 tokens).
        Self::new(2000)
    }
}
