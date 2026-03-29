//! Literary workshop.
//!
//! The workshop operates with four members: Orozco writes, Brenner edits,
//! Sato checks continuity, and Hartmann publishes. Every task is a chapter
//! in the ongoing story, processed through the workshop's collaborative
//! lifecycle.

mod author;
mod continuity;
mod editor;
mod publisher;

pub use author::{Author, AuthorConfig, ChapterPlan, Draft, DraftResult};
pub use continuity::{
    Convention, ContinuityChecker, ContinuityFinding, ContinuityReport, FindingSeverity,
};
pub use editor::{AnnotationMode, ChapterAnnotation, Editor, PremiseAnalysis};
pub use publisher::{
    CorrespondenceDirection, CorrespondenceEntry, PublicationStatus, Publisher, PublisherConfig,
    ScheduleEntry,
};

use crate::narrative::{BudgetMode, NarrativeEngine};
use crate::types::{
    AgentId, ArcId, ChapterColophon, ChapterId, ChapterPhase, ChapterPlot,
    ChapterSetting, ChapterProgress, MotifId, NarrativeConfig, TensionEntry, TensionId, WorkshopRole,
};

/// The literary workshop. Coordinates the four agents through the chapter lifecycle.
pub struct Workshop {
    author: Author,
    editor: Editor,
    continuity_checker: ContinuityChecker,
    publisher: Publisher,
    narrative: NarrativeEngine,
    current_phase: ChapterPhase,
    tokens_used: u64,
}

impl Workshop {
    /// Create a new workshop with default agent identities.
    pub fn new(config: NarrativeConfig) -> Self {
        Self {
            author: Author::new(AuthorConfig::default()),
            editor: Editor::new(AgentId("brenner".to_string())),
            continuity_checker: ContinuityChecker::new(AgentId("sato".to_string())),
            publisher: Publisher::new(PublisherConfig::default()),
            narrative: NarrativeEngine::new(config),
            current_phase: ChapterPhase::Premise,
            tokens_used: 0,
        }
    }

    /// Create a workshop with custom agent configurations.
    pub fn with_agents(
        config: NarrativeConfig,
        author_config: AuthorConfig,
        publisher_config: PublisherConfig,
    ) -> Self {
        Self {
            author: Author::new(author_config),
            editor: Editor::new(AgentId("brenner".to_string())),
            continuity_checker: ContinuityChecker::new(AgentId("sato".to_string())),
            publisher: Publisher::new(publisher_config),
            narrative: NarrativeEngine::new(config),
            current_phase: ChapterPhase::Premise,
            tokens_used: 0,
        }
    }

    /// Begin a new chapter: analyze the premise and identify thematic connections.
    pub fn begin_chapter(
        &mut self,
        task_description: &str,
        arc_id: ArcId,
        _arc_title: &str,
    ) -> PremiseAnalysis {
        self.current_phase = ChapterPhase::Premise;

        // Retrieve relevant chapters from narrative memory.
        let relevant = self.narrative.retrieve(task_description, Some(arc_id.clone()), 10);
        let chapter_refs: Vec<&crate::types::Chapter> = relevant.iter().map(|(ch, _)| *ch).collect();

        // Editor analyzes the premise.
        let analysis = self.editor.analyze_premise(task_description, &chapter_refs);

        // Update budget mode.
        let mode = self.narrative.budget_mode(self.tokens_used);
        self.editor.set_budget_mode(mode);

        if mode == BudgetMode::SingleDraft || mode == BudgetMode::Cliffhanger {
            self.author.enable_single_draft_mode();
        }

        analysis
    }

    /// Submit the author's outline for a chapter.
    pub fn submit_outline(&mut self, plan: ChapterPlan) {
        self.current_phase = ChapterPhase::Outline;
        self.author.outline(plan);
    }

    /// Submit a draft from the author.
    pub fn submit_draft(&mut self, draft: Draft) -> anyhow::Result<DraftResult> {
        self.current_phase = ChapterPhase::Draft;
        self.author.submit_draft(draft)
    }

