//! Memory weaving engine.
//!
//! The loom manages the interplay between warp (persistent) and weft (task-specific)
//! threads. The heddle controller adjusts the weave pattern in real time based on
//! token budget and task familiarity.

mod heddle;
mod pattern;
mod warp;
mod weft;

pub use heddle::{HeddleController, HeddleDecision};
pub use pattern::{PatternConfig, PatternSelector, RelevanceScore};
pub use warp::{WarpQuery, WarpStore};
pub use weft::{ArchivedTask, WeftQuery, WeftStore};

use crate::types::{LoomConfig, TaskId, Thread, ThreadId, ThreadType, WeavePattern};

/// The core memory engine. Manages the interplay between
/// warp (persistent) and weft (task-specific) threads.
pub struct LoomEngine {
    config: LoomConfig,
    warp: WarpStore,
    weft: WeftStore,
    heddle: HeddleController,
}

impl LoomEngine {
    /// Create a new LoomEngine with the given configuration.
    pub fn new(config: LoomConfig) -> Self {
        let warp = WarpStore::new(config.max_warp_threads, config.warp_ttl_seconds);
        let weft = WeftStore::new(config.max_weft_threads);
        let heddle = HeddleController::new(
            config.default_pattern,
            config.promotion_threshold,
            config.tension_decay,
        );
        Self {
            config,
            warp,
            weft,
            heddle,
        }
    }

    /// Retrieve threads relevant to a query, using the active weave pattern
    /// to determine which warp threads are "up" (active) vs "down" (inactive).
    pub fn retrieve(&self, query: &str, pattern: WeavePattern) -> Vec<(Thread, RelevanceScore)> {
        let warp_threads = self.warp.query(&WarpQuery {
            text: query.to_string(),
            pattern,
            max_results: self.config.max_warp_threads,
        });
        let weft_threads = self.weft.query(&WeftQuery {
            text: query.to_string(),
            max_results: self.config.max_weft_threads,
        });

        let mut results: Vec<(Thread, RelevanceScore)> = Vec::new();
        results.extend(warp_threads);
        results.extend(weft_threads);
        results.sort_by(|a, b| {
            b.1.total()
                .partial_cmp(&a.1.total())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Store a new thread. Warp threads go to WarpStore, weft threads to WeftStore.
    pub fn store(&mut self, thread: Thread) -> anyhow::Result<ThreadId> {
        match thread.thread_type {
            ThreadType::Warp => self.warp.store(thread),
            ThreadType::Weft => self.weft.store(thread),
        }
    }

    /// Record an interlacement between two threads.
    pub fn interlace(&mut self, a: &ThreadId, b: &ThreadId) -> anyhow::Result<()> {
        self.warp.record_interlacement(a, b);
        self.weft.record_interlacement(a, b);
        Ok(())
    }

    /// Run the heddle controller to decide if the weave pattern should change.
    pub fn adjust_pattern(&mut self, tokens_used: u64) -> HeddleDecision {
        self.heddle
            .evaluate(tokens_used, self.config.token_budget, &self.weft)
    }

    /// Archive a completed task's weft threads and check for promotion to warp.
    pub fn archive_task(&mut self, task_id: &TaskId) -> anyhow::Result<Vec<ThreadId>> {
        let _archived = self.weft.archive(task_id)?;
        let promoted = self.heddle.check_promotions(&self.weft, &mut self.warp)?;
        Ok(promoted)
    }

    /// Apply tension decay to all warp threads (called periodically).
    pub fn decay_tension(&mut self) {
        self.warp.decay_tension(self.config.tension_decay);
    }
}
