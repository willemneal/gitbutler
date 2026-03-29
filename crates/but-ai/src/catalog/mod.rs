//! The card catalog — the heart of ShelfOS memory.
//!
//! This module implements the core cataloging operations:
//! - **Classification**: Five simultaneous classification systems
//! - **Call numbers**: Hierarchical knowledge addressing
//! - **See also**: Bidirectional cross-reference graph
//! - **Controlled vocabulary**: Term normalization and expansion
//!
//! The catalog is not just queried — it is continuously updated.
//! Every tool call interaction potentially produces new knowledge
//! that is classified in real time.

pub mod call_number;
pub mod classification;
pub mod controlled_vocab;
pub mod see_also;

use crate::types::{
    CallNumber, CatalogConfig, CatalogEntry, CirculationRecord, ItemId, ReferenceShelf,
    RetrievalWeights, ScoredEntry,
};
use call_number::CallNumberHierarchy;
use controlled_vocab::VocabularyIndex;
use see_also::SeeAlsoGraph;
use std::collections::HashMap;

/// The card catalog: indexes, stores, and retrieves memories.
///
/// Combines the five classification systems, the "see also" graph,
/// the call number hierarchy, and the controlled vocabulary into
/// a unified retrieval interface.
#[derive(Debug, Clone)]
pub struct CardCatalog {
    /// All catalog entries, keyed by item ID.
    entries: HashMap<ItemId, CatalogEntry>,
    /// The "see also" cross-reference graph.
    graph: SeeAlsoGraph,
    /// The call number hierarchy.
    hierarchy: CallNumberHierarchy,
    /// The controlled vocabulary index.
    vocabulary: VocabularyIndex,
    /// Configuration.
    config: CatalogConfig,
    /// Counter for generating unique item IDs.
    next_id: u64,
}

impl CardCatalog {
    /// Create a new empty catalog with default configuration.
    pub fn new(config: CatalogConfig) -> Self {
        let vocab = controlled_vocab::default_vocabulary();
        let vocabulary = VocabularyIndex::from_vocabulary(&vocab, 200);
        let hierarchy = CallNumberHierarchy::default_hierarchy(config.call_number_depth);
        let graph = SeeAlsoGraph::new(config.max_see_also);

        Self {
            entries: HashMap::new(),
            graph,
            hierarchy,
            vocabulary,
            config,
            next_id: 1,
        }
    }

    /// Generate a new unique item ID.
    fn next_item_id(&mut self) -> ItemId {
        let id = ItemId(format!("mem_{:04}", self.next_id));
        self.next_id += 1;
        id
    }

    /// Accession (add) a new memory to the catalog.
    ///
    /// The entry is classified, registered in the hierarchy, and
    /// subject headings are normalized through the controlled vocabulary.
    pub fn accession(&mut self, mut entry: CatalogEntry) -> ItemId {
        // Assign an ID if the entry doesn't have one.
        if entry.item_id.0.is_empty() {
            entry.item_id = self.next_item_id();
        }

        // Normalize subject headings through controlled vocabulary.
        entry.classification.subject_headings = self
            .vocabulary
            .normalize_subjects(&entry.classification.subject_headings);

        // Enforce max subject headings.
        entry
            .classification
            .subject_headings
            .truncate(self.config.max_subject_headings);

        // Register the call number in the hierarchy.
        self.hierarchy
            .register(&entry.classification.call_number);

        let id = entry.item_id.clone();
        self.entries.insert(id.clone(), entry);
        id
    }

    /// Deaccession (archive) a memory. The entry is marked as deaccessioned
    /// but not deleted — it can still be retrieved for historical research.
    pub fn deaccession(&mut self, item_id: &ItemId) -> bool {
        if let Some(entry) = self.entries.get_mut(item_id) {
            entry.deaccessioned = true;
            // Remove from the active graph but keep the entry itself.
            self.graph.remove_item(item_id);
            true
        } else {
            false
        }
    }

