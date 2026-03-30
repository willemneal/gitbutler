//! Architect agent -- read-only planning.
//!
//! Given a task description, the architect queries memory for relevant context
//! and produces a structured `TaskPlan` with approach, files to touch, and
//! estimated complexity. Inspired by Org 083's loom pattern where the warp
//! (persistent memory) shapes the plan before any shuttle pass begins.

use crate::types::{MemoryRetriever, RelevanceWeights};

use super::{Complexity, TaskPlan};

/// Default maximum memory results to consider during planning.
const MAX_MEMORY_RESULTS: usize = 20;

/// Token estimate multipliers per complexity level.
const SIMPLE_TOKEN_ESTIMATE: u64 = 4_000;
const MODERATE_TOKEN_ESTIMATE: u64 = 12_000;
const COMPLEX_TOKEN_ESTIMATE: u64 = 24_000;

/// The architect agent. Uses read-only tools to analyze a task and
/// produce a plan before any implementation begins.
pub struct Architect;

impl Architect {
    /// Analyze a task description against memory and produce a plan.
    ///
    /// The architect:
    /// 1. Queries memory for relevant context (prior work, tensions, motifs).
    /// 2. Estimates complexity from memory overlap and task description length.
    /// 3. Extracts likely files to touch from memory entries' source commits.
    /// 4. Formulates an approach summary.
    pub fn plan(task: &str, retriever: &dyn MemoryRetriever) -> anyhow::Result<TaskPlan> {
        let weights = RelevanceWeights::default();
        let memories = retriever.retrieve(task, MAX_MEMORY_RESULTS, &weights)?;

        // Estimate complexity from memory coverage and task scope.
        let complexity = estimate_complexity(task, memories.len());

        // Extract files from memory context (source commits, content hints).
        let files = extract_likely_files(task, &memories);

        // Build approach from task + relevant memory entries.
        let approach = build_approach(task, &memories);

        let estimated_tokens = match complexity {
            Complexity::Simple => SIMPLE_TOKEN_ESTIMATE,
            Complexity::Moderate => MODERATE_TOKEN_ESTIMATE,
            Complexity::Complex => COMPLEX_TOKEN_ESTIMATE,
        };

        Ok(TaskPlan {
            approach,
            files,
            complexity,
            estimated_tokens,
        })
    }
}

/// Estimate task complexity from description characteristics and memory overlap.
///
/// - **Simple**: short description, strong memory overlap (familiar territory).
/// - **Moderate**: medium description or partial memory overlap.
/// - **Complex**: long description, cross-cutting concerns, or novel territory.
fn estimate_complexity(task: &str, memory_hits: usize) -> Complexity {
    let word_count = task.split_whitespace().count();
    let has_cross_cutting = task.contains("cross-repo")
        || task.contains("coordination")
        || task.contains("migration")
        || task.contains("refactor");

    if has_cross_cutting || (word_count > 50 && memory_hits < 3) {
        Complexity::Complex
    } else if word_count > 20 || memory_hits < 5 {
        Complexity::Moderate
    } else {
        Complexity::Simple
    }
}

/// Extract likely files to touch from memory entries and task description.
///
/// Looks for file-path-like patterns in memory content and the task
/// description itself (e.g., `src/foo/bar.rs`, `crates/x/y.rs`).
fn extract_likely_files(
    task: &str,
    memories: &[crate::types::ScoredMemory],
) -> Vec<String> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Extract paths from task description.
    for word in task.split_whitespace() {
        if looks_like_path(word) {
            let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
            if seen.insert(clean.to_string()) {
                files.push(clean.to_string());
            }
        }
    }

    // Extract paths from top-scoring memory entries.
    for scored in memories.iter().take(10) {
        // Check source commit references for file context.
        if let Some(ref _commit) = scored.entry.source_commit {
            // The commit hash itself isn't a file, but entries with commits
            // tend to have file paths in their content.
        }
        for word in scored.entry.content.split_whitespace() {
            if looks_like_path(word) {
                let clean = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-');
                if seen.insert(clean.to_string()) {
                    files.push(clean.to_string());
                }
            }
        }
    }

    files
}

/// Heuristic: does this token look like a file path?
fn looks_like_path(s: &str) -> bool {
    let s = s.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == ',');
    (s.contains('/') || s.contains('.'))
        && s.len() > 3
        && !s.starts_with("http")
        && !s.starts_with("git@")
        && s.chars().all(|c| c.is_alphanumeric() || "/._ -".contains(c))
}

/// Build an approach description from the task and relevant memories.
fn build_approach(task: &str, memories: &[crate::types::ScoredMemory]) -> String {
    let mut approach = format!("Task: {task}");

    if memories.is_empty() {
        approach.push_str("\n\nNo prior context found -- greenfield implementation.");
        return approach;
    }

    let high_score_count = memories.iter().filter(|m| m.score > 0.5).count();
    let has_tensions = memories
        .iter()
        .any(|m| !m.entry.tension_refs.is_empty());

    approach.push_str(&format!(
        "\n\nContext: {} relevant memories ({} high-confidence).",
        memories.len(),
        high_score_count,
    ));

    if has_tensions {
        approach.push_str(" Active tensions detected -- validation pass recommended.");
    }

    // Summarize top 3 memory entries.
    for (i, scored) in memories.iter().take(3).enumerate() {
        let preview: String = scored.entry.content.chars().take(80).collect();
        approach.push_str(&format!(
            "\n  [{}] (score={:.2}) {}...",
            i + 1,
            scored.score,
            preview,
        ));
    }

    approach
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::retrieval::RetrievalEngine;
    use crate::memory::see_also::SeeAlsoGraph;
    use crate::memory::store::InMemoryStore;
    use crate::types::*;

    fn make_entry(id: &str, content: &str) -> MemoryEntry {
        MemoryEntry {
            id: EntryId(id.into()),
            agent: AgentId("test".into()),
            content: content.into(),
            created_at: "2026-03-29T00:00:00Z".into(),
            last_accessed: "2026-03-29T00:00:00Z".into(),
            classification: Classification {
                subject_headings: vec!["testing".into()],
                call_number: CallNumber::parse("TEST.UNIT"),
                controlled_vocab: true,
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
            access_count: 5,
            source_commit: None,
        }
    }

    #[test]
    fn plan_with_empty_memory() {
        let store = InMemoryStore::new();
        let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
        let plan = Architect::plan("add authentication middleware", &engine).unwrap();
        assert_eq!(plan.complexity, Complexity::Moderate);
        assert!(plan.approach.contains("No prior context"));
    }

    #[test]
    fn plan_with_relevant_memory() {
        let store = InMemoryStore::new();
        for i in 0..6 {
            store
                .store(&make_entry(
                    &format!("e{i}"),
                    "authentication middleware JWT handler",
                ))
                .unwrap();
        }
        let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
        let plan = Architect::plan("fix auth bug", &engine).unwrap();
        assert_eq!(plan.complexity, Complexity::Simple);
        assert!(plan.approach.contains("relevant memories"));
    }

    #[test]
    fn complexity_cross_cutting() {
        assert_eq!(
            estimate_complexity("cross-repo migration of auth system", 0),
            Complexity::Complex
        );
    }

    #[test]
    fn path_detection() {
        assert!(looks_like_path("src/auth/middleware.rs"));
        assert!(!looks_like_path("hello"));
        assert!(!looks_like_path("https://example.com"));
    }
}
