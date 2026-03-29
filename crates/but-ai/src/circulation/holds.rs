//! Task dependency queue: holds, requests, and interlibrary loans.
//!
//! In a library, a hold is a request for an item that is currently
//! checked out. The holds queue tells the library which items are
//! in highest demand. Circ maintains an analogous queue for agent
//! tasks: which tasks are waiting for other tasks to complete, which
//! cross-repo dependencies are blocking progress.

use crate::types::{AgentId, CollectionRef, Hold, HoldUrgency, ItemId};

/// The holds queue: manages task dependencies and pending requests.
#[derive(Debug, Clone, Default)]
pub struct HoldsQueue {
    /// All holds, in queue order.
    holds: Vec<Hold>,
    /// Counter for generating hold IDs.
    next_id: u64,
}

impl HoldsQueue {
    /// Create a new empty holds queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Place a hold on an item. Returns the hold ID.
    pub fn place_hold(
        &mut self,
        item_id: ItemId,
        collection: CollectionRef,
        requesting_agent: AgentId,
        urgency: HoldUrgency,
        now: &str,
        due_date: Option<String>,
    ) -> String {
        self.next_id += 1;
        let hold_id = format!("hold_{:04}", self.next_id);
        let queue_position = self.pending_count() as u32 + 1;

        self.holds.push(Hold {
            hold_id: hold_id.clone(),
            item_id,
            collection,
            requesting_agent,
            urgency,
            queue_position,
            placed_at: now.to_string(),
            due_date,
            satisfied: false,
        });

        // Re-sort by urgency: rush first, then interlibrary loan, then normal.
        self.holds.sort_by_key(|h| match h.urgency {
            HoldUrgency::Rush => 0,
            HoldUrgency::InterlibraryLoan => 1,
            HoldUrgency::Normal => 2,
        });

        // Update queue positions after sorting.
        let mut pos = 1;
        for hold in &mut self.holds {
            if !hold.satisfied {
                hold.queue_position = pos;
                pos += 1;
            }
        }

        hold_id
    }

    /// Satisfy a hold, marking it as resolved.
    pub fn satisfy(&mut self, hold_id: &str) -> bool {
        if let Some(hold) = self.holds.iter_mut().find(|h| h.hold_id == hold_id) {
            hold.satisfied = true;
            true
        } else {
            false
        }
    }

    /// Satisfy all holds on a specific item.
    pub fn satisfy_item(&mut self, item_id: &ItemId) -> usize {
        let mut count = 0;
        for hold in &mut self.holds {
            if hold.item_id == *item_id && !hold.satisfied {
                hold.satisfied = true;
                count += 1;
            }
        }
        count
    }

    /// Get all pending (unsatisfied) holds.
    pub fn pending(&self) -> Vec<&Hold> {
        self.holds.iter().filter(|h| !h.satisfied).collect()
    }

    /// Number of pending holds.
    pub fn pending_count(&self) -> usize {
        self.holds.iter().filter(|h| !h.satisfied).count()
    }

    /// Get all holds for a specific item.
    pub fn holds_for_item(&self, item_id: &ItemId) -> Vec<&Hold> {
        self.holds
            .iter()
            .filter(|h| h.item_id == *item_id)
            .collect()
    }

    /// Get all interlibrary loan holds (cross-repo dependencies).
    pub fn interlibrary_loans(&self) -> Vec<&Hold> {
        self.holds
            .iter()
            .filter(|h| h.urgency == HoldUrgency::InterlibraryLoan && !h.satisfied)
            .collect()
    }

    /// Validate all pending holds: check if any have passed their due date.
    /// Returns the IDs of overdue holds.
    pub fn find_overdue(&self, now: &str) -> Vec<String> {
        self.holds
            .iter()
            .filter(|h| !h.satisfied)
            .filter(|h| {
                h.due_date
                    .as_ref()
                    .map(|d| d.as_str() < now)
                    .unwrap_or(false)
            })
            .map(|h| h.hold_id.clone())
            .collect()
    }

    /// Remove all satisfied holds from the queue (cleanup).
    pub fn purge_satisfied(&mut self) -> usize {
        let before = self.holds.len();
        self.holds.retain(|h| !h.satisfied);
        before - self.holds.len()
    }

    /// Get a hold by ID.
    pub fn get(&self, hold_id: &str) -> Option<&Hold> {
        self.holds.iter().find(|h| h.hold_id == hold_id)
    }

    /// Total number of holds (including satisfied).
    pub fn total_count(&self) -> usize {
        self.holds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hold(queue: &mut HoldsQueue, item: &str, urgency: HoldUrgency) -> String {
        queue.place_hold(
            ItemId(item.into()),
            CollectionRef {
                uri: "github.com/org/repo".into(),
            },
            AgentId("shelver".into()),
            urgency,
            "2026-03-28T14:00:00Z",
            None,
        )
    }

    #[test]
    fn place_and_satisfy() {
        let mut queue = HoldsQueue::new();
        let id = make_hold(&mut queue, "mem_001", HoldUrgency::Normal);

        assert_eq!(queue.pending_count(), 1);
        assert!(queue.satisfy(&id));
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn urgency_ordering() {
        let mut queue = HoldsQueue::new();
        let _normal = make_hold(&mut queue, "item_a", HoldUrgency::Normal);
        let rush = make_hold(&mut queue, "item_b", HoldUrgency::Rush);

        let pending = queue.pending();
        // Rush should be first in queue.
        assert_eq!(pending[0].hold_id, rush);
    }

    #[test]
    fn satisfy_item_batch() {
        let mut queue = HoldsQueue::new();
        make_hold(&mut queue, "mem_001", HoldUrgency::Normal);
        make_hold(&mut queue, "mem_001", HoldUrgency::Rush);
        make_hold(&mut queue, "mem_002", HoldUrgency::Normal);

        let count = queue.satisfy_item(&ItemId("mem_001".into()));
        assert_eq!(count, 2);
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn overdue_detection() {
        let mut queue = HoldsQueue::new();
        queue.place_hold(
            ItemId("mem_001".into()),
            CollectionRef {
                uri: "github.com/org/repo".into(),
            },
            AgentId("shelver".into()),
            HoldUrgency::Normal,
            "2026-03-01T00:00:00Z",
            Some("2026-03-15T00:00:00Z".into()),
        );

        let overdue = queue.find_overdue("2026-03-28T00:00:00Z");
        assert_eq!(overdue.len(), 1);
    }

    #[test]
    fn purge_satisfied() {
        let mut queue = HoldsQueue::new();
        let id = make_hold(&mut queue, "mem_001", HoldUrgency::Normal);
        make_hold(&mut queue, "mem_002", HoldUrgency::Normal);

        queue.satisfy(&id);
        let purged = queue.purge_satisfied();
        assert_eq!(purged, 1);
        assert_eq!(queue.total_count(), 1);
    }
}
