//! Tension lifecycle management.
//!
//! Tensions are contradictions or unresolved issues tracked across memory
//! entries. They follow a lifecycle: introduced -> referenced -> escalated
//! (after 14 days unresolved) -> resolved.
//!
//! Urgency scoring uses a Weibull-style function (k=2.0, lambda=14 days)
//! so that urgency increases over time until resolution.

use std::collections::HashMap;

use crate::types::{EntryId, MemoryEntry, Tension, TensionId, TensionRole, TensionSeverity};

/// Weibull shape parameter for urgency scoring.
const URGENCY_K: f64 = 2.0;

/// Weibull scale parameter for urgency scoring (14 days in seconds).
const URGENCY_LAMBDA_SECS: f64 = 14.0 * 24.0 * 3600.0;

/// Escalation threshold: 14 days in seconds.
const ESCALATION_THRESHOLD_SECS: u64 = 14 * 24 * 3600;

/// Registry tracking all known tensions and their lifecycle state.
pub struct TensionRegistry {
    tensions: HashMap<TensionId, TrackedTension>,
}

/// Internal representation of a tension with lifecycle timestamps.
#[derive(Debug, Clone)]
struct TrackedTension {
    tension: Tension,
    /// Timestamps (as seconds) when this tension was referenced by entries.
    references: Vec<(EntryId, u64)>,
    /// Timestamp (seconds) when first introduced.
    introduced_at: u64,
    /// Whether this tension has been escalated.
    escalated: bool,
}

impl TensionRegistry {
    /// Create an empty tension registry.
    pub fn new() -> Self {
        Self {
            tensions: HashMap::new(),
        }
    }

    /// Introduce a new tension, originating from a memory entry.
    ///
    /// If a tension with the same ID already exists, this is a no-op.
    pub fn introduce(&mut self, tension: Tension, timestamp_secs: u64) {
        if self.tensions.contains_key(&tension.id) {
            return;
        }
        let id = tension.id.clone();
        self.tensions.insert(
            id,
            TrackedTension {
                tension,
                references: Vec::new(),
                introduced_at: timestamp_secs,
                escalated: false,
            },
        );
    }

    /// Record that an entry references an existing tension.
    pub fn reference(&mut self, tension_id: &TensionId, entry_id: &EntryId, timestamp_secs: u64) {
        if let Some(tracked) = self.tensions.get_mut(tension_id) {
            tracked
                .references
                .push((entry_id.clone(), timestamp_secs));
        }
    }

    /// Resolve a tension, recording which entry resolved it.
    ///
    /// Returns `true` if the tension existed and was resolved.
    pub fn resolve(&mut self, tension_id: &TensionId, resolved_by: &EntryId) -> bool {
        if let Some(tracked) = self.tensions.get_mut(tension_id) {
            tracked.tension.resolved_in = Some(resolved_by.clone());
            true
        } else {
            false
        }
    }

    /// Escalate all tensions that have been unresolved for longer than
    /// `ESCALATION_THRESHOLD_SECS`. Returns the IDs of newly escalated tensions.
    pub fn escalate_overdue(&mut self, current_timestamp_secs: u64) -> Vec<TensionId> {
        let mut escalated = Vec::new();

        for tracked in self.tensions.values_mut() {
            if tracked.tension.resolved_in.is_some() || tracked.escalated {
                continue;
            }
            let age = current_timestamp_secs.saturating_sub(tracked.introduced_at);
            if age >= ESCALATION_THRESHOLD_SECS {
                tracked.escalated = true;
                tracked.tension.severity = TensionSeverity::Critical;
                escalated.push(tracked.tension.id.clone());
            }
        }

        escalated
    }

    /// Compute the urgency score for a tension using a Weibull CDF.
    ///
    /// `urgency(t) = 1 - exp(-(t/lambda)^k)` where t is the age in seconds.
    /// Returns 0.0 for resolved tensions. The score is in [0, 1].
    pub fn urgency_score(&self, tension_id: &TensionId, current_timestamp_secs: u64) -> f64 {
        let tracked = match self.tensions.get(tension_id) {
            Some(t) => t,
            None => return 0.0,
        };

        // Resolved tensions have zero urgency.
        if tracked.tension.resolved_in.is_some() {
            return 0.0;
        }

        let age_secs =
            current_timestamp_secs.saturating_sub(tracked.introduced_at) as f64;

        // Weibull CDF: F(t) = 1 - exp(-(t/lambda)^k)
        let ratio = age_secs / URGENCY_LAMBDA_SECS;
        let urgency = 1.0 - (-ratio.powf(URGENCY_K)).exp();

        // Severity multiplier.
        let severity_mult = match tracked.tension.severity {
            TensionSeverity::Low => 0.5,
            TensionSeverity::Moderate => 0.75,
            TensionSeverity::High => 1.0,
            TensionSeverity::Critical => 1.0,
        };

        // Escalation bonus.
        let escalation_bonus = if tracked.escalated { 0.15 } else { 0.0 };

        (urgency * severity_mult + escalation_bonus).clamp(0.0, 1.0)
    }

    /// Get a tension by ID.
    pub fn get(&self, id: &TensionId) -> Option<&Tension> {
        self.tensions.get(id).map(|t| &t.tension)
    }

    /// Return all unresolved (active) tensions.
    pub fn active_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .values()
            .filter(|t| t.tension.resolved_in.is_none())
            .map(|t| &t.tension)
            .collect()
    }

    /// Return all escalated tensions.
    pub fn escalated_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .values()
            .filter(|t| t.escalated)
            .map(|t| &t.tension)
            .collect()
    }

    /// Return all resolved tensions.
    pub fn resolved_tensions(&self) -> Vec<&Tension> {
        self.tensions
            .values()
            .filter(|t| t.tension.resolved_in.is_some())
            .map(|t| &t.tension)
            .collect()
    }

    /// Return the total number of tracked tensions.
    pub fn count(&self) -> usize {
        self.tensions.len()
    }

    /// Compute the aggregate urgency for a memory entry based on its
    /// tension references.
    pub fn entry_urgency(&self, entry: &MemoryEntry, current_timestamp_secs: u64) -> f64 {
        if entry.tension_refs.is_empty() {
            return 0.0;
        }

        let mut total = 0.0_f64;
        let mut count = 0_u32;

        for tref in &entry.tension_refs {
            let u = self.urgency_score(&tref.tension_id, current_timestamp_secs);
            // Introduced/unresolved tensions have higher weight than mere references.
            let weight = match tref.role {
                TensionRole::Introduced => 1.0,
                TensionRole::Referenced => 0.6,
                TensionRole::Resolved => 0.0, // Resolved tensions don't add urgency.
            };
            total += u * weight;
            count += 1;
        }

        if count == 0 {
            return 0.0;
        }

        (total / count as f64).clamp(0.0, 1.0)
    }
}

impl Default for TensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