    /// Retrieve a single entry by ID, recording a checkout event.
    pub fn checkout(&mut self, item_id: &ItemId, context: &str) -> Option<&CatalogEntry> {
        if let Some(entry) = self.entries.get_mut(item_id) {
            if self.config.circulation_tracking {
                entry.circulation.total_checkouts += 1;
                entry.circulation.last_checkout = Some(context.to_string());
                if !entry.circulation.checkout_contexts.contains(&context.to_string()) {
                    entry.circulation.checkout_contexts.push(context.to_string());
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Search the catalog and produce a scored, ranked reference shelf.
    ///
    /// This is the primary retrieval method. It combines:
    /// 1. Subject heading matching (with vocabulary expansion)
    /// 2. Call number proximity
    /// 3. "See also" graph traversal
    /// 4. Circulation frequency
    /// 5. Freshness
    pub fn search(
        &self,
        query_subjects: &[String],
        query_call_number: Option<&CallNumber>,
        max_results: usize,
    ) -> ReferenceShelf {
        let weights = RetrievalWeights::default();

        // Expand query subjects through controlled vocabulary.
        let expanded_subjects = self.vocabulary.expand_query(query_subjects);

        // Find the maximum circulation count for normalization.
        let max_circ = self
            .entries
            .values()
            .filter(|e| !e.deaccessioned)
            .map(|e| e.circulation.total_checkouts)
            .max()
            .unwrap_or(1);

        // Determine "see also" hop counts for entries reachable from
        // entries that directly match the query.
        let mut see_also_hops: HashMap<ItemId, u32> = HashMap::new();
        for entry in self.entries.values() {
            if entry.deaccessioned {
                continue;
            }
            let matches_subject = expanded_subjects.iter().any(|qs| {
                entry
                    .classification
                    .subject_headings
                    .iter()
                    .any(|sh| sh.eq_ignore_ascii_case(qs))
            });
            if matches_subject {
                see_also_hops.insert(entry.item_id.clone(), 0);
                // Traverse the graph from this entry.
                for (reachable, hops) in self.graph.traverse(&entry.item_id, 2) {
                    let existing = see_also_hops.entry(reachable).or_insert(hops);
                    if hops < *existing {
                        *existing = hops;
                    }
                }
            }
        }

        // Score all active entries.
        let mut scored: Vec<ScoredEntry> = self
            .entries
            .values()
            .filter(|e| !e.deaccessioned)
            .map(|entry| {
                let hops = see_also_hops.get(&entry.item_id).copied();
                classification::score_entry(
                    entry,
                    &expanded_subjects,
                    query_call_number,
                    hops,
                    max_circ,
                    0, // simplified: use 0 for now_epoch
                    0, // simplified: use 0 for entry_epoch
                    1, // avoid division by zero
                    &weights,
                )
            })
            .collect();

        // Sort by score descending.
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_results);

        // Generate a finding aid.
        let finding_aid = generate_finding_aid(&scored, query_subjects);

        ReferenceShelf {
            entries: scored,
            finding_aid,
        }
    }

    /// Get a reference to the "see also" graph.
    pub fn graph(&self) -> &SeeAlsoGraph {
        &self.graph
    }

    /// Get a mutable reference to the "see also" graph.
    pub fn graph_mut(&mut self) -> &mut SeeAlsoGraph {
        &mut self.graph
    }

    /// Get a reference to the vocabulary index.
    pub fn vocabulary(&self) -> &VocabularyIndex {
        &self.vocabulary
    }

    /// Get a mutable reference to the vocabulary index.
    pub fn vocabulary_mut(&mut self) -> &mut VocabularyIndex {
        &mut self.vocabulary
    }

    /// Get a reference to the call number hierarchy.
    pub fn hierarchy(&self) -> &CallNumberHierarchy {
        &self.hierarchy
    }

    /// Get a mutable reference to the call number hierarchy.
    pub fn hierarchy_mut(&mut self) -> &mut CallNumberHierarchy {
        &mut self.hierarchy
    }

    /// Number of active (non-deaccessioned) entries.
    pub fn active_count(&self) -> usize {
        self.entries.values().filter(|e| !e.deaccessioned).count()
    }

    /// Total number of entries (including deaccessioned).
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Get an entry by ID without recording a checkout.
    pub fn get(&self, item_id: &ItemId) -> Option<&CatalogEntry> {
        self.entries.get(item_id)
    }

    /// Get all active entries.
    pub fn active_entries(&self) -> impl Iterator<Item = &CatalogEntry> {
        self.entries.values().filter(|e| !e.deaccessioned)
    }

    /// Find entries that should be reviewed for deaccession.
    ///
    /// Returns entries whose circulation count is zero and that have
    /// not been accessed since creation.
    pub fn deaccession_candidates(&self) -> Vec<&CatalogEntry> {
        self.entries
            .values()
            .filter(|e| !e.deaccessioned)
            .filter(|e| {
                e.circulation.total_checkouts == 0
                    && e.classification.temporal.last_accessed
                        == e.classification.temporal.created
            })
            .collect()
    }

    /// Summarize entries for compaction survival.
    ///
    /// Returns entries grouped by circulation tier:
    /// - High (>5 checkouts): preserved in full
    /// - Medium (1-5 checkouts): summary only
    /// - Low (0 checkouts): call number only
    pub fn compaction_tiers(&self) -> CompactionTiers {
        let mut high = Vec::new();
        let mut medium = Vec::new();
        let mut low = Vec::new();

        for entry in self.entries.values().filter(|e| !e.deaccessioned) {
            match entry.circulation.total_checkouts {
                n if n > 5 => high.push(entry.clone()),
                1..=5 => medium.push(CompactionSummary {
                    item_id: entry.item_id.clone(),
                    call_number: entry.classification.call_number.clone(),
                    subject_headings: entry.classification.subject_headings.clone(),
                    content_summary: entry
                        .content
                        .chars()
                        .take(100)
                        .collect::<String>(),
                }),
                _ => low.push(entry.classification.call_number.clone()),
            }
        }

        CompactionTiers { high, medium, low }
    }
}

/// Generate a natural-language finding aid for a set of scored entries.
fn generate_finding_aid(entries: &[ScoredEntry], query_subjects: &[String]) -> String {
    if entries.is_empty() {
        return format!(
            "No catalog entries found for query subjects: {}.",
            query_subjects.join(", ")
        );
    }

    let mut aid = format!(
        "Found {} entries relevant to: {}.",
        entries.len(),
        query_subjects.join(", ")
    );

    // Summarize the top entries.
    let top = entries.iter().take(3);
    for (i, scored) in top.enumerate() {
        aid.push_str(&format!(
            " [{}] {} (score: {:.2}, call number: {}).",
            i + 1,
            scored.entry.content.chars().take(60).collect::<String>(),
            scored.score,
            scored.entry.classification.call_number,
        ));
    }

    // Note see-also connections.
    let has_see_also = entries.iter().any(|s| !s.entry.see_also.is_empty());
    if has_see_also {
        aid.push_str(" Cross-references available via 'see also' links.");
    }

    aid
}

/// Entries grouped by circulation tier for context compaction.
#[derive(Debug, Clone)]
pub struct CompactionTiers {
    /// High-circulation entries (>5 checkouts): preserved in full.
    pub high: Vec<CatalogEntry>,
    /// Medium-circulation entries (1-5 checkouts): summary only.
    pub medium: Vec<CompactionSummary>,
    /// Low-circulation entries (0 checkouts): call number only.
    pub low: Vec<CallNumber>,
}

/// A summarized entry for medium-tier compaction.
#[derive(Debug, Clone)]
pub struct CompactionSummary {
    pub item_id: ItemId,
    pub call_number: CallNumber,
    pub subject_headings: Vec<String>,
    pub content_summary: String,
}

/// Create a new catalog entry with the given content and classification.
///
/// This is a convenience function for building entries before accession.
pub fn new_entry(
    content: String,
    call_number: CallNumber,
    subject_headings: Vec<String>,
    confidence: f64,
    now: &str,
) -> CatalogEntry {
    CatalogEntry {
        item_id: ItemId(String::new()), // Will be assigned during accession.
        content,
        classification: crate::types::Classification {
            subject_headings,
            call_number,
            source: crate::types::SourceClassification {
                task: None,
                agent: None,
                branch: None,
                tool: None,
            },
            temporal: crate::types::TemporalClassification {
                created: now.to_string(),
                last_accessed: now.to_string(),
                last_validated: now.to_string(),
            },
        },
        see_also: Vec::new(),
        circulation: CirculationRecord::default(),
        ttl: "30d".to_string(),
        confidence,
        deaccessioned: false,
    }
}
