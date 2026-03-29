//! Narrative memory engine.
//!
//! The narrative module stores agent memory as chapters in an ongoing story.
//! Recurring themes emerge as motifs that serve as retrieval anchors.
//! Contradictions between memories create dramatic tensions that the system
//! flags for resolution rather than silently overwriting.
//! Compaction produces arc summaries that preserve thematic content.

mod chapter;
mod motif;
mod summary;
mod tension;

pub use chapter::{ChapterQuery, ChapterStore};
pub use motif::{MotifConfig, MotifTracker};
pub use summary::{ArcManager, Summarizer};
pub use tension::{TensionConfig, TensionRegistry};

use crate::types::{
    ArcId, Chapter, ChapterId, ChapterColophon, ChapterPlot, ChapterSetting,
    MotifId, NarrativeConfig, RelevanceScore, TensionEntry, TensionId,
};

/// The core narrative memory engine. Manages the interplay between chapters,
/// motifs, tensions, and arcs to provide thematically rich memory retrieval.
pub struct NarrativeEngine {
    config: NarrativeConfig,
    chapters: ChapterStore,
    motifs: MotifTracker,
    tensions: TensionRegistry,
    arcs: ArcManager,
    summarizer: Summarizer,
}

impl NarrativeEngine {
    /// Create a new narrative engine with the given configuration.
    pub fn new(config: NarrativeConfig) -> Self {
        let chapters = ChapterStore::new(config.max_chapters);
        let motifs = MotifTracker::new(MotifConfig {
            emergence_threshold: config.motif_threshold,
        });
        let tensions = TensionRegistry::new(TensionConfig {
            escalation_threshold_seconds: config.tension_escalation_seconds,
        });
        let arcs = ArcManager::new(config.arc_dormancy_seconds);
        let summarizer = Summarizer::new(config.summary_max_tokens);

        Self {
            config,
            chapters,
            motifs,
            tensions,
            arcs,
            summarizer,
        }
    }

    /// Write a new chapter to the narrative, updating motifs, tensions, and arcs.
    pub fn write_chapter(
        &mut self,
        title: String,
        arc_id: ArcId,
        arc_title: &str,
        setting: ChapterSetting,
        characters: Vec<String>,
        plot: ChapterPlot,
        theme_appearances: Vec<(String, String, String)>, // (theme_id, description, form)
        tensions_introduced: Vec<TensionEntry>,
        tensions_resolved: Vec<TensionId>,
        colophon: ChapterColophon,
        timestamp: &str,
    ) -> anyhow::Result<ChapterId> {
        // Ensure the arc exists.
        self.arcs
            .get_or_create_arc(arc_id.clone(), arc_title, timestamp);

        // Collect motif IDs from theme appearances.
        let motif_ids: Vec<MotifId> = theme_appearances
            .iter()
            .map(|(id, _, _)| MotifId(id.clone()))
            .collect();

        // Write the chapter.
        let chapter_id = self.chapters.write_chapter(
            title,
            arc_id.clone(),
            setting,
            characters,
            plot,
            motif_ids.clone(),
            tensions_introduced.clone(),
            tensions_resolved.clone(),
            colophon,
        )?;

        // Record theme appearances, potentially promoting to motifs.
        for (theme_id, description, form) in &theme_appearances {
            self.motifs
                .record_appearance(theme_id, description, chapter_id.0, form);
        }

        // Register new tensions.
        for tension in &tensions_introduced {
            self.tensions.introduce(tension, chapter_id.0, timestamp);
        }

        // Resolve tensions.
        for tension_id in &tensions_resolved {
            // Ignore errors for tensions that might not exist.
            let _ = self.tensions.resolve(tension_id, chapter_id.0);
        }

        // Update the arc.
        let tension_ids: Vec<TensionId> = tensions_introduced.iter().map(|t| t.id.clone()).collect();
        self.arcs
            .record_chapter(&arc_id, chapter_id.0, &motif_ids, &tension_ids, timestamp);

        Ok(chapter_id)
    }