    /// Run continuity check on a chapter.
    pub fn check_continuity(&mut self, chapter: &crate::types::Chapter) -> ContinuityReport {
        self.current_phase = ChapterPhase::Continuity;

        let existing = self.narrative.retrieve(&chapter.plot.task, Some(chapter.arc.clone()), 20);
        let chapter_refs: Vec<&crate::types::Chapter> = existing.iter().map(|(ch, _)| *ch).collect();

        self.continuity_checker
            .check_continuity(chapter, &chapter_refs)
    }

    /// Complete a chapter: write it to narrative memory and produce the manuscript.
    pub fn complete_chapter(
        &mut self,
        title: String,
        arc_id: ArcId,
        arc_title: &str,
        setting: ChapterSetting,
        characters: Vec<String>,
        plot: ChapterPlot,
        theme_appearances: Vec<(String, String, String)>,
        tensions_introduced: Vec<TensionEntry>,
        tensions_resolved: Vec<TensionId>,
        timestamp: &str,
    ) -> anyhow::Result<ChapterId> {
        self.current_phase = ChapterPhase::Publication;

        let colophon = ChapterColophon {
            author: self.author.agent_id().clone(),
            editor: self.editor.agent_id().clone(),
            continuity_checker: self.continuity_checker.agent_id().clone(),
            patch_ref: format!("chapter/{}/{}", arc_id.0, "latest"),
            commit_sha: None,
        };

        self.narrative.write_chapter(
            title,
            arc_id,
            arc_title,
            setting,
            characters,
            plot,
            theme_appearances,
            tensions_introduced,
            tensions_resolved,
            colophon,
            timestamp,
        )
    }

    /// Record tokens consumed.
    pub fn record_tokens(&mut self, tokens: u64) {
        self.tokens_used += tokens;
    }

    /// Get a progress report.
    pub fn progress(&self, chapter_number: u64, arc: ArcId) -> ChapterProgress {
        let active_motifs: Vec<MotifId> = self
            .narrative
            .motifs()
            .all_motifs()
            .iter()
            .take(10)
            .map(|m| m.motif_id.clone())
            .collect();

        let budget = self.narrative.budget_mode(self.tokens_used);
        let coherence = match budget {
            BudgetMode::Normal => 0.95,
            BudgetMode::FlashFiction => 0.80,
            BudgetMode::SingleDraft => 0.65,
            BudgetMode::Cliffhanger => 0.40,
        };

        ChapterProgress {
            phase: self.current_phase,
            agent: self.current_agent_id(),
            role: self.current_role(),
            chapter: chapter_number,
            arc,
            draft_number: self.author.draft_count(),
            tokens_used: self.tokens_used,
            tokens_budget: 45_000, // From config default.
            motifs_active: active_motifs,
            tensions_unresolved: self.narrative.tensions().active_count(),
            narrative_coherence: coherence,
        }
    }

    /// Get the current phase.
    pub fn current_phase(&self) -> ChapterPhase {
        self.current_phase
    }

    /// Access the narrative engine.
    pub fn narrative(&self) -> &NarrativeEngine {
        &self.narrative
    }

    /// Access the publisher.
    pub fn publisher(&self) -> &Publisher {
        &self.publisher
    }

    /// Access the publisher mutably.
    pub fn publisher_mut(&mut self) -> &mut Publisher {
        &mut self.publisher
    }

    fn current_agent_id(&self) -> AgentId {
        match self.current_phase {
            ChapterPhase::Premise | ChapterPhase::Editing => self.editor.agent_id().clone(),
            ChapterPhase::Outline | ChapterPhase::Draft | ChapterPhase::Revision => {
                self.author.agent_id().clone()
            }
            ChapterPhase::Continuity => self.continuity_checker.agent_id().clone(),
            ChapterPhase::Publication => AgentId("hartmann".to_string()),
        }
    }

    fn current_role(&self) -> WorkshopRole {
        match self.current_phase {
            ChapterPhase::Premise | ChapterPhase::Editing => WorkshopRole::Editor,
            ChapterPhase::Outline | ChapterPhase::Draft | ChapterPhase::Revision => {
                WorkshopRole::Author
            }
            ChapterPhase::Continuity => WorkshopRole::ContinuityChecker,
            ChapterPhase::Publication => WorkshopRole::Publisher,
        }
    }
}
