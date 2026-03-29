//! Commit and patch metadata.
//!
//! The colophon metadata module produces structured narrative metadata for
//! commits and patches. This metadata is not decorative -- it provides
//! structured information that Brenner uses for future memory retrieval.

use crate::types::{AgentId, ArcId, ManuscriptColophon, MotifId, TensionId};

/// Narrative trailer for a commit message.
///
/// Follows the format:
/// ```text
/// Chapter: 42 of the authentication arc
/// Motifs: security-boundary, trust-verification
/// Tension-introduced: timeout-vs-longrun
/// Continuity: verified by sato
/// ```
#[derive(Debug, Clone)]
pub struct NarrativeTrailer {
    pub chapter_number: u64,
    pub arc: ArcId,
    pub motifs: Vec<MotifId>,
    pub tensions_introduced: Vec<TensionId>,
    pub continuity_verified_by: Option<AgentId>,
}

impl NarrativeTrailer {
    /// Create a trailer from manuscript colophon data.
    pub fn from_colophon(colophon: &ManuscriptColophon) -> Self {
        Self {
            chapter_number: colophon.chapter_number,
            arc: colophon.arc.clone(),
            motifs: colophon.motifs.clone(),
            tensions_introduced: colophon.tensions_introduced.clone(),
            continuity_verified_by: colophon.continuity_verified_by.clone(),
        }
    }

    /// Format as git commit trailer lines.
    pub fn to_trailer_lines(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Chapter: {} of the {} arc",
            self.chapter_number, self.arc.0
        ));

        if !self.motifs.is_empty() {
            let motif_list: Vec<&str> = self.motifs.iter().map(|m| m.0.as_str()).collect();
            lines.push(format!("Motifs: {}", motif_list.join(", ")));
        }

        for tension in &self.tensions_introduced {
            lines.push(format!("Tension-introduced: {}", tension.0));
        }

        if let Some(ref checker) = self.continuity_verified_by {
            lines.push(format!("Continuity: verified by {}", checker.0));
        }

        lines.join("\n")
    }

    /// Parse trailer lines from a commit message body.
    ///
    /// Looks for lines starting with "Chapter:", "Motifs:", "Tension-introduced:",
    /// and "Continuity:" to reconstruct the trailer.
    pub fn parse_from_message(message: &str) -> Option<Self> {
        let mut chapter_number = None;
        let mut arc = None;
        let mut motifs = Vec::new();
        let mut tensions = Vec::new();
        let mut continuity_by = None;

        for line in message.lines() {
            let line = line.trim();

            if let Some(rest) = line.strip_prefix("Chapter: ") {
                // Parse "42 of the authentication arc"
                let parts: Vec<&str> = rest.splitn(2, " of the ").collect();
                if let Some(num_str) = parts.first() {
                    chapter_number = num_str.trim().parse().ok();
                }
                if let Some(arc_str) = parts.get(1) {
                    arc = Some(ArcId(arc_str.trim_end_matches(" arc").to_string()));
                }
            } else if let Some(rest) = line.strip_prefix("Motifs: ") {
                motifs = rest
                    .split(", ")
                    .map(|m| MotifId(m.trim().to_string()))
                    .collect();
            } else if let Some(rest) = line.strip_prefix("Tension-introduced: ") {
                tensions.push(TensionId(rest.trim().to_string()));
            } else if let Some(rest) = line.strip_prefix("Continuity: verified by ") {
                continuity_by = Some(AgentId(rest.trim().to_string()));
            }
        }

        Some(NarrativeTrailer {
            chapter_number: chapter_number?,
            arc: arc?,
            motifs,
            tensions_introduced: tensions,
            continuity_verified_by: continuity_by,
        })
    }
}

/// Builds a complete commit message with narrative metadata.
pub struct CommitMessageBuilder {
    title: Option<String>,
    body: Vec<String>,
    trailer: Option<NarrativeTrailer>,
    extra_trailers: Vec<(String, String)>,
}

impl CommitMessageBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            title: None,
            body: Vec::new(),
            trailer: None,
            extra_trailers: Vec::new(),
        }
    }

    /// Set the commit message title (first line).
    pub fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Add a body paragraph.
    pub fn with_body(mut self, paragraph: &str) -> Self {
        self.body.push(paragraph.to_string());
        self
    }

    /// Set the narrative trailer.
    pub fn with_trailer(mut self, trailer: NarrativeTrailer) -> Self {
        self.trailer = Some(trailer);
        self
    }

    /// Add an extra key-value trailer (e.g., "Co-authored-by").
    pub fn with_extra_trailer(mut self, key: &str, value: &str) -> Self {
        self.extra_trailers
            .push((key.to_string(), value.to_string()));
        self
    }

    /// Build the final commit message.
    pub fn build(self) -> anyhow::Result<String> {
        let title = self
            .title
            .ok_or_else(|| anyhow::anyhow!("Commit message requires a title"))?;

        let mut message = title;

        if !self.body.is_empty() {
            message.push_str("\n\n");
            message.push_str(&self.body.join("\n\n"));
        }

        let has_trailers = self.trailer.is_some() || !self.extra_trailers.is_empty();
        if has_trailers {
            message.push_str("\n\n");

            let mut trailer_lines = Vec::new();
            if let Some(trailer) = self.trailer {
                trailer_lines.push(trailer.to_trailer_lines());
            }
            for (key, value) in &self.extra_trailers {
                trailer_lines.push(format!("{}: {}", key, value));
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

/// Generate a branch name following the arc-based convention.
///
/// Format: `chapter/<arc>/<chapter-number>[.<dependency>]`
pub fn branch_name(arc: &ArcId, chapter_number: u64, depends_on: Option<u64>) -> String {
    let base = format!(
        "chapter/{}/{:03}",
        arc.0, chapter_number
    );
    match depends_on {
        Some(dep) => format!("{}.{:03}", base, dep),
        None => base,
    }
}

/// Extract narrative metadata from an existing commit message.
pub fn extract_metadata(commit_message: &str) -> Option<NarrativeTrailer> {
    NarrativeTrailer::parse_from_message(commit_message)
}
