//! Arc summarization and compression.
//!
//! When the context window is compacted or an arc goes dormant, the narrative
//! engine produces chapter summaries -- concise representations that preserve
//! motifs, tensions, and thematic content even when specific details are lost.
//!
//! The compaction process is analogous to summarizing a novel: you lose the
//! prose but keep the plot, the characters, and the themes.

use crate::types::{Arc, ArcId, ArcState, ArcSummary, Chapter, MotifId, TensionId};

/// Produces summaries of arcs and chapters for compaction.
pub struct Summarizer {
    /// Maximum tokens (approximated as words) per summary.
    max_summary_tokens: u64,
}

impl Summarizer {
    /// Create a new summarizer with the given token limit.
    pub fn new(max_summary_tokens: u64) -> Self {
        Self { max_summary_tokens }
    }

    /// Summarize an arc's chapters into a compressed representation.
    ///
    /// The summary preserves:
    /// 1. Active motifs (always preserved)
    /// 2. Unresolved tensions (always preserved)
    /// 3. A narrative summary of the arc's development
    /// 4. Key character (file) involvement
    pub fn summarize_arc(
        &self,
        arc: &Arc,
        chapters: &[&Chapter],
        timestamp: &str,
    ) -> ArcSummary {
        let motifs = Self::collect_motifs(chapters);
        let tensions = Self::collect_active_tensions(chapters);
        let summary_text = self.compose_summary(arc, chapters);

        ArcSummary {
            arc_id: arc.arc_id.clone(),
            title: arc.title.clone(),
            chapter_count: chapters.len() as u64,
            motifs,
            active_tensions: tensions,
            summary_text,
            summarized_at: timestamp.to_string(),
        }
    }

    /// Produce a compacted memory representation from multiple arcs.
    ///
    /// Returns a list of arc summaries suitable for inclusion in a reduced
    /// context window. Active arcs get fuller summaries; dormant arcs get
    /// minimal summaries preserving only motifs and tensions.
    pub fn compact(
        &self,
        arcs: &[(&Arc, Vec<&Chapter>)],
        timestamp: &str,
    ) -> Vec<ArcSummary> {
        arcs.iter()
            .map(|(arc, chapters)| {
                self.summarize_arc(arc, chapters, timestamp)
            })
            .collect()
    }

    /// Collect all unique motifs from a set of chapters.
    fn collect_motifs(chapters: &[&Chapter]) -> Vec<MotifId> {
        let mut motifs: Vec<MotifId> = chapters
            .iter()
            .flat_map(|c| c.motifs.iter().cloned())
            .collect();
        motifs.sort_by(|a, b| a.0.cmp(&b.0));
        motifs.dedup();
        motifs
    }

    /// Collect tension IDs that were introduced but not resolved in the given chapters.
    fn collect_active_tensions(chapters: &[&Chapter]) -> Vec<TensionId> {
        let introduced: Vec<TensionId> = chapters
            .iter()
            .flat_map(|c| c.tensions_introduced.iter().map(|t| t.id.clone()))
            .collect();

        let resolved: Vec<TensionId> = chapters
            .iter()
            .flat_map(|c| c.tensions_resolved.iter().cloned())
            .collect();

        introduced
            .into_iter()
            .filter(|t| !resolved.contains(t))
            .collect()
    }

    /// Compose a narrative summary text for an arc, respecting the token limit.
    fn compose_summary(&self, arc: &Arc, chapters: &[&Chapter]) -> String {
        let mut parts = Vec::new();

        // Arc overview.
        parts.push(format!(
            "Arc '{}' ({} chapters).",
            arc.title,
            chapters.len()
        ));

        // Key characters (files) involved.
        let mut all_characters: Vec<String> = chapters
            .iter()
            .flat_map(|c| c.characters.iter().cloned())
            .collect();
        all_characters.sort();
        all_characters.dedup();
        if !all_characters.is_empty() {
            let truncated: Vec<&str> = all_characters.iter().take(10).map(|s| s.as_str()).collect();
            parts.push(format!("Key files: {}.", truncated.join(", ")));
        }

        // Per-chapter plot summaries (abbreviated to fit token budget).
        let budget_per_chapter = if chapters.is_empty() {
            self.max_summary_tokens as usize
        } else {
            (self.max_summary_tokens as usize).saturating_sub(parts.iter().map(|p| p.len()).sum::<usize>())
                / chapters.len().max(1)
        };

        for chapter in chapters {
            let plot_summary = if chapter.plot.outcome.len() > budget_per_chapter {
                format!(
                    "Ch.{}: {}...",
                    chapter.chapter_number,
                    &chapter.plot.outcome[..budget_per_chapter.min(chapter.plot.outcome.len())]
                )
            } else {
                format!("Ch.{}: {}", chapter.chapter_number, chapter.plot.outcome)
            };
            parts.push(plot_summary);
        }

        let full_text = parts.join(" ");

        // Truncate to approximate token limit (rough: 1 token ~ 4 chars).
        let char_limit = (self.max_summary_tokens as usize) * 4;
        if full_text.len() > char_limit {
            format!("{}...", &full_text[..char_limit])
        } else {
            full_text
        }
    }
}

