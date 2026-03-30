//! Arc management: grouping related memory entries.
//!
//! An arc groups memory entries that share thematic continuity. Arc dormancy
//! is governed by survival probability: when all entries in an arc have
//! `S(t) < 0.25`, the arc goes dormant. A dormant arc reactivates when new
//! entries are added.

use std::collections::HashMap;

use crate::types::{EntryId, MemoryEntry, MotifId};

/// Survival probability below which an entry is considered fading.
const DORMANCY_THRESHOLD: f64 = 0.25;

/// Unique identifier for an arc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArcId(pub String);

impl std::fmt::Display for ArcId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// State of an arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcState {
    /// Arc has entries with sufficient survival probability.
    Active,
    /// All entries have S(t) < dormancy threshold.
    Dormant,
}

/// A group of thematically related memory entries.
#[derive(Debug, Clone)]
pub struct Arc {
    /// Unique identifier.
    pub id: ArcId,
    /// Human-readable title.
    pub title: String,
    /// Entry IDs belonging to this arc.
    pub entries: Vec<EntryId>,
    /// Motifs associated with this arc (union of entry motifs).
    pub motifs: Vec<MotifId>,
    /// Current state.
    pub state: ArcState,
}

/// Manages the lifecycle of arcs: creation, dormancy, reactivation.
pub struct ArcManager {
    arcs: HashMap<ArcId, Arc>,
}

impl ArcManager {
    /// Create a new, empty arc manager.
    pub fn new() -> Self {
        Self {
            arcs: HashMap::new(),
        }
    }

    /// Create a new arc with the given ID, title, and initial entries.
    /// Returns `false` if an arc with that ID already exists.
    pub fn create_arc(
        &mut self,
        id: ArcId,
        title: String,
        entries: Vec<EntryId>,
        motifs: Vec<MotifId>,
    ) -> bool {
        if self.arcs.contains_key(&id) {
            return false;
        }
        self.arcs.insert(
            id.clone(),
            Arc {
                id,
                title,
                entries,
                motifs,
                state: ArcState::Active,
            },
        );
        true
    }

    /// Add an entry to an existing arc.
    ///
    /// If the arc was dormant, it is reactivated. New motifs from the entry
    /// are merged into the arc's motif list.
    pub fn add_entry(
        &mut self,
        arc_id: &ArcId,
        entry: &MemoryEntry,
    ) -> bool {
        let arc = match self.arcs.get_mut(arc_id) {
            Some(a) => a,
            None => return false,
        };

        if !arc.entries.contains(&entry.id) {
            arc.entries.push(entry.id.clone());
        }

        // Merge motifs from the entry.
        for motif_id in &entry.motifs {
            if !arc.motifs.contains(motif_id) {
                arc.motifs.push(motif_id.clone());
            }
        }

        // Reactivate if dormant (new entries revive the arc).
        if arc.state == ArcState::Dormant {
            arc.state = ArcState::Active;
        }

        true
    }

    /// Evaluate dormancy for all active arcs based on their entries' survival
    /// probability. An arc goes dormant when all its entries have
    /// `S(t) < DORMANCY_THRESHOLD`.
    ///
    /// `entries_by_id` provides the current survival metadata for each entry.
    /// Returns the IDs of newly dormant arcs.
    pub fn evaluate_dormancy(
        &mut self,
        entries_by_id: &HashMap<EntryId, &MemoryEntry>,
    ) -> Vec<ArcId> {
        let mut newly_dormant = Vec::new();

        for arc in self.arcs.values_mut() {
            if arc.state != ArcState::Active {
                continue;
            }

            if arc.entries.is_empty() {
                continue;
            }

            // Check whether all entries have survival below threshold.
            let all_fading = arc.entries.iter().all(|eid| {
                entries_by_id
                    .get(eid)
                    .map(|e| e.survival.current_probability < DORMANCY_THRESHOLD)
                    .unwrap_or(true) // Missing entries count as fading.
            });

            if all_fading {
                arc.state = ArcState::Dormant;
                newly_dormant.push(arc.id.clone());
            }
        }

        newly_dormant
    }

    /// Get an arc by ID.
    pub fn get(&self, id: &ArcId) -> Option<&Arc> {
        self.arcs.get(id)
    }

    /// Return all active arcs.
    pub fn active_arcs(&self) -> Vec<&Arc> {
        self.arcs
            .values()
            .filter(|a| a.state == ArcState::Active)
            .collect()
    }

    /// Return all dormant arcs.
    pub fn dormant_arcs(&self) -> Vec<&Arc> {
        self.arcs
            .values()
            .filter(|a| a.state == ArcState::Dormant)
            .collect()
    }

    /// Return the total number of arcs.
    pub fn count(&self) -> usize {
        self.arcs.len()
    }
}

impl Default for ArcManager {
    fn default() -> Self {
        Self::new()
    }
}
