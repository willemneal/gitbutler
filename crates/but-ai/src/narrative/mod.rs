//! Narrative metadata: motifs, tensions, arcs, and summarization.
//!
//! This module tracks thematic structure across memory entries:
//!
//! - **motif**: Recurring themes that emerge after 3+ appearances, serving
//!   as retrieval anchors beyond keyword matching.
//! - **tension**: Contradictions and unresolved issues with lifecycle
//!   management and Weibull-based urgency scoring.
//! - **arc**: Grouping of related entries with survival-based dormancy.
//! - **summary**: Compaction of arcs preserving motifs and tensions.

pub mod arc;
pub mod motif;
pub mod summary;
pub mod tension;

pub use arc::{ArcId, ArcManager, ArcState};
pub use motif::MotifIndex;
pub use summary::{ArcSummary, Summarizer};
pub use tension::TensionRegistry;

use crate::types::MemoryEntry;

/// Compute the motif resonance score between a query and a memory entry.
///
/// Returns a value in [0, 1] representing how strongly the entry's motifs
/// resonate with the query text. Used as the `motif_resonance` component
/// in the unified retrieval scoring formula.
pub fn motif_resonance(query: &str, entry: &MemoryEntry, index: &MotifIndex) -> f64 {
    index.resonance(query, entry)
}

/// Compute the tension urgency score for a memory entry.
///
/// Returns a value in [0, 1] representing the aggregate urgency of
/// unresolved tensions referenced by the entry. Used as the
/// `tension_boost` component in the unified retrieval scoring formula.
pub fn tension_urgency(entry: &MemoryEntry, registry: &TensionRegistry, now_secs: u64) -> f64 {
    registry.entry_urgency(entry, now_secs)
}
