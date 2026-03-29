//! Memory curation -- Brenner's role.
//!
//! The editor maintains narrative coherence. Brenner manages memory like a poet
//! manages a long work: by attending to motifs, tracking recurring images, and
//! noticing when a theme introduced in an early chapter reappears in a later
//! one with altered meaning.

use crate::narrative::BudgetMode;
use crate::types::{AgentId, ArcId, Chapter, MotifId, TensionEntry, TensionSeverity};

/// The editor agent. Responsible for narrative annotation, motif identification,
/// and tension detection.
pub struct Editor {
    agent_id: AgentId,
    /// The current budget mode (affects annotation depth).
    budget_mode: BudgetMode,
}

impl Editor {
    /// Create a new editor.
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            budget_mode: BudgetMode::Normal,
        }
    }

    /// Update the budget mode (called when token pressure changes).
    pub fn set_budget_mode(&mut self, mode: BudgetMode) {
        self.budget_mode = mode;
    }

    /// Identify thematic connections between a new task and existing chapters.
    ///
    /// Returns a premise analysis: which arcs are relevant, which motifs
    /// resonate, and which tensions might be affected.
    pub fn analyze_premise(
        &self,
        task_description: &str,
        existing_chapters: &[&Chapter],
    ) -> PremiseAnalysis {
        // Identify potentially relevant arcs.
        let relevant_arcs: Vec<ArcId> = existing_chapters
            .iter()
            .filter(|ch| {
                let task_lower = task_description.to_lowercase();
                let title_lower = ch.title.to_lowercase();
                let plot_lower = ch.plot.task.to_lowercase();
                // Simple heuristic: any word overlap suggests arc relevance.
                task_lower
                    .split_whitespace()
                    .any(|w| title_lower.contains(w) || plot_lower.contains(w))
            })
            .map(|ch| ch.arc.clone())
            .collect::<Vec<_>>();

        // Collect motifs from relevant chapters.
        let resonant_motifs: Vec<MotifId> = existing_chapters
            .iter()
            .filter(|ch| relevant_arcs.contains(&ch.arc))
            .flat_map(|ch| ch.motifs.iter().cloned())
            .collect::<Vec<_>>();

        // Identify unresolved tensions from relevant chapters.
        let active_tensions: Vec<TensionEntry> = existing_chapters
            .iter()
            .filter(|ch| relevant_arcs.contains(&ch.arc))
            .flat_map(|ch| ch.tensions_introduced.iter().cloned())
            .collect();

        PremiseAnalysis {
            relevant_arcs,
            resonant_motifs,
            active_tensions,
        }
    }

    /// Annotate a chapter with narrative metadata.
    ///
    /// In normal mode, this produces full narrative annotations with motif tags
    /// and tension descriptions. In flash fiction mode, it produces minimal
    /// annotations (motif tags only).
    pub fn annotate_chapter(&self, chapter: &Chapter) -> ChapterAnnotation {
        match self.budget_mode {
            BudgetMode::Normal => self.full_annotation(chapter),
            BudgetMode::FlashFiction => self.minimal_annotation(chapter),
            BudgetMode::SingleDraft | BudgetMode::Cliffhanger => self.minimal_annotation(chapter),
        }
    }

    /// Identify new themes in a chapter that might become motifs.
    ///
    /// Returns a list of (theme_id, description, form_in_this_chapter).
    pub fn identify_themes(&self, chapter: &Chapter) -> Vec<(String, String, String)> {
        let mut themes = Vec::new();

        // Extract themes from the chapter's characters (files).
        for character in &chapter.characters {
            // Derive a theme from file paths (e.g., "auth.rs" -> "authentication").
            if let Some(stem) = character.split('/').last().and_then(|f| f.strip_suffix(".rs")) {
                themes.push((
                    stem.to_string(),
                    format!("Work involving the {} module", stem),
                    format!("{} in chapter {}", chapter.plot.task, chapter.chapter_number),
                ));
            }
        }

        // Extract themes from the plot description.
        let key_words = extract_key_themes(&chapter.plot.task);
        for word in key_words {
            themes.push((
                word.clone(),
                format!("Theme: {}", word),
                format!("{} (ch.{})", chapter.plot.task, chapter.chapter_number),
            ));
        }

        themes
    }

    /// Detect potential tensions (contradictions) between a new chapter and
    /// existing chapters.
    pub fn detect_tensions(
        &self,
        new_chapter: &Chapter,
        existing_chapters: &[&Chapter],
    ) -> Vec<TensionEntry> {
        let mut tensions = Vec::new();

        for existing in existing_chapters {
            // Check for overlapping characters (files) with different approaches.
            let overlapping_files: Vec<&String> = new_chapter
                .characters
                .iter()
                .filter(|f| existing.characters.contains(f))
                .collect();

            if !overlapping_files.is_empty() && existing.arc == new_chapter.arc {
                // Same arc, same files, different chapters -- potential tension.
                let file_list: Vec<&str> = overlapping_files.iter().map(|f| f.as_str()).collect();
                tensions.push(TensionEntry {
                    id: crate::types::TensionId(format!(
                        "overlap-{}-{}",
                        existing.chapter_number, new_chapter.chapter_number
                    )),
                    description: format!(
                        "Chapters {} and {} both modify: {}",
                        existing.chapter_number,
                        new_chapter.chapter_number,
                        file_list.join(", ")
                    ),
                    severity: TensionSeverity::Moderate,
                    suggested_resolution: Some(
                        "Review for consistency between the two chapters' changes".to_string(),
                    ),
                });
            }
        }

        tensions
    }

    /// Get the editor's agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    fn full_annotation(&self, chapter: &Chapter) -> ChapterAnnotation {
        let narrative = format!(
            "Chapter {} ('{}') in the {} arc. {}: {}. Outcome: {}. \
             Motifs active: {}. Tensions introduced: {}.",
            chapter.chapter_number,
            chapter.title,
            chapter.arc.0,
            chapter.plot.task,
            chapter.plot.approach,
            chapter.plot.outcome,
            chapter
                .motifs
                .iter()
                .map(|m| m.0.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            chapter.tensions_introduced.len(),
        );

        ChapterAnnotation {
            narrative_summary: narrative,
            motif_tags: chapter.motifs.iter().map(|m| m.0.clone()).collect(),
            tension_count: chapter.tensions_introduced.len() as u32,
            annotation_mode: AnnotationMode::Full,
        }
    }

    fn minimal_annotation(&self, chapter: &Chapter) -> ChapterAnnotation {
        ChapterAnnotation {
            narrative_summary: format!("Ch.{}: {}", chapter.chapter_number, chapter.title),
            motif_tags: chapter.motifs.iter().map(|m| m.0.clone()).collect(),
            tension_count: chapter.tensions_introduced.len() as u32,
            annotation_mode: AnnotationMode::FlashFiction,
        }
    }
}

