use crate::types::{Thread, ThreadId, WeavePattern};

use super::pattern::RelevanceScore;

/// Query parameters for warp thread retrieval.
pub struct WarpQuery {
    pub text: String,
    pub pattern: WeavePattern,
    pub max_results: usize,
}

/// Storage and retrieval for warp (persistent) threads.
pub struct WarpStore {
    threads: Vec<Thread>,
    max_threads: usize,
    _default_ttl: u64,
}

impl WarpStore {
    pub fn new(max_threads: usize, default_ttl: u64) -> Self {
        Self {
            threads: Vec::new(),
            max_threads,
            _default_ttl: default_ttl,
        }
    }

    /// Query warp threads relevant to the given text, filtered by the active weave pattern.
    pub fn query(&self, query: &WarpQuery) -> Vec<(Thread, RelevanceScore)> {
        let active_threads = self.threads_for_pattern(query.pattern);
        active_threads
            .into_iter()
            .map(|t| {
                let score = RelevanceScore::compute(&t, &query.text, query.pattern);
                (t, score)
            })
            .take(query.max_results)
            .collect()
    }

    /// Store a warp thread. Evicts the lowest-tension thread if at capacity.
    pub fn store(&mut self, thread: Thread) -> anyhow::Result<ThreadId> {
        let id = thread.id.clone();
        if self.threads.len() >= self.max_threads {
            self.evict_lowest_tension()?;
        }
        self.threads.push(thread);
        Ok(id)
    }

    /// Record that two threads were interlaced (used together in a retrieval).
    pub fn record_interlacement(&mut self, a: &ThreadId, b: &ThreadId) {
        for thread in &mut self.threads {
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

    /// Apply tension decay factor to all threads.
    pub fn decay_tension(&mut self, factor: f64) {
        for thread in &mut self.threads {
            thread.tension *= factor;
        }
    }

    /// Filter threads based on weave pattern.
    fn threads_for_pattern(&self, pattern: WeavePattern) -> Vec<Thread> {
        match pattern {
            WeavePattern::Plain => self.threads.clone(),
            WeavePattern::Twill => self
                .threads
                .iter()
                .filter(|t| t.position % 2 == 0 || t.tension > 0.5)
                .cloned()
                .collect(),
            WeavePattern::Satin => self
                .threads
                .iter()
                .filter(|t| t.tension > 0.7)
                .cloned()
                .collect(),
        }
    }

    fn evict_lowest_tension(&mut self) -> anyhow::Result<()> {
        if self.threads.is_empty() {
            anyhow::bail!("Warp store is empty, cannot evict");
        }
        let min_idx = self
            .threads
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.tension
                    .partial_cmp(&b.tension)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap();
        self.threads.remove(min_idx);
        Ok(())
    }
}