    /// Retrieve chapters relevant to a query, ranked by narrative relevance.
    ///
    /// The scoring function weights motif resonance highest, followed by arc
    /// relevance, textual similarity, recency, and tension urgency.
    pub fn retrieve(
        &self,
        query: &str,
        arc: Option<ArcId>,
        max_results: usize,
    ) -> Vec<(&Chapter, RelevanceScore)> {
        // Find resonant motifs.
        let resonant_motifs = self.motifs.find_resonant(query, 10);
        let resonant_ids: Vec<MotifId> = resonant_motifs
            .iter()
            .map(|m| m.motif_id.clone())
            .collect();

        // Get chapters connected to resonant motifs (including transitive).
        let motif_chapters = self.motifs.chapters_from_motifs(&resonant_ids);

        // Build chapter query.
        let chapter_query = ChapterQuery {
            text: query.to_string(),
            arc,
            motifs: resonant_ids,
            max_results: max_results * 2, // Over-fetch for re-scoring.
        };

        let candidates = self.chapters.query(&chapter_query);

        // Re-score with full relevance model.
        let mut scored: Vec<(&Chapter, RelevanceScore)> = candidates
            .into_iter()
            .map(|chapter| {
                let motif_resonance = if motif_chapters.is_empty() {
                    0.0
                } else {
                    let in_motif = motif_chapters.contains(&chapter.chapter_number);
                    if in_motif { 1.0 } else { 0.0 }
                };

                let arc_relevance = if let Some(ref qa) = chapter_query.arc {
                    if chapter.arc == *qa { 1.0 } else { 0.0 }
                } else {
                    0.5
                };

                let max_num = self.chapters.chapter_count().max(1) as f64;
                let recency = chapter.chapter_number as f64 / max_num;

                let chapter_tension_ids: Vec<TensionId> = chapter
                    .tensions_introduced
                    .iter()
                    .map(|t| t.id.clone())
                    .collect();
                let tension_urgency = self.tensions.urgency_score(&chapter_tension_ids);

                let score = RelevanceScore {
                    motif_resonance,
                    embedding_similarity: 0.0, // Would use real embeddings in production.
                    arc_relevance,
                    recency,
                    tension_urgency,
                };

                (chapter, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.total()
                .partial_cmp(&a.1.total())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored.truncate(max_results);
        scored
    }

    /// Determine the current budget mode based on tokens consumed.
    pub fn budget_mode(&self, tokens_used: u64) -> BudgetMode {
        let ratio = tokens_used as f64 / self.config.token_budget as f64;
        if ratio >= self.config.cliffhanger_threshold {
            BudgetMode::Cliffhanger
        } else if ratio >= self.config.single_draft_threshold {
            BudgetMode::SingleDraft
        } else if ratio >= self.config.flash_fiction_threshold {
            BudgetMode::FlashFiction
        } else {
            BudgetMode::Normal
        }
    }

    /// Run periodic maintenance: escalate overdue tensions, detect dormant arcs.
    pub fn maintenance(&mut self, current_timestamp_seconds: u64) -> MaintenanceReport {
        let escalated = self.tensions.escalate_overdue(current_timestamp_seconds);
        let dormant_arcs = self.arcs.detect_dormancy(current_timestamp_seconds);
        MaintenanceReport {
            tensions_escalated: escalated,
            arcs_dormant: dormant_arcs,
        }
    }

    /// Access the chapter store for direct queries.
    pub fn chapters(&self) -> &ChapterStore {
        &self.chapters
    }

    /// Access the motif tracker.
    pub fn motifs(&self) -> &MotifTracker {
        &self.motifs
    }

    /// Access the tension registry.
    pub fn tensions(&self) -> &TensionRegistry {
        &self.tensions
    }

    /// Access the arc manager.
    pub fn arcs(&self) -> &ArcManager {
        &self.arcs
    }

    /// Access the summarizer.
    pub fn summarizer(&self) -> &Summarizer {
        &self.summarizer
    }
}

/// Budget mode determines the annotation and drafting depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetMode {
    /// Full narrative annotation, multi-draft.
    Normal,
    /// Minimal annotations, motif tags only.
    FlashFiction,
    /// Single-draft mode, reduced generation.
    SingleDraft,
    /// Partial output: produce whatever is available and stop.
    Cliffhanger,
}

/// Report from periodic maintenance operations.
#[derive(Debug, Clone)]
pub struct MaintenanceReport {
    pub tensions_escalated: Vec<TensionId>,
    pub arcs_dormant: Vec<ArcId>,
}