/// The result of analyzing a task's premise (thematic connections).
#[derive(Debug, Clone)]
pub struct PremiseAnalysis {
    pub relevant_arcs: Vec<ArcId>,
    pub resonant_motifs: Vec<MotifId>,
    pub active_tensions: Vec<TensionEntry>,
}

/// Narrative annotation for a chapter.
#[derive(Debug, Clone)]
pub struct ChapterAnnotation {
    pub narrative_summary: String,
    pub motif_tags: Vec<String>,
    pub tension_count: u32,
    pub annotation_mode: AnnotationMode,
}

/// The depth of narrative annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationMode {
    /// Full narrative with motifs, tensions, and prose summary.
    Full,
    /// Minimal: motif tags and chapter reference only.
    FlashFiction,
}

/// Extract key thematic words from a task description.
fn extract_key_themes(text: &str) -> Vec<String> {
    let stop_words = [
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "can", "shall", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "and", "but", "or", "nor", "not",
        "so", "yet", "both", "either", "neither", "each", "every", "all",
        "any", "few", "more", "most", "some", "such", "no", "only", "own",
        "same", "than", "too", "very", "just", "because", "if", "when",
        "where", "how", "what", "which", "who", "whom", "this", "that",
        "these", "those", "it", "its", "add", "update", "fix", "change",
        "modify", "create", "delete", "remove",
    ];

    text.to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3 && !stop_words.contains(w))
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect()
}
