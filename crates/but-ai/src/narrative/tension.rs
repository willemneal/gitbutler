//! Dramatic tension detection and lifecycle management.
//!
//! Tensions are contradictions or unresolved issues in the narrative. They are
//! not errors -- they are information. Unresolved tensions create dramatic
//! pressure that biases the agent toward resolution.
//!
//! Tensions have their own lifecycle: active -> escalated -> resolved.
//! If a tension persists for more than the escalation threshold without
//! resolution, it is escalated and flagged in every relevant task.

use crate::types::{Tension, TensionEntry, TensionId, TensionSeverity, TensionState};

/// Configuration for tension lifecycle management.
#[derive(Debug, Clone)]
pub struct TensionConfig {
    /// Seconds before an active tension is escalated.
    pub escalation_threshold_seconds: u64,
}

impl Default for TensionConfig {
    fn default() -> Self {
        Self {
            escalation_threshold_seconds: 1_209_600, // 14 days
        }
    }
}

/// Manages the lifecycle of narrative tensions (contradictions and unresolved issues).
pub struct TensionRegistry {
    tensions: Vec<Tension>,
    config: TensionConfig,
}

impl TensionRegistry {
    /// Create a new, empty tension registry.
    pub fn new(config: TensionConfig) -> Self {
        Self {
            tensions: Vec::new(),
            config,
        }
    }

    /// Register a new tension introduced by a chapter.
    pub fn introduce(&mut self, entry: &TensionEntry, chapter_number: u64, timestamp: &str) {
        // Don't introduce duplicates.
        if self.tensions.iter().any(|t| t.id == entry.id) {
            return;
        }

        self.tensions.push(Tension {
            id: entry.id.clone(),
            description: entry.description.clone(),
            severity: entry.severity,
            state: TensionState::Active,
            introduced_in: chapter_number,
            resolved_in: None,
            created_at: timestamp.to_string(),
            escalated_at: None,
            suggested_resolution: entry.suggested_resolution.clone(),
        });
    }

    /// Mark a tension as resolved by a chapter.
    pub fn resolve(&mut self, tension_id: &TensionId, chapter_number: u64) -> anyhow::Result<()> {
        let tension = self
            .tensions
            .iter_mut()
            .find(|t| t.id == *tension_id)
            .ok_or_else(|| anyhow::anyhow!("Tension not found: {:?}", tension_id))?;

        tension.state = TensionState::Resolved;
        tension.resolved_in = Some(chapter_number);
        Ok(())
    }

    /// Escalate tensions that have been active longer than the threshold.
    ///
    /// The `current_timestamp_seconds` is a Unix timestamp used to compare
    /// against tension creation time. Returns the IDs of newly escalated tensions.
    pub fn escalate_overdue(&mut self, current_timestamp_seconds: u64) -> Vec<TensionId> {
        let threshold = self.config.escalation_threshold_seconds;
        let mut escalated = Vec::new();

        for tension in &mut self.tensions {
            if tension.state != TensionState::Active {
                continue;
            }

            // Parse created_at as seconds (simplified: we use numeric timestamps).
            if let Ok(created) = tension.created_at.parse::<u64>() {
                if current_timestamp_seconds.saturating_sub(created) >= threshold {
                    tension.state = TensionState::Escalated;
                    tension.severity = TensionSeverity::Critical;
                    tension.escalated_at = Some(current_timestamp_seconds.to_string());
                    escalated.push(tension.id.clone());
                }
            }
        }

        escalated
    }

    /// Get all active (non-resolved) tensions.
    pub fn active_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .iter()
            .filter(|t| t.state == TensionState::Active || t.state == TensionState::Escalated)
            .collect()
    }

    /// Get all escalated tensions (subset of active).
    pub fn escalated_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .iter()
            .filter(|t| t.state == TensionState::Escalated)
            .collect()
    }

    /// Get all resolved tensions.
    pub fn resolved_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .iter()
            .filter(|t| t.state == TensionState::Resolved)
            .collect()
    }

    /// Get a tension by its ID.
    pub fn get_tension(&self, id: &TensionId) -> Option<&Tension> {
        self.tensions.iter().find(|t| t.id == *id)
    }

    /// Return the count of active (including escalated) tensions.
    pub fn active_count(&self) -> u32 {
        self.active_tensions().len() as u32
    }

    /// Compute a tension urgency score for a set of tension IDs.
    ///
    /// Higher urgency for escalated tensions and higher severity.
    pub fn urgency_score(&self, tension_ids: &[TensionId]) -> f64 {
        if tension_ids.is_empty() {
            return 0.0;
        }

        let total: f64 = tension_ids
            .iter()
            .filter_map(|id| self.get_tension(id))
            .map(|t| {
                let base: f64 = match t.severity {
                    TensionSeverity::Low => 0.25,
                    TensionSeverity::Moderate => 0.50,
                    TensionSeverity::High => 0.75,
                    TensionSeverity::Critical => 1.00,
                };
                let escalation_bonus: f64 = if t.state == TensionState::Escalated {
                    0.25
                } else {
                    0.0
                };
                (base + escalation_bonus).min(1.0)
            })
            .sum();

        (total / tension_ids.len() as f64).min(1.0)
    }

    /// Return all tension IDs associated with a given arc, based on the
    /// chapters they were introduced in.
    pub fn tensions_for_chapters(&self, chapter_numbers: &[u64]) -> Vec<&Tension> {
        self.tensions
            .iter()
            .filter(|t| chapter_numbers.contains(&t.introduced_in))
            .collect()
    }

    /// Return the escalation threshold in seconds.
    pub fn escalation_threshold(&self) -> u64 {
        self.config.escalation_threshold_seconds
    }
}
