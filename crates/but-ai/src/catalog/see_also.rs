//! Bidirectional "see also" cross-reference graph.
//!
//! The "see also" graph is the most powerful retrieval mechanism in the
//! card catalog. When an agent searches for "authentication", the graph
//! traversal catches memories about session management, token rotation,
//! and API v1 comparisons — memories that keyword search alone would miss.
//!
//! Every link is bidirectional: if A "see also" B, then B "see also" A.
//! Relationship types may differ in each direction (A depends_on B implies
//! B is depended_on_by A, though we store the canonical direction only).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::types::{ItemId, Relationship, SeeAlsoLink};

/// The "see also" cross-reference graph.
///
/// Internally stored as an adjacency list. Each item maps to its outgoing
/// links. Bidirectionality is maintained by adding edges in both directions.
#[derive(Debug, Clone, Default)]
pub struct SeeAlsoGraph {
    /// Adjacency list: item -> outgoing links.
    edges: HashMap<ItemId, Vec<SeeAlsoLink>>,
    /// Maximum links per item.
    max_links: usize,
}

impl SeeAlsoGraph {
    /// Create a new graph with the given maximum links per item.
    pub fn new(max_links: usize) -> Self {
        Self {
            edges: HashMap::new(),
            max_links,
        }
    }

    /// Add a bidirectional "see also" link between two items.
    ///
    /// Returns `false` if either item has reached its link limit.
    pub fn add_link(
        &mut self,
        from: ItemId,
        to: ItemId,
        relationship: Relationship,
        note: String,
    ) -> bool {
        // Check limits before adding.
        let from_links = self.edges.entry(from.clone()).or_default();
        if from_links.len() >= self.max_links {
            tracing::warn!(
                item = %from,
                max = self.max_links,
                "item has reached maximum see-also links"
            );
            return false;
        }

        // Don't add duplicate links.
        if from_links.iter().any(|l| l.target == to) {
            return true; // Already linked, treat as success.
        }

        let to_links = self.edges.entry(to.clone()).or_default();
        if to_links.len() >= self.max_links {
            tracing::warn!(
                item = %to,
                max = self.max_links,
                "target item has reached maximum see-also links"
            );
            return false;
        }

        // Add forward link.
        let from_links = self.edges.get_mut(&from).unwrap();
        from_links.push(SeeAlsoLink {
            target: to.clone(),
            relationship,
            note: note.clone(),
        });

        // Add reverse link with the inverse relationship.
        let reverse_rel = invert_relationship(relationship);
        let to_links = self.edges.get_mut(&to).unwrap();
        to_links.push(SeeAlsoLink {
            target: from,
            relationship: reverse_rel,
            note,
        });

        true
    }

    /// Remove all links involving a given item (used during deaccession).
    pub fn remove_item(&mut self, item: &ItemId) {
        // Remove the item's own edges.
        if let Some(links) = self.edges.remove(item) {
            // Remove reverse edges from targets.
            for link in links {
                if let Some(target_links) = self.edges.get_mut(&link.target) {
                    target_links.retain(|l| l.target != *item);
                }
            }
        }
    }

    /// Get all direct links from an item.
    pub fn get_links(&self, item: &ItemId) -> &[SeeAlsoLink] {
        self.edges.get(item).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Breadth-first traversal from a starting item, up to `max_hops` hops.
    ///
    /// Returns a list of `(item_id, hop_count)` pairs, excluding the start.
    pub fn traverse(&self, start: &ItemId, max_hops: u32) -> Vec<(ItemId, u32)> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut results = Vec::new();

        visited.insert(start.clone());
        queue.push_back((start.clone(), 0u32));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_hops {
                continue;
            }

            if let Some(links) = self.edges.get(&current) {
                for link in links {
                    if visited.insert(link.target.clone()) {
                        let hop = depth + 1;
                        results.push((link.target.clone(), hop));
                        queue.push_back((link.target.clone(), hop));
                    }
                }
            }
        }

