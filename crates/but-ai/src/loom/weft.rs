use serde::{Deserialize, Serialize};

use crate::types::{TaskId, Thread, ThreadId, WeavePattern};

use super::pattern::RelevanceScore;

/// Query parameters for weft thread retrieval.
pub struct WeftQuery {
    pub text: String,
    pub max_results: usize,
}

/// Archived task data: compressed weft with interlacement metadata preserved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedTask {
    pub task_id: TaskId,
    pub summary: String,
    pub interlacement_pattern: Vec<(ThreadId, ThreadId)>,
    pub thread_count: usize,
}

/// Storage and retrieval for weft (task-specific) threads.
pub struct WeftStore {
    /// Active weft threads for the current task.
    active: Vec<Thread>,
    /// Archived tasks (compressed weft with interlacement metadata).
    archive: Vec<ArchivedTask>,
    max_threads: usize,
}

impl WeftStore {
    pub fn new(max_threads: usize) -> Self {
        Self {
            active: Vec::new(),
            archive: Vec::new(),
            max_threads,
        }
    }

    /// Query active weft threads relevant to the given text.
    pub fn query(&self, query: &WeftQuery) -> Vec<(Thread, RelevanceScore)> {
        self.active
            .iter()
            .map(|t| {
                let score = RelevanceScore::compute(t, &query.text, WeavePattern::Plain);
                (t.clone(), score)
            })
            .take(query.max_results)
            .collect()
    }

    /// Store a new weft thread for the current task.
    pub fn store(&mut self, thread: Thread) -> anyhow::Result<ThreadId> {
        let id = thread.id.clone();
        if self.active.len() >= self.max_threads {
            anyhow::bail!("Weft store at capacity ({} threads)", self.max_threads);
        }
        self.active.push(thread);
        Ok(id)
    }

    /// Record interlacement between threads (weft side).
    pub fn record_interlacement(&mut self, a: &ThreadId, b: &ThreadId) {
        for thread in &mut self.active {
            if thread.id == *a || thread.id == *b {
                thread.interlacement_count += 1;
                if !thread.connected_threads.contains(a) && thread.id != *a {
                    thread.connected_threads.push(a.clone());
                }
                if !thread.connected_threads.contains(b) && thread.id != *b {
                    thread.connected_threads.push(b.clone());
                }
            }
        }
    }

    /// Archive a completed task's weft threads. Compresses content,
    /// preserves interlacement metadata.
    pub fn archive(&mut self, task_id: &TaskId) -> anyhow::Result<Vec<ThreadId>> {
        let archived_ids: Vec<ThreadId> = self.active.iter().map(|t| t.id.clone()).collect();
        let interlacements: Vec<(ThreadId, ThreadId)> = self
            .active
            .iter()
            .flat_map(|t| {
                t.connected_threads
                    .iter()
                    .map(|c| (t.id.clone(), c.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();

        let summary = self.compress_active();

        self.archive.push(ArchivedTask {
            task_id: task_id.clone(),
            summary,
            interlacement_pattern: interlacements,
            thread_count: self.active.len(),
        });
        self.active.clear();
        Ok(archived_ids)
    }

    /// Return the number of active weft threads.
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Access archived tasks for pattern detection.
    pub fn archived_tasks(&self) -> &[ArchivedTask] {
        &self.archive
    }

    /// Compress active weft threads into a summary string.
    fn compress_active(&self) -> String {
        self.active
            .iter()
            .map(|t| t.content.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}
