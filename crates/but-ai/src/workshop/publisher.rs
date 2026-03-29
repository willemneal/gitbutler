//! Cross-repo coordination -- Hartmann's role.
//!
//! The publisher manages cross-repo communication as correspondence between
//! authors. Hartmann translates the commune's internal narrative language
//! into structured correspondence for external consumption, and incoming
//! correspondence into narrative context.

use crate::types::{
    AgentId, ArcId, BudgetStatus, Correspondent, Letter, LetterContent, LetterId,
    LetterType, MotifId, PrId, RepoRef,
};

/// Configuration for the publisher.
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    pub agent_id: AgentId,
    pub commune: String,
    pub home_repo: RepoRef,
    pub correspondence_schema: String,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            agent_id: AgentId("hartmann".to_string()),
            commune: "084-loom-and-verse".to_string(),
            home_repo: RepoRef {
                owner: "gitbutler".to_string(),
                name: "gitbutler".to_string(),
            },
            correspondence_schema: "but-ai/correspondence/v1".to_string(),
        }
    }
}

/// A publication schedule entry -- a tracked coordination obligation.
#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    pub target: RepoRef,
    pub pr: Option<PrId>,
    pub arc: ArcId,
    pub obligation: String,
    pub priority: u32,
    pub status: PublicationStatus,
}

/// Status of a publication (PR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStatus {
    /// Not yet submitted.
    Pending,
    /// PR opened, awaiting review.
    Submitted,
    /// Under review.
    InReview,
    /// Approved.
    Approved,
    /// Merged.
    Published,
    /// Rejected or closed.
    Rejected,
}

/// The correspondence log: tracks narrative threads across repositories.
#[derive(Debug, Clone)]
pub struct CorrespondenceEntry {
    pub letter_id: LetterId,
    pub letter: Letter,
    pub direction: CorrespondenceDirection,
}

/// Direction of correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrespondenceDirection {
    Outgoing,
    Incoming,
}

/// The publisher agent. Manages cross-repo coordination and external communication.
pub struct Publisher {
    config: PublisherConfig,
    schedule: Vec<ScheduleEntry>,
    correspondence_log: Vec<CorrespondenceEntry>,
    next_letter_id: u64,
}

impl Publisher {
    /// Create a new publisher with the given configuration.
    pub fn new(config: PublisherConfig) -> Self {
        Self {
            config,
            schedule: Vec::new(),
            correspondence_log: Vec::new(),
            next_letter_id: 1,
        }
    }

    /// Compose a letter for external delivery.
    pub fn compose_letter(
        &mut self,
        letter_type: LetterType,
        to_author: AgentId,
        to_commune: &str,
        to_repo: RepoRef,
        subject: &str,
        arc: Option<ArcId>,
        chapter_ref: Option<String>,
        status: Option<String>,
        dependencies: Vec<String>,
        budget: Option<BudgetStatus>,
        motifs: Vec<MotifId>,
        timestamp: &str,
    ) -> Letter {
        let letter = Letter {
            schema: self.config.correspondence_schema.clone(),
            letter_type,
            from: Correspondent {
                author: self.config.agent_id.clone(),
                commune: self.config.commune.clone(),
                repo: self.config.home_repo.clone(),
            },
            to: Correspondent {
                author: to_author,
                commune: to_commune.to_string(),
                repo: to_repo,
            },
            content: LetterContent {
                subject: subject.to_string(),
                arc,
                chapter_ref,
                status,
                dependencies,
                budget,
                motifs,
            },
            timestamp: timestamp.to_string(),
        };

        // Record in correspondence log.
        let letter_id = LetterId(format!("letter-{}", self.next_letter_id));
        self.next_letter_id += 1;
        self.correspondence_log.push(CorrespondenceEntry {
            letter_id,
            letter: letter.clone(),
            direction: CorrespondenceDirection::Outgoing,
        });

        letter
    }

