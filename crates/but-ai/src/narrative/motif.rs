//! Recurring theme/motif tracking across memory entries.
//!
//! A motif is a recurring theme identified across three or more memory entries.
//! Proto-motifs (1-2 appearances) are tracked at reduced weight (0.3x) until
//! they reach the emergence threshold.
//!
//! The motif index provides:
//! - Emergence detection (proto-motif -> motif promotion at 3+ appearances)
//! - Resonance scoring against query strings
//! - Transitive motif traversal via related_motifs

use std::collections::{HashMap, HashSet};

use crate::types::{EntryId, MemoryEntry, Motif, MotifId};

/// Minimum appearances before a proto-motif becomes a full motif.
const EMERGENCE_THRESHOLD: usize = 3;

/// Weight multiplier for proto-motifs in resonance scoring.
const PROTO_MOTIF_WEIGHT: f64 = 0.3;

/// Index of motifs and proto-motifs for efficient lookup and resonance scoring.
pub struct MotifIndex {
    /// Confirmed motifs (3+ appearances).
    motifs: HashMap<MotifId, Motif>,
    /// Proto-motifs: themes with 1-2 appearances, not yet promoted.
    proto_motifs: HashMap<MotifId, ProtoMotif>,
}

/// A theme that has appeared fewer than `EMERGENCE_THRESHOLD` times.
#[derive(Debug, Clone)]
struct ProtoMotif {
    description: String,
    appearances: Vec<EntryId>,
    related_motifs: Vec<MotifId>,
}

impl MotifIndex {
    /// Create an empty motif index.
    pub fn new() -> Self {
        Self {
            motifs: HashMap::new(),
            proto_motifs: HashMap::new(),
        }
    }

    /// Record that a motif appears in a memory entry.
    ///
    /// If the motif has not been seen before, it is created as a proto-motif.
    /// When it reaches `EMERGENCE_THRESHOLD` appearances it is promoted to a
    /// full motif. Returns `true` if the motif was promoted in this call.
    pub fn record_appearance(
        &mut self,
        motif_id: &MotifId,
        description: &str,
        entry_id: &EntryId,
    ) -> bool {
        // Already a full motif -- just add the appearance.
        if let Some(motif) = self.motifs.get_mut(motif_id) {
            if !motif.appearances.contains(entry_id) {
                motif.appearances.push(entry_id.clone());
            }
            return false;
        }

        // Existing proto-motif -- add appearance, maybe promote.
        if let Some(proto) = self.proto_motifs.get_mut(motif_id) {
            if !proto.appearances.contains(entry_id) {
                proto.appearances.push(entry_id.clone());
            }
            if proto.appearances.len() >= EMERGENCE_THRESHOLD {
                let promoted = Motif {
                    id: motif_id.clone(),
                    description: proto.description.clone(),
                    appearances: proto.appearances.clone(),
                    related_motifs: proto.related_motifs.clone(),
                };
                self.motifs.insert(motif_id.clone(), promoted);
                self.proto_motifs.remove(motif_id);
                return true;
            }
            return false;
        }

        // Brand new theme -- create proto-motif.
        self.proto_motifs.insert(
            motif_id.clone(),
            ProtoMotif {
                description: description.to_string(),
                appearances: vec![entry_id.clone()],
                related_motifs: Vec::new(),
            },
        );
        false
    }

    /// Establish a bidirectional relationship between two motifs (or proto-motifs).
    pub fn relate(&mut self, a: &MotifId, b: &MotifId) {
        // Helper: add b to a's related list if a exists in either map.
        fn link(
            motifs: &mut HashMap<MotifId, Motif>,
            protos: &mut HashMap<MotifId, ProtoMotif>,
            from: &MotifId,
            to: &MotifId,
        ) {
            if let Some(m) = motifs.get_mut(from) {
                if !m.related_motifs.contains(to) {
                    m.related_motifs.push(to.clone());
                }
            } else if let Some(p) = protos.get_mut(from) {
                if !p.related_motifs.contains(to) {
                    p.related_motifs.push(to.clone());
                }
            }
        }
        link(&mut self.motifs, &mut self.proto_motifs, a, b);
        link(&mut self.motifs, &mut self.proto_motifs, b, a);
    }

