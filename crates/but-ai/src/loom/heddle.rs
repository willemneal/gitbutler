use std::collections::HashMap;

use crate::types::{ThreadId, WeavePattern};

use super::warp::WarpStore;
use super::weft::WeftStore;

/// The decision produced by the heddle controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeddleDecision {
    /// Continue with the current pattern.
    Maintain(WeavePattern),
    /// Downgrade to a simpler pattern due to budget pressure.
    Downgrade {
        from: WeavePattern,
        to: WeavePattern,
    },
    /// Halt the loom -- budget critically low.
    Halt,
}

/// Controls dynamic pattern adjustment based on token budget and task progress.
pub struct HeddleController {
    active_pattern: WeavePattern,
    promotion_threshold: u32,
    _tension_decay: f64,
}

impl HeddleController {
    pub fn new(default_pattern: WeavePattern, promotion_threshold: u32, tension_decay: f64) -> Self {
        Self {
            active_pattern: default_pattern,
            promotion_threshold,
            _tension_decay: tension_decay,
        }
    }

    /// Evaluate whether the weave pattern should change based on budget consumption.
    pub fn evaluate(
        &mut self,
        tokens_used: u64,
        token_budget: u64,
        _weft: &WeftStore,
    ) -> HeddleDecision {
        let ratio = tokens_used as f64 / token_budget as f64;

        if ratio >= 0.95 {
            HeddleDecision::Halt
        } else if ratio >= 0.90 {
            let from = self.active_pattern;
            self.active_pattern = WeavePattern::Plain;
            if from != WeavePattern::Plain {
                HeddleDecision::Downgrade {
                    from,
                    to: WeavePattern::Plain,
                }
            } else {
                HeddleDecision::Maintain(self.active_pattern)
            }
        } else if ratio >= 0.80 {
            let from = self.active_pattern;
            if from == WeavePattern::Satin {
                self.active_pattern = WeavePattern::Twill;
                HeddleDecision::Downgrade {
                    from,
                    to: WeavePattern::Twill,
                }
            } else {
                HeddleDecision::Maintain(self.active_pattern)
            }
        } else {
            HeddleDecision::Maintain(self.active_pattern)
        }
    }

    /// Get the currently active weave pattern.
    pub fn active_pattern(&self) -> WeavePattern {
        self.active_pattern
    }

    /// Check if any recurring weft patterns should be promoted to warp threads.
    /// A weft pattern is promoted when it recurs across `promotion_threshold` tasks.
    pub fn check_promotions(
        &self,
        weft: &WeftStore,
        _warp: &mut WarpStore,
    ) -> anyhow::Result<Vec<ThreadId>> {
        let promoted = Vec::new();
        let archives = weft.archived_tasks();

        let mut pattern_counts: HashMap<String, u32> = HashMap::new();
        for task in archives {
            for (a, b) in &task.interlacement_pattern {
                let key = format!("{}:{}", a.0, b.0);
                *pattern_counts.entry(key).or_insert(0) += 1;
            }
        }

        for (pattern_key, count) in &pattern_counts {
            if *count >= self.promotion_threshold {
                tracing::info!(
                    pattern = %pattern_key,
                    count = %count,
                    "Weft pattern eligible for warp promotion (recurred {} times)",
                    count
                );
            }
        }

        Ok(promoted)
    }
}