    /// Format a letter as a PR comment (markdown with JSON code fence).
    pub fn format_as_comment(letter: &Letter) -> anyhow::Result<String> {
        let json = serde_json::to_string_pretty(letter)?;
        Ok(format!("```but-ai-letter\n{}\n```", json))
    }

    /// Parse a letter from a PR comment body.
    pub fn parse_from_comment(comment_body: &str) -> anyhow::Result<Letter> {
        let start = comment_body
            .find("```but-ai-letter")
            .ok_or_else(|| anyhow::anyhow!("No but-ai-letter code fence found"))?;
        let json_start = comment_body[start..]
            .find('\n')
            .map(|n| start + n + 1)
            .ok_or_else(|| anyhow::anyhow!("Malformed code fence"))?;
        let json_end = comment_body[json_start..]
            .find("```")
            .map(|n| json_start + n)
            .ok_or_else(|| anyhow::anyhow!("Unclosed code fence"))?;
        let json_str = &comment_body[json_start..json_end];
        let letter: Letter = serde_json::from_str(json_str)?;
        Ok(letter)
    }

    /// Record an incoming letter in the correspondence log.
    pub fn receive_letter(&mut self, letter: Letter) -> LetterId {
        let letter_id = LetterId(format!("letter-{}", self.next_letter_id));
        self.next_letter_id += 1;
        self.correspondence_log.push(CorrespondenceEntry {
            letter_id: letter_id.clone(),
            letter,
            direction: CorrespondenceDirection::Incoming,
        });
        letter_id
    }

    /// Add a publication schedule entry.
    pub fn schedule_publication(&mut self, entry: ScheduleEntry) {
        self.schedule.push(entry);
    }

    /// Update the status of a scheduled publication.
    pub fn update_publication_status(
        &mut self,
        target: &RepoRef,
        arc: &ArcId,
        status: PublicationStatus,
    ) {
        for entry in &mut self.schedule {
            if entry.target == *target && entry.arc == *arc {
                entry.status = status;
            }
        }
    }

    /// Get all pending schedule entries, sorted by priority.
    pub fn pending_publications(&self) -> Vec<&ScheduleEntry> {
        let mut pending: Vec<&ScheduleEntry> = self
            .schedule
            .iter()
            .filter(|e| e.status == PublicationStatus::Pending)
            .collect();
        pending.sort_by(|a, b| a.priority.cmp(&b.priority));
        pending
    }

    /// Get the correspondence log.
    pub fn correspondence_log(&self) -> &[CorrespondenceEntry] {
        &self.correspondence_log
    }

    /// Get the publisher's agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.config.agent_id
    }

    /// Get incoming letters for a specific arc.
    pub fn incoming_for_arc(&self, arc: &ArcId) -> Vec<&Letter> {
        self.correspondence_log
            .iter()
            .filter(|e| e.direction == CorrespondenceDirection::Incoming)
            .filter(|e| e.letter.content.arc.as_ref() == Some(arc))
            .map(|e| &e.letter)
            .collect()
    }

    /// Translate an internal narrative description to external-facing language.
    ///
    /// Hartmann's key skill: converting the commune's textile-inflected
    /// language to industry-standard terminology.
    pub fn translate_for_external(internal_description: &str) -> String {
        // A simplified translation table. In production, this would use
        // the LLM to perform nuanced translation.
        let translations = [
            ("wove", "integrated"),
            ("thread", "component"),
            ("warp", "persistent context"),
            ("weft", "task-specific context"),
            ("fabric", "output"),
            ("loom", "memory engine"),
            ("tension", "conflict"),
            ("motif", "recurring theme"),
            ("chapter", "task"),
            ("arc", "feature area"),
            ("colophon", "metadata"),
        ];

        let mut result = internal_description.to_string();
        for (from, to) in &translations {
            result = result.replace(from, to);
        }
        result
    }
}