/// Manager for arc lifecycle: creation, dormancy detection, and archival.
pub struct ArcManager {
    arcs: Vec<Arc>,
    summaries: Vec<ArcSummary>,
    dormancy_threshold_seconds: u64,
}

impl ArcManager {
    /// Create a new arc manager.
    pub fn new(dormancy_threshold_seconds: u64) -> Self {
        Self {
            arcs: Vec::new(),
            summaries: Vec::new(),
            dormancy_threshold_seconds,
        }
    }

    /// Get or create an arc with the given ID and title.
    pub fn get_or_create_arc(&mut self, arc_id: ArcId, title: &str, timestamp: &str) -> &mut Arc {
        if !self.arcs.iter().any(|a| a.arc_id == arc_id) {
            self.arcs.push(Arc {
                arc_id: arc_id.clone(),
                title: title.to_string(),
                state: ArcState::Active,
                chapters: Vec::new(),
                motifs: Vec::new(),
                active_tensions: Vec::new(),
                last_chapter_at: timestamp.to_string(),
                created_at: timestamp.to_string(),
            });
        }
        self.arcs.iter_mut().find(|a| a.arc_id == arc_id).unwrap()
    }

    /// Record that a chapter was added to an arc.
    pub fn record_chapter(
        &mut self,
        arc_id: &ArcId,
        chapter_number: u64,
        motifs: &[MotifId],
        tension_ids: &[TensionId],
        timestamp: &str,
    ) {
        if let Some(arc) = self.arcs.iter_mut().find(|a| a.arc_id == *arc_id) {
            if !arc.chapters.contains(&chapter_number) {
                arc.chapters.push(chapter_number);
            }
            for motif in motifs {
                if !arc.motifs.contains(motif) {
                    arc.motifs.push(motif.clone());
                }
            }
            for tid in tension_ids {
                if !arc.active_tensions.contains(tid) {
                    arc.active_tensions.push(tid.clone());
                }
            }
            arc.last_chapter_at = timestamp.to_string();
            arc.state = ArcState::Active;
        }
    }

    /// Detect arcs that should go dormant based on inactivity.
    ///
    /// Returns the IDs of newly dormant arcs.
    pub fn detect_dormancy(&mut self, current_timestamp_seconds: u64) -> Vec<ArcId> {
        let threshold = self.dormancy_threshold_seconds;
        let mut newly_dormant = Vec::new();

        for arc in &mut self.arcs {
            if arc.state != ArcState::Active {
                continue;
            }
            if let Ok(last) = arc.last_chapter_at.parse::<u64>() {
                if current_timestamp_seconds.saturating_sub(last) >= threshold {
                    arc.state = ArcState::Dormant;
                    newly_dormant.push(arc.arc_id.clone());
                }
            }
        }

        newly_dormant
    }

    /// Archive an arc, replacing its chapter list with a summary.
    pub fn archive_arc(&mut self, arc_id: &ArcId, summary: ArcSummary) {
        if let Some(arc) = self.arcs.iter_mut().find(|a| a.arc_id == *arc_id) {
            arc.state = ArcState::Archived;
            arc.chapters.clear();
        }
        self.summaries.push(summary);
    }

    /// Get an arc by ID.
    pub fn get_arc(&self, arc_id: &ArcId) -> Option<&Arc> {
        self.arcs.iter().find(|a| a.arc_id == *arc_id)
    }

    /// Get all active arcs.
    pub fn active_arcs(&self) -> Vec<&Arc> {
        self.arcs
            .iter()
            .filter(|a| a.state == ArcState::Active)
            .collect()
    }

    /// Get all arc summaries.
    pub fn summaries(&self) -> &[ArcSummary] {
        &self.summaries
    }

    /// Return the dormancy threshold in seconds.
    pub fn dormancy_threshold(&self) -> u64 {
        self.dormancy_threshold_seconds
    }

    /// Get a summary for a specific arc, if one has been produced.
    pub fn get_summary(&self, arc_id: &ArcId) -> Option<&ArcSummary> {
        self.summaries.iter().find(|s| s.arc_id == *arc_id)
    }
}