    /// Look up a confirmed motif by ID.
    pub fn get(&self, id: &MotifId) -> Option<&Motif> {
        self.motifs.get(id)
    }

    /// Return all confirmed motifs.
    pub fn all_motifs(&self) -> impl Iterator<Item = &Motif> {
        self.motifs.values()
    }

    /// Return the number of confirmed motifs.
    pub fn motif_count(&self) -> usize {
        self.motifs.len()
    }

    /// Return the number of proto-motifs.
    pub fn proto_motif_count(&self) -> usize {
        self.proto_motifs.len()
    }

    /// Check whether a motif ID corresponds to a confirmed motif (not proto).
    pub fn is_emerged(&self, id: &MotifId) -> bool {
        self.motifs.contains_key(id)
    }

    /// Compute motif resonance between a query string and a memory entry.
    ///
    /// Resonance is the proportion of the entry's motifs that also appear
    /// in the query (by word overlap with the motif description). Proto-motifs
    /// contribute at `PROTO_MOTIF_WEIGHT` (0.3x).
    pub fn resonance(&self, query: &str, entry: &MemoryEntry) -> f64 {
        if entry.motifs.is_empty() {
            return 0.0;
        }

        let query_lower = query.to_lowercase();
        let query_words: HashSet<&str> = query_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return 0.0;
        }

        let mut total = 0.0_f64;
        let mut count = 0.0_f64;

        for motif_id in &entry.motifs {
            if let Some(motif) = self.motifs.get(motif_id) {
                count += 1.0;
                let desc_lower = motif.description.to_lowercase();
                let desc_words: HashSet<&str> = desc_lower.split_whitespace().collect();
                let overlap = query_words.intersection(&desc_words).count();
                if overlap > 0 {
                    total += (overlap as f64 / desc_words.len().max(1) as f64).min(1.0);
                }
            } else if let Some(proto) = self.proto_motifs.get(motif_id) {
                count += PROTO_MOTIF_WEIGHT;
                let desc_lower = proto.description.to_lowercase();
                let desc_words: HashSet<&str> = desc_lower.split_whitespace().collect();
                let overlap = query_words.intersection(&desc_words).count();
                if overlap > 0 {
                    total += PROTO_MOTIF_WEIGHT
                        * (overlap as f64 / desc_words.len().max(1) as f64).min(1.0);
                }
            }
        }

        if count == 0.0 {
            return 0.0;
        }

        (total / count).clamp(0.0, 1.0)
    }

    /// Collect all entry IDs reachable from a set of motif IDs, including
    /// one level of transitive traversal via `related_motifs`.
    pub fn entries_from_motifs(&self, motif_ids: &[MotifId]) -> Vec<EntryId> {
        let mut entries = Vec::new();
        let mut visited: HashSet<&MotifId> = HashSet::new();

        for id in motif_ids {
            if visited.contains(id) {
                continue;
            }
            visited.insert(id);

            if let Some(motif) = self.motifs.get(id) {
                entries.extend(motif.appearances.iter().cloned());

                // One level of transitive traversal.
                for related_id in &motif.related_motifs {
                    if visited.contains(related_id) {
                        continue;
                    }
                    visited.insert(related_id);
                    if let Some(related) = self.motifs.get(related_id) {
                        entries.extend(related.appearances.iter().cloned());
                    }
                }
            }
        }

        // Deduplicate while preserving order.
        let mut seen = HashSet::new();
        entries.retain(|e| seen.insert(e.0.clone()));
        entries
    }
}

impl Default for MotifIndex {
    fn default() -> Self {
        Self::new()
    }
}
