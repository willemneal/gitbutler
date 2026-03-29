//! Patch generation -- Orozco's role.
//!
//! The author writes the text. In our workshop, this means generating code
//! patches as unified diffs. Orozco works in steady, rhythmic passes like a
//! shuttle crossing a loom -- reading the task, studying the relevant code,
//! and weaving changes through the existing structure.

use crate::types::{AgentId, ChapterPhase, Manuscript, ManuscriptColophon, ArcId, MotifId, TensionId};

/// Configuration for the author's drafting process.
#[derive(Debug, Clone)]
pub struct AuthorConfig {
    /// Maximum number of drafts before finalizing.
    pub max_drafts: u32,
    /// Maximum lines per patch (max_chapter_size).
    pub max_patch_lines: u32,
    /// The author's agent ID.
    pub agent_id: AgentId,
}

impl Default for AuthorConfig {
    fn default() -> Self {
        Self {
            max_drafts: 2,
            max_patch_lines: 500,
            agent_id: AgentId("orozco".to_string()),
        }
    }
}

/// A draft produced by the author -- an intermediate artifact before publication.
#[derive(Debug, Clone)]
pub struct Draft {
    /// Draft number (1-indexed).
    pub number: u32,
    /// The unified diff content.
    pub patch_content: String,
    /// Self-review notes from the author.
    pub review_notes: String,
    /// Files modified in this draft.
    pub files_modified: Vec<String>,
    /// Lines added.
    pub lines_added: u32,
    /// Lines removed.
    pub lines_removed: u32,
}

impl Draft {
    /// Check if this draft exceeds the line limit.
    pub fn exceeds_limit(&self, max_lines: u32) -> bool {
        self.lines_added + self.lines_removed > max_lines
    }

    /// Approximate token cost of this draft.
    pub fn estimated_tokens(&self) -> u64 {
        // Rough estimate: 1 line ~ 10 tokens for code.
        (self.lines_added as u64 + self.lines_removed as u64) * 10
    }
}

/// A chapter plan: the outline Orozco creates before drafting.
#[derive(Debug, Clone)]
pub struct ChapterPlan {
    pub files_to_modify: Vec<String>,
    pub approach: String,
    pub estimated_lines: u32,
    pub estimated_drafts: u32,
}

/// The author agent. Manages the drafting lifecycle for a single chapter.
pub struct Author {
    config: AuthorConfig,
    drafts: Vec<Draft>,
    plan: Option<ChapterPlan>,
    current_phase: ChapterPhase,
}

impl Author {
    /// Create a new author with the given configuration.
    pub fn new(config: AuthorConfig) -> Self {
        Self {
            config,
            drafts: Vec::new(),
            plan: None,
            current_phase: ChapterPhase::Premise,
        }
    }

    /// Set the chapter plan (OUTLINE phase).
    pub fn outline(&mut self, plan: ChapterPlan) {
        self.plan = Some(plan);
        self.current_phase = ChapterPhase::Outline;
    }

    /// Submit a draft (DRAFT or REVISION phase).
    pub fn submit_draft(&mut self, draft: Draft) -> anyhow::Result<DraftResult> {
        let draft_number = draft.number;

        // Validate line limit.
        if draft.exceeds_limit(self.config.max_patch_lines) {
            return Ok(DraftResult::ExceedsLimit {
                lines: draft.lines_added + draft.lines_removed,
                limit: self.config.max_patch_lines,
            });
        }

        self.drafts.push(draft);

        if draft_number >= self.config.max_drafts {
            self.current_phase = ChapterPhase::Continuity;
            Ok(DraftResult::Final)
        } else {
            self.current_phase = ChapterPhase::Revision;
            Ok(DraftResult::NeedsRevision {
                draft_number,
                max_drafts: self.config.max_drafts,
            })
        }
    }

    /// Finalize the author's work into a manuscript, ready for continuity check
    /// and editorial annotation.
    pub fn finalize(
        &self,
        chapter_number: u64,
        arc: ArcId,
        motifs: Vec<MotifId>,
        tensions: Vec<TensionId>,
        continuity_checker: Option<AgentId>,
    ) -> anyhow::Result<Manuscript> {
        let final_draft = self
            .drafts
            .last()
            .ok_or_else(|| anyhow::anyhow!("No drafts submitted, cannot finalize"))?;

        let commit_message = self.compose_commit_message(
            chapter_number,
            &arc,
            &motifs,
            &tensions,
            continuity_checker.as_ref(),
        );

        Ok(Manuscript {
            patch: final_draft.patch_content.clone(),
            commit_message,
            colophon: ManuscriptColophon {
                chapter_number,
                arc,
                motifs,
                tensions_introduced: tensions,
                continuity_verified_by: continuity_checker,
            },
        })
    }

    /// Compose a commit message with narrative colophon.
    fn compose_commit_message(
        &self,
        chapter_number: u64,
        arc: &ArcId,
        motifs: &[MotifId],
        tensions: &[TensionId],
        continuity_checker: Option<&AgentId>,
    ) -> String {
        let plan_desc = self
            .plan
            .as_ref()
            .map(|p| p.approach.as_str())
            .unwrap_or("No plan recorded");

        let motif_list: Vec<&str> = motifs.iter().map(|m| m.0.as_str()).collect();
        let tension_list: Vec<&str> = tensions.iter().map(|t| t.0.as_str()).collect();

        let mut msg = format!("{}\n", plan_desc);
        msg.push_str(&format!("\nChapter: {} of the {} arc", chapter_number, arc.0));
        msg.push_str(&format!("\nMotifs: {}", motif_list.join(", ")));

        if !tension_list.is_empty() {
            msg.push_str(&format!("\nTensions introduced: {}", tension_list.join(", ")));
        }

        if let Some(checker) = continuity_checker {
            msg.push_str(&format!("\nContinuity: verified by {}", checker.0));
        }

        msg
    }

    /// Get the current phase.
    pub fn current_phase(&self) -> ChapterPhase {
        self.current_phase
    }

    /// Get the number of drafts submitted so far.
    pub fn draft_count(&self) -> u32 {
        self.drafts.len() as u32
    }

    /// Get the author's agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.config.agent_id
    }

    /// Switch to single-draft mode (budget pressure).
    pub fn enable_single_draft_mode(&mut self) {
        self.config.max_drafts = 1;
    }
}

/// Result of submitting a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftResult {
    /// Draft accepted, but more revisions are expected.
    NeedsRevision { draft_number: u32, max_drafts: u32 },
    /// This was the final draft.
    Final,
    /// Draft exceeds the line limit.
    ExceedsLimit { lines: u32, limit: u32 },
}
