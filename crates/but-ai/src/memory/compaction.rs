//! Circulation-based compaction tiers.
//!
//! When context windows fill up, not all memories deserve equal space.
//! Compaction groups entries by their circulation frequency:
//!
//! - **High** (>5 accesses): preserved in full
//! - **Medium** (1-5 accesses): content summarized, metadata kept
//! - **Low** (0 accesses): metadata-only (call number + subject headings)
//!
//! This mirrors library collection development: heavily-circulated books
//! stay on the main shelves, rarely-used ones go to storage, uncirculated
//! ones are deaccessioned.

use crate::types::{CallNumber, EntryId, MemoryEntry, MemoryState, MemoryStore};

/// Threshold for high-circulation entries (>5 accesses = full content).
pub const HIGH_CIRCULATION_THRESHOLD: u64 = 5;

/// Entries grouped by circulation tier for context compaction.
#[derive(Debug, Clone)]
pub struct CompactionTiers {
    /// High-circulation entries: preserved in full.
    pub high: Vec<MemoryEntry>,
    /// Medium-circulation entries: summary only.
    pub medium: Vec<CompactionSummary>,
    /// Low-circulation entries: metadata only.
    pub low: Vec<MetadataOnly>,
}

/// A summarized entry for medium-tier compaction.
#[derive(Debug, Clone)]
pub struct CompactionSummary {
    /// Entry ID.
    pub id: EntryId,
    /// Call number.
    pub call_number: CallNumber,
    /// Subject headings.
    pub subject_headings: Vec<String>,
    /// First 100 chars of content.
    pub content_summary: String,
    /// Access count.
    pub access_count: u64,
}

/// Metadata-only representation for low-tier compaction.
#[derive(Debug, Clone)]
pub struct MetadataOnly {
    /// Entry ID.
    pub id: EntryId,
    /// Call number.
    pub call_number: CallNumber,
    /// Subject headings.
    pub subject_headings: Vec<String>,
}

/// Compact all alive entries in the store into tiered representations.
pub fn compact<S: MemoryStore>(store: &S) -> anyhow::Result<CompactionTiers> {
    let alive_ids = store.list(Some(MemoryState::Alive))?;
    let mut high = Vec::new();
    let mut medium = Vec::new();
    let mut low = Vec::new();

    for id in &alive_ids {
        if let Some(entry) = store.load(id)? {
            match entry.access_count {
                n if n > HIGH_CIRCULATION_THRESHOLD => {
                    high.push(entry);
                }
                1..=5 => {
                    medium.push(CompactionSummary {
                        id: entry.id.clone(),
                        call_number: entry.classification.call_number.clone(),
                        subject_headings: entry.classification.subject_headings.clone(),
                        content_summary: entry.content.chars().take(100).collect(),
                        access_count: entry.access_count,
                    });
                }
                _ => {
                    low.push(MetadataOnly {
                        id: entry.id.clone(),
                        call_number: entry.classification.call_number.clone(),
                        subject_headings: entry.classification.subject_headings.clone(),
                    });
                }
            }
        }
    }

    Ok(CompactionTiers { high, medium, low })
}

/// Estimate the token cost of a compaction tier set.
///
/// Rough estimates: full entry ~200 tokens, summary ~50, metadata ~15.
pub fn estimate_tokens(tiers: &CompactionTiers) -> u64 {
    let high_tokens = tiers.high.len() as u64 * 200;
    let medium_tokens = tiers.medium.len() as u64 * 50;
    let low_tokens = tiers.low.len() as u64 * 15;
    high_tokens + medium_tokens + low_tokens
}

/// Render a compaction tier set as a compact text representation.
pub fn render_compact(tiers: &CompactionTiers) -> String {
    let mut out = String::new();

    if !tiers.high.is_empty() {
        out.push_str(&format!("## High circulation ({} entries)\n\n", tiers.high.len()));
        for entry in &tiers.high {
            out.push_str(&format!(
                "- [{}] {} (accesses: {})\n  {}\n\n",
                entry.classification.call_number,
                entry.classification.subject_headings.join(", "),
                entry.access_count,
                entry.content,
            ));
        }
    }

    if !tiers.medium.is_empty() {
        out.push_str(&format!(
            "## Medium circulation ({} entries)\n\n",
            tiers.medium.len()
        ));
        for summary in &tiers.medium {
            out.push_str(&format!(
                "- [{}] {} (accesses: {}): {}...\n",
                summary.call_number,
                summary.subject_headings.join(", "),
                summary.access_count,
                summary.content_summary,
            ));
        }
        out.push('\n');
    }

    if !tiers.low.is_empty() {
        out.push_str(&format!(
            "## Low circulation ({} entries)\n\n",
            tiers.low.len()
        ));
        for meta in &tiers.low {
            out.push_str(&format!(
                "- [{}] {}\n",
                meta.call_number,
                meta.subject_headings.join(", "),
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_entry(id: &str, access_count: u64) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: "test content for compaction".into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec!["testing".into()],
                call_number: CallNumber::parse("TEST.UNIT"),
                controlled_vocab: false,
            },
            see_also: Vec::new(),
            motifs: Vec::new(),
            tension_refs: Vec::new(),
            survival: SurvivalMetadata {
                distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
                current_probability: 0.9,
                hazard_rate: 0.01,
                surprise_index: 0.0,
                goodness_of_fit: 0.95,
            },
            state: MemoryState::Alive,
            consensus_citations: 0,
            access_count,
            source_commit: None,
        }
    }

    #[test]
    fn compaction_tiers() {
        let store = InMemoryStore::new();
        store.store(&make_entry("high", 10)).unwrap();
        store.store(&make_entry("medium", 3)).unwrap();
        store.store(&make_entry("low", 0)).unwrap();

        let tiers = compact(&store).unwrap();
        assert_eq!(tiers.high.len(), 1);
        assert_eq!(tiers.medium.len(), 1);
        assert_eq!(tiers.low.len(), 1);
    }

    #[test]
    fn token_estimation() {
        let store = InMemoryStore::new();
        store.store(&make_entry("high", 10)).unwrap();
        store.store(&make_entry("medium", 3)).unwrap();
        store.store(&make_entry("low", 0)).unwrap();

        let tiers = compact(&store).unwrap();
        let tokens = estimate_tokens(&tiers);
        // 1*200 + 1*50 + 1*15 = 265
        assert_eq!(tokens, 265);
    }
}