        results
    }

    /// Find the shortest path between two items. Returns `None` if no
    /// path exists, otherwise returns the hop count.
    pub fn shortest_path(&self, from: &ItemId, to: &ItemId) -> Option<u32> {
        if from == to {
            return Some(0);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        visited.insert(from.clone());
        queue.push_back((from.clone(), 0u32));

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(links) = self.edges.get(&current) {
                for link in links {
                    if link.target == *to {
                        return Some(depth + 1);
                    }
                    if visited.insert(link.target.clone()) {
                        queue.push_back((link.target.clone(), depth + 1));
                    }
                }
            }
        }

        None
    }

    /// Number of items in the graph.
    pub fn item_count(&self) -> usize {
        self.edges.len()
    }

    /// Total number of directed edges.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Validate the graph: check that all bidirectional invariants hold
    /// and that no links point to items not in the graph.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for (item, links) in &self.edges {
            for link in links {
                // Check that the target exists in the graph.
                if !self.edges.contains_key(&link.target) {
                    errors.push(format!(
                        "link from {} to {} points to non-existent item",
                        item, link.target
                    ));
                    continue;
                }

                // Check that a reverse link exists.
                let target_links = &self.edges[&link.target];
                if !target_links.iter().any(|l| l.target == *item) {
                    errors.push(format!(
                        "link from {} to {} has no reverse link",
                        item, link.target
                    ));
                }
            }

            // Check link count limit.
            if links.len() > self.max_links {
                errors.push(format!(
                    "item {} has {} links, exceeding max {}",
                    item,
                    links.len(),
                    self.max_links
                ));
            }
        }

        errors
    }

    /// Export all edges for serialization.
    pub fn all_edges(&self) -> &HashMap<ItemId, Vec<SeeAlsoLink>> {
        &self.edges
    }

    /// Import edges from a previously serialized graph.
    pub fn import_edges(&mut self, edges: HashMap<ItemId, Vec<SeeAlsoLink>>) {
        self.edges = edges;
    }
}

/// Invert a relationship for the reverse direction.
fn invert_relationship(rel: Relationship) -> Relationship {
    match rel {
        Relationship::RelatedTo => Relationship::RelatedTo,
        Relationship::DependsOn => Relationship::DependsOn, // stored canonically
        Relationship::ContrastsWith => Relationship::ContrastsWith,
        Relationship::Supersedes => Relationship::Supersedes, // stored canonically
        Relationship::NarrowerTerm => Relationship::BroaderTerm,
        Relationship::BroaderTerm => Relationship::NarrowerTerm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidirectional_linking() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = ItemId("mem_001".into());
        let b = ItemId("mem_002".into());

        assert!(graph.add_link(
            a.clone(),
            b.clone(),
            Relationship::RelatedTo,
            "related work".into()
        ));

        // Forward link exists.
        assert_eq!(graph.get_links(&a).len(), 1);
        assert_eq!(graph.get_links(&a)[0].target, b);

        // Reverse link exists.
        assert_eq!(graph.get_links(&b).len(), 1);
        assert_eq!(graph.get_links(&b)[0].target, a);
    }

    #[test]
    fn traversal_with_max_hops() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = ItemId("a".into());
        let b = ItemId("b".into());
        let c = ItemId("c".into());
        let d = ItemId("d".into());

        // a -> b -> c -> d
        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(c.clone(), d.clone(), Relationship::RelatedTo, "".into());

        // 1 hop from a: only b
        let one_hop = graph.traverse(&a, 1);
        assert_eq!(one_hop.len(), 1);
        assert_eq!(one_hop[0].0, b);

        // 2 hops from a: b and c
        let two_hops = graph.traverse(&a, 2);
        assert_eq!(two_hops.len(), 2);
    }

    #[test]
    fn shortest_path() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = ItemId("a".into());
        let b = ItemId("b".into());
        let c = ItemId("c".into());

        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());

        assert_eq!(graph.shortest_path(&a, &c), Some(2));
        assert_eq!(graph.shortest_path(&a, &a), Some(0));
        assert_eq!(
            graph.shortest_path(&a, &ItemId("nonexistent".into())),
            None
        );
    }

    #[test]
    fn link_limit_enforced() {
        let mut graph = SeeAlsoGraph::new(2);
        let a = ItemId("a".into());
        let b = ItemId("b".into());
        let c = ItemId("c".into());
        let d = ItemId("d".into());

        assert!(graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into()));
        assert!(graph.add_link(a.clone(), c.clone(), Relationship::RelatedTo, "".into()));
        // a now has 2 links (the max), so this should fail.
        assert!(!graph.add_link(
            a.clone(),
            d.clone(),
            Relationship::RelatedTo,
            "".into()
        ));
    }

    #[test]
    fn remove_item_cleans_reverse_links() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = ItemId("a".into());
        let b = ItemId("b".into());
        let c = ItemId("c".into());

        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());

        graph.remove_item(&b);

        // a should have no links (its link to b was removed).
        assert_eq!(graph.get_links(&a).len(), 0);
        // c should have no links (its link to b was removed).
        assert_eq!(graph.get_links(&c).len(), 0);
    }

    #[test]
    fn narrower_broader_inversion() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = ItemId("auth".into());
        let b = ItemId("jwt".into());

        graph.add_link(
            a.clone(),
            b.clone(),
            Relationship::BroaderTerm,
            "auth is broader than jwt".into(),
        );

        // Forward: auth -> jwt is BroaderTerm
        assert_eq!(
            graph.get_links(&a)[0].relationship,
            Relationship::BroaderTerm
        );
        // Reverse: jwt -> auth is NarrowerTerm
        assert_eq!(
            graph.get_links(&b)[0].relationship,
            Relationship::NarrowerTerm
        );
    }
}
