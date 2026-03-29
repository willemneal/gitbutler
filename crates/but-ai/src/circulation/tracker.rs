//! Memory access and circulation tracking.
//!
//! In library science, circulation statistics — which books are checked
//! out, how often, by whom — are the primary signal for collection
//! development. Memories that circulate frequently are valuable.
//! Memories that never circulate should be deaccessioned.
//!
//! The tracker maintains an append-only log of circulation events.
//! Counts are derived from the log, not from a mutable counter,
//! making them auditable and correctable.

use crate::types::{AgentId, CirculationEvent, CirculationEventType, CirculationRecord, ItemId};
use std::collections::HashMap;

/// The circulation tracker: an append-only log of memory access events.
#[derive(Debug, Clone, Default)]
pub struct CirculationTracker {
    /// The event log (append-only).
    log: Vec<CirculationEvent>,
    /// Cached per-item statistics, recomputed from the log.
    stats_cache: HashMap<ItemId, CirculationStats>,
    /// Whether the cache needs rebuilding.
    cache_dirty: bool,
}

/// Per-item circulation statistics derived from the event log.
#[derive(Debug, Clone, Default)]
pub struct CirculationStats {
    /// Number of checkout events.
    pub checkouts: u64,
    /// Number of check-in events.
    pub checkins: u64,
    /// Unique contexts in which the item was accessed.
    pub contexts: Vec<String>,
    /// Unique agents that accessed the item.
    pub agents: Vec<AgentId>,
    /// Timestamp of the most recent event.
    pub last_event: Option<String>,
}

impl CirculationTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a checkout event (memory was read/retrieved).
    pub fn record_checkout(
        &mut self,
        item_id: ItemId,
        agent: AgentId,
        context: String,
        timestamp: String,
    ) {
        self.log.push(CirculationEvent {
            item_id,
            agent,
            timestamp,
            context,
            event_type: CirculationEventType::Checkout,
        });
        self.cache_dirty = true;
    }

    /// Record a check-in event (memory was updated/returned).
    pub fn record_checkin(
        &mut self,
        item_id: ItemId,
        agent: AgentId,
        context: String,
        timestamp: String,
    ) {
        self.log.push(CirculationEvent {
            item_id,
            agent,
            timestamp,
            context,
            event_type: CirculationEventType::Checkin,
        });
        self.cache_dirty = true;
    }

    /// Get circulation statistics for a specific item.
    pub fn stats_for(&mut self, item_id: &ItemId) -> &CirculationStats {
        if self.cache_dirty {
            self.rebuild_cache();
        }
        self.stats_cache
            .get(item_id)
            .unwrap_or(&DEFAULT_STATS)
    }

    /// Get the total number of events in the log.
    pub fn event_count(&self) -> usize {
        self.log.len()
    }

    /// Get all events for a specific item.
    pub fn events_for(&self, item_id: &ItemId) -> Vec<&CirculationEvent> {
        self.log.iter().filter(|e| e.item_id == *item_id).collect()
    }

    /// Convert tracker stats into a [`CirculationRecord`] for a catalog entry.
    pub fn to_record(&mut self, item_id: &ItemId) -> CirculationRecord {
        let stats = self.stats_for(item_id);
        CirculationRecord {
            total_checkouts: stats.checkouts,
            last_checkout: stats.last_event.clone(),
            checkout_contexts: stats.contexts.clone(),
        }
    }

    /// Find the most-circulated items, sorted by checkout count descending.
    pub fn most_circulated(&mut self, limit: usize) -> Vec<(ItemId, u64)> {
        if self.cache_dirty {
            self.rebuild_cache();
        }

        let mut items: Vec<(ItemId, u64)> = self
            .stats_cache
            .iter()
            .map(|(id, stats)| (id.clone(), stats.checkouts))
            .collect();

        items.sort_by(|a, b| b.1.cmp(&a.1));
        items.truncate(limit);
        items
    }

    /// Find items that have never been checked out.
    pub fn uncirculated_items(&mut self) -> Vec<ItemId> {
        if self.cache_dirty {
            self.rebuild_cache();
        }

        self.stats_cache
            .iter()
            .filter(|(_, stats)| stats.checkouts == 0)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get the raw event log.
    pub fn log(&self) -> &[CirculationEvent] {
        &self.log
    }

    /// Rebuild the statistics cache from the event log.
    fn rebuild_cache(&mut self) {
        self.stats_cache.clear();

        for event in &self.log {
            let stats = self
                .stats_cache
                .entry(event.item_id.clone())
                .or_default();

            match event.event_type {
                CirculationEventType::Checkout => stats.checkouts += 1,
                CirculationEventType::Checkin => stats.checkins += 1,
            }

            if !stats.contexts.contains(&event.context) {
                stats.contexts.push(event.context.clone());
            }

            if !stats.agents.contains(&event.agent) {
                stats.agents.push(event.agent.clone());
            }

            stats.last_event = Some(event.timestamp.clone());
        }

        self.cache_dirty = false;
    }
}

/// Static empty stats used as a default return value.
static DEFAULT_STATS: CirculationStats = CirculationStats {
    checkouts: 0,
    checkins: 0,
    contexts: Vec::new(),
    agents: Vec::new(),
    last_event: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_tracking() {
        let mut tracker = CirculationTracker::new();
        let item = ItemId("mem_001".into());
        let agent = AgentId("shelver".into());

        tracker.record_checkout(
            item.clone(),
            agent.clone(),
            "auth-refactor".into(),
            "2026-03-28T14:00:00Z".into(),
        );
        tracker.record_checkout(
            item.clone(),
            agent.clone(),
            "security-audit".into(),
            "2026-03-28T15:00:00Z".into(),
        );

        let stats = tracker.stats_for(&item);
        assert_eq!(stats.checkouts, 2);
        assert_eq!(stats.contexts.len(), 2);
    }

    #[test]
    fn most_circulated_ranking() {
        let mut tracker = CirculationTracker::new();
        let agent = AgentId("shelver".into());

        // Item A: 3 checkouts.
        for i in 0..3 {
            tracker.record_checkout(
                ItemId("a".into()),
                agent.clone(),
                format!("ctx_{}", i),
                "2026-03-28T14:00:00Z".into(),
            );
        }
        // Item B: 1 checkout.
        tracker.record_checkout(
            ItemId("b".into()),
            agent.clone(),
            "ctx_0".into(),
            "2026-03-28T14:00:00Z".into(),
        );

        let top = tracker.most_circulated(10);
        assert_eq!(top[0].0, ItemId("a".into()));
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn to_record_conversion() {
        let mut tracker = CirculationTracker::new();
        let item = ItemId("mem_001".into());

        tracker.record_checkout(
            item.clone(),
            AgentId("shelver".into()),
            "task-1".into(),
            "2026-03-28T14:00:00Z".into(),
        );

        let record = tracker.to_record(&item);
        assert_eq!(record.total_checkouts, 1);
        assert!(record.last_checkout.is_some());
    }
}
