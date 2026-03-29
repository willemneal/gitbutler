//! Recurring theme/motif tracking.
//!
//! A motif is a recurring theme identified by the editor. When a theme appears
//! in three or more chapters, it becomes a motif -- a retrieval anchor that
//! captures thematic resonance beyond keyword matching.
//!
//! Motifs persist across arc dormancy. Even when the specific details of old
//! chapters are forgotten, the motifs they established continue to influence
//! retrieval.

use crate::types::{Motif, MotifId, MotifVariation};

/// Configuration for motif emergence and tracking.
#[derive(Debug, Clone)]
pub struct MotifConfig {
    /// Minimum number of appearances before a theme becomes a motif.
    pub emergence_threshold: u32,
}

impl Default for MotifConfig {
    fn default() -> Self {
        Self {
            emergence_threshold: 3,
        }
    }
}

/// Tracks recurring themes across chapters, managing their lifecycle
/// from first appearance through emergence to full motif status.
pub struct MotifTracker {
    /// Confirmed motifs (themes that have reached the emergence threshold).
    motifs: Vec<Motif>,
    /// Proto-motifs: themes that have appeared but not yet reached threshold.
    /// Maps theme description to (appearances, variations).
    proto_motifs: Vec<ProtoMotif>,
    config: MotifConfig,
}

/// A theme that has not yet reached motif status.
#[derive(Debug, Clone)]
struct ProtoMotif {
    id: MotifId,
    description: String,
    appearances: Vec<u64>,
    variations: Vec<MotifVariation>,
}

impl MotifTracker {
    /// Create a new tracker with the given configuration.
    pub fn new(config: MotifConfig) -> Self {
        Self {
            motifs: Vec::new(),
            proto_motifs: Vec::new(),
            config,
        }
    }

    /// Record an appearance of a theme in a chapter. If the theme has appeared
    /// enough times, it is promoted to a full motif.
    ///
    /// Returns `Some(MotifId)` if the theme was promoted to motif status.
    pub fn record_appearance(
        &mut self,
        theme_id: &str,
        description: &str,
        chapter_number: u64,
        form: &str,
    ) -> Option<MotifId> {
        let motif_id = MotifId(theme_id.to_string());
        let variation = MotifVariation {
            chapter: chapter_number,
            form: form.to_string(),
        };

        // Check if it is already a confirmed motif.
        if let Some(motif) = self.motifs.iter_mut().find(|m| m.motif_id == motif_id) {
            if !motif.appearances.contains(&chapter_number) {
                motif.appearances.push(chapter_number);
                motif.variations.push(variation);
            }
            return None; // Already a motif, no promotion event.
        }

        // Check if it is a proto-motif.
        if let Some(proto) = self.proto_motifs.iter_mut().find(|p| p.id == motif_id) {
            if !proto.appearances.contains(&chapter_number) {
                proto.appearances.push(chapter_number);
                proto.variations.push(variation);
            }

            // Check for promotion.
            if proto.appearances.len() as u32 >= self.config.emergence_threshold {
                let new_motif = Motif {
                    motif_id: proto.id.clone(),
                    description: proto.description.clone(),
                    first_appearance: proto.appearances[0],
                    appearances: proto.appearances.clone(),
                    variations: proto.variations.clone(),
                    related_motifs: Vec::new(),
                };
                self.motifs.push(new_motif);

                // Remove from proto-motifs.
                let id_clone = motif_id.clone();
                self.proto_motifs.retain(|p| p.id != id_clone);

                return Some(motif_id);
            }
            return None;
        }

        // New theme: register as proto-motif.
        self.proto_motifs.push(ProtoMotif {
            id: motif_id,
            description: description.to_string(),
            appearances: vec![chapter_number],
            variations: vec![variation],
        });
        None
    }

    /// Establish a relationship between two motifs.
    pub fn relate_motifs(&mut self, a: &MotifId, b: &MotifId) {
        if let Some(motif_a) = self.motifs.iter_mut().find(|m| m.motif_id == *a) {
            if !motif_a.related_motifs.contains(b) {
                motif_a.related_motifs.push(b.clone());
            }
        }
        if let Some(motif_b) = self.motifs.iter_mut().find(|m| m.motif_id == *b) {
            if !motif_b.related_motifs.contains(a) {
                motif_b.related_motifs.push(a.clone());
            }
        }
    }

    /// Search for motifs that resonate with the given query text.
    ///
    /// Resonance is measured by word overlap between the query and the motif's
    /// description and variation forms. This is a simple heuristic; in production,
    /// embedding-based similarity would be used.
    pub fn find_resonant(&self, query: &str, max_results: usize) -> Vec<&Motif> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(&Motif, f64)> = self
            .motifs
            .iter()
            .map(|motif| {
                let desc_lower = motif.description.to_lowercase();
                let mut score = 0.0_f64;

                // Description word overlap.
                for word in &query_words {
                    if desc_lower.contains(word) {
                        score += 1.0;
                    }
                }

                // Variation form overlap.
                for variation in &motif.variations {
                    let form_lower = variation.form.to_lowercase();
                    for word in &query_words {
                        if form_lower.contains(word) {
                            score += 0.5;
                        }
                    }
                }

                // Breadth bonus: motifs that appear in many chapters are more
                // likely to be thematically central.
                score += (motif.appearances.len() as f64).ln().max(0.0) * 0.3;

                (motif, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        scored
            .into_iter()
            .take(max_results)
            .map(|(m, _)| m)
            .collect()
    }

    /// Get a motif by its ID.
    pub fn get_motif(&self, id: &MotifId) -> Option<&Motif> {
        self.motifs.iter().find(|m| m.motif_id == *id)
    }

    /// Return all confirmed motifs.
    pub fn all_motifs(&self) -> &[Motif] {
        &self.motifs
    }

    /// Return the number of confirmed motifs.
    pub fn motif_count(&self) -> usize {
        self.motifs.len()
    }

    /// Return motifs that include the given chapter in their appearances.
    pub fn motifs_for_chapter(&self, chapter_number: u64) -> Vec<&Motif> {
        self.motifs
            .iter()
            .filter(|m| m.appearances.contains(&chapter_number))
            .collect()
    }

    /// Collect all chapter numbers referenced by a set of motif IDs,
    /// including chapters from related motifs (transitive resonance, one level).
    pub fn chapters_from_motifs(&self, motif_ids: &[MotifId]) -> Vec<u64> {
        let mut chapters = Vec::new();
        let mut visited = Vec::new();

        for id in motif_ids {
            if let Some(motif) = self.get_motif(id) {
                chapters.extend_from_slice(&motif.appearances);
                visited.push(id.clone());

                // One level of transitive resonance.
                for related_id in &motif.related_motifs {
                    if !visited.contains(related_id) {
                        if let Some(related) = self.get_motif(related_id) {
                            chapters.extend_from_slice(&related.appearances);
                        }
                        visited.push(related_id.clone());
                    }
                }
            }
        }

        chapters.sort_unstable();
        chapters.dedup();
        chapters
    }
}
