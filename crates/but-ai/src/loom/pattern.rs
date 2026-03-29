use std::collections::HashSet;

use crate::types::{Thread, WeavePattern};

/// Configuration for pattern selection.
#[derive(Debug, Clone)]
pub struct PatternConfig {
    pub embedding_weight: f64,
    pub tension_weight: f64,
    pub recency_weight: f64,
    pub pattern_weight: f64,
    pub color_weight: f64,
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            embedding_weight: 0.35,
            tension_weight: 0.25,
            recency_weight: 0.20,
            pattern_weight: 0.10,
            color_weight: 0.10,
        }
    }
}

/// Relevance score for a thread against a query.
#[derive(Debug, Clone)]
pub struct RelevanceScore {
    pub embedding_similarity: f64,
    pub tension: f64,
    pub recency: f64,
    pub pattern_match: f64,
    pub color_match: f64,
    config: PatternConfig,
}

impl RelevanceScore {
    /// Compute a relevance score for a thread against a query text and pattern.
    pub fn compute(thread: &Thread, query: &str, pattern: WeavePattern) -> Self {
        let embedding_similarity = Self::text_similarity(&thread.content, query);
        let tension = thread.tension;
        let recency = Self::recency_score(&thread.last_interlaced);
        let pattern_match = Self::pattern_position_score(thread.position, pattern);
        let color_match = 0.5; // Neutral default; refined when task type is known.

        Self {
            embedding_similarity,
            tension,
            recency,
            pattern_match,
            color_match,
            config: PatternConfig::default(),
        }
    }

    /// Weighted total relevance score.
    pub fn total(&self) -> f64 {
        self.config.embedding_weight * self.embedding_similarity
            + self.config.tension_weight * self.tension
            + self.config.recency_weight * self.recency
            + self.config.pattern_weight * self.pattern_match
            + self.config.color_weight * self.color_match
    }

    /// Simple text similarity (token overlap ratio). Placeholder for
    /// embedding-based similarity via but-llm.
    fn text_similarity(content: &str, query: &str) -> f64 {
        let content_words: HashSet<&str> = content.split_whitespace().collect();
        let query_words: HashSet<&str> = query.split_whitespace().collect();
        if query_words.is_empty() {
            return 0.0;
        }
        let intersection = content_words.intersection(&query_words).count();
        intersection as f64 / query_words.len() as f64
    }

    /// Score based on recency of last interlacement (placeholder).
    fn recency_score(_last_interlaced: &str) -> f64 {
        0.5 // Neutral default until timestamp parsing is implemented.
    }

    /// Score based on thread position within the active weave pattern.
    fn pattern_position_score(position: u32, pattern: WeavePattern) -> f64 {
        match pattern {
            WeavePattern::Plain => 1.0,
            WeavePattern::Twill => {
                if position % 2 == 0 {
                    1.0
                } else {
                    0.3
                }
            }
            WeavePattern::Satin => {
                if position % 5 == 0 {
                    1.0
                } else {
                    0.1
                }
            }
        }
    }
}

/// Selects the appropriate weave pattern based on task familiarity.
pub struct PatternSelector;

impl PatternSelector {
    /// Determine the weave pattern for a task based on warp coverage.
    /// If few warp threads match the task query, it is unfamiliar -> plain weave.
    /// If many match, it is familiar -> twill. If nearly all match -> satin.
    pub fn select(warp_match_ratio: f64, _default: WeavePattern) -> WeavePattern {
        if warp_match_ratio < 0.2 {
            WeavePattern::Plain
        } else if warp_match_ratio < 0.6 {
            WeavePattern::Twill
        } else {
            WeavePattern::Satin
        }
    }
}
