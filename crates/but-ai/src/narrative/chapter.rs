//! Chapter-based memory storage.
//!
//! Every task writes a new chapter in the codebase's story. Chapters are the
//! atomic unit of narrative memory -- self-contained narrative units with a
//! setting, characters, plot, motifs, and tensions.

use crate::types::{ArcId, Chapter, ChapterId, ChapterSetting, ChapterPlot, ChapterColophon, MotifId, TensionEntry};

/// Query parameters for chapter retrieval.
#[derive(Debug, Clone)]
pub struct ChapterQuery {
    /// Text to match against chapter content.
    pub text: String,
    /// Optionally restrict to a specific arc.
    pub arc: Option<ArcId>,
    /// Optionally restrict to chapters containing these motifs.
    pub motifs: Vec<MotifId>,
    /// Maximum number of results to return.
    pub max_results: usize,
}

/// Storage and retrieval for chapters in the ongoing story.
pub struct ChapterStore {
    chapters: Vec<Chapter>,
    next_chapter_number: u64,
    max_chapters: u64,
}

impl ChapterStore {
    /// Create a new, empty chapter store.
    pub fn new(max_chapters: u64) -> Self {
        Self {
            chapters: Vec::new(),
            next_chapter_number: 1,
            max_chapters,
        }
    }

    /// Write a new chapter to the store, returning its assigned number.
    ///
    /// If the store has reached `max_chapters`, the oldest chapter without
    /// active tensions is archived before the new chapter is written.
    pub fn write_chapter(
        &mut self,
        title: String,
        arc: ArcId,
        setting: ChapterSetting,
        characters: Vec<String>,
        plot: ChapterPlot,
        motifs: Vec<MotifId>,
        tensions_introduced: Vec<TensionEntry>,
        tensions_resolved: Vec<crate::types::TensionId>,
        colophon: ChapterColophon,
    ) -> anyhow::Result<ChapterId> {
        if self.chapters.len() as u64 >= self.max_chapters {
            self.archive_oldest()?;
        }

        let chapter_number = self.next_chapter_number;
        self.next_chapter_number += 1;

        let chapter = Chapter {
            chapter_number,
            title,
            arc,
            setting,
            characters,
            plot,
            motifs,
            tensions_introduced,
            tensions_resolved,
            colophon,
        };

        self.chapters.push(chapter);
        Ok(ChapterId(chapter_number))
    }

    /// Retrieve a chapter by its number.
    pub fn get_chapter(&self, id: &ChapterId) -> Option<&Chapter> {
        self.chapters.iter().find(|c| c.chapter_number == id.0)
    }

    /// Query chapters by thematic resonance.
    ///
    /// Returns chapters sorted by relevance: motif overlap is weighted highest,
    /// followed by arc membership, then textual similarity, then recency.
    pub fn query(&self, query: &ChapterQuery) -> Vec<&Chapter> {
        let mut scored: Vec<(&Chapter, f64)> = self
            .chapters
            .iter()
            .map(|chapter| {
                let mut score = 0.0_f64;

                // Motif resonance (highest weight)
                if !query.motifs.is_empty() {
                    let overlap = chapter
                        .motifs
                        .iter()
                        .filter(|m| query.motifs.contains(m))
                        .count();
                    let max_possible = query.motifs.len().max(1);
                    score += 0.40 * (overlap as f64 / max_possible as f64);
                }

                // Arc relevance
                if let Some(ref query_arc) = query.arc {
                    if chapter.arc == *query_arc {
                        score += 0.25;
                    }
                }

                // Textual similarity (simple word overlap)
                let query_words: Vec<&str> = query.text.split_whitespace().collect();
                if !query_words.is_empty() {
                    let title_lower = chapter.title.to_lowercase();
                    let task_lower = chapter.plot.task.to_lowercase();
                    let matching = query_words
                        .iter()
                        .filter(|w| {
                            let w_lower = w.to_lowercase();
                            title_lower.contains(&w_lower) || task_lower.contains(&w_lower)
                        })
                        .count();
                    score += 0.20 * (matching as f64 / query_words.len() as f64);
                }

                // Recency: higher chapter numbers are more recent.
                let max_num = self.next_chapter_number.saturating_sub(1).max(1) as f64;
                score += 0.15 * (chapter.chapter_number as f64 / max_num);

                (chapter, score)
            })
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored
            .into_iter()
            .take(query.max_results)
            .map(|(ch, _)| ch)
            .collect()
    }

    /// Return all chapters in a given arc.
    pub fn chapters_in_arc(&self, arc: &ArcId) -> Vec<&Chapter> {
        self.chapters.iter().filter(|c| c.arc == *arc).collect()
    }

    /// Return the total number of stored chapters.
    pub fn chapter_count(&self) -> u64 {
        self.chapters.len() as u64
    }

    /// Return the most recently written chapter.
    pub fn latest_chapter(&self) -> Option<&Chapter> {
        self.chapters.last()
    }

    /// Archive the oldest chapter that has no active tensions.
    fn archive_oldest(&mut self) -> anyhow::Result<()> {
        if self.chapters.is_empty() {
            anyhow::bail!("Chapter store is empty, cannot archive");
        }

        // Prefer to archive chapters with no introduced tensions first.
        let idx = self
            .chapters
            .iter()
            .position(|c| c.tensions_introduced.is_empty())
            .unwrap_or(0);

        self.chapters.remove(idx);
        Ok(())
    }
}
