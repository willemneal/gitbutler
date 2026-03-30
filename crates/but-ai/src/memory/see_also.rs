//! Bidirectional "see also" cross-reference graph.
//!
//! The "see also" graph is a powerful retrieval mechanism. When an agent
//! searches for "authentication", graph traversal catches memories about
//! session management, token rotation, and API comparisons -- memories
//! that keyword search alone would miss.
//!
//! Every link is bidirectional: if A "see also" B, then B "see also" A.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::types::{EntryId, Relationship, SeeAlsoLink};

/// The "see also" cross-reference graph.
///
/// Internally stored as an adjacency list. Each entry maps to its outgoing
/// links. Bidirectionality is maintained by adding edges in both directions.
#[derive(Debug, Clone, Default)]
pub struct SeeAlsoGraph {
    edges: HashMap<EntryId, Vec<SeeAlsoLink>>,
    max_links: usize,
}

impl SeeAlsoGraph {
    /// Create a new graph with the given maximum links per entry.
    pub fn new(max_links: usize) -> Self {
        Self {
            edges: HashMap::new(),
            max_links,
        }
    }

    /// Add a bidirectional "see also" link between two entries.
    ///
    /// Returns `false` if either entry has reached its link limit.
    pub fn add_link(
        &mut self,
        from: EntryId,
        to: EntryId,
        relationship: Relationship,
        note: String,
    ) -> bool {
        let from_links = self.edges.entry(from.clone()).or_default();
        if from_links.len() >= self.max_links {
            return false;
        }
        if from_links.iter().any(|l| l.target_id == to) {
            return true; // Already linked.
        }

        let to_links = self.edges.entry(to.clone()).or_default();
        if to_links.len() >= self.max_links {
            return false;
        }

        // Add forward link.
        let from_links = self.edges.get_mut(&from).unwrap();
        from_links.push(SeeAlsoLink {
            target_id: to.clone(),
            relationship,
            note: note.clone(),
        });

        // Add reverse link with the inverse relationship.
        let reverse_rel = invert_relationship(relationship);
        let to_links = self.edges.get_mut(&to).unwrap();
        to_links.push(SeeAlsoLink {
            target_id: from,
            relationship: reverse_rel,
            note,
        });

        true
    }

    /// Remove all links involving a given entry.
    pub fn remove_entry(&mut self, entry_id: &EntryId) {
        if let Some(links) = self.edges.remove(entry_id) {
            for link in links {
                if let Some(target_links) = self.edges.get_mut(&link.target_id) {
                    target_links.retain(|l| l.target_id != *entry_id);
                }
            }
        }
    }

    /// Get all direct links from an entry.
    pub fn get_links(&self, entry_id: &EntryId) -> &[SeeAlsoLink] {
        self.edges
            .get(entry_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Breadth-first traversal from a starting entry, up to `max_hops` hops.
    ///
    /// Returns `(entry_id, hop_count)` pairs, excluding the start.
    pub fn traverse(&self, start: &EntryId, max_hops: u32) -> Vec<(EntryId, u32)> {
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
                    if visited.insert(link.target_id.clone()) {
                        let hop = depth + 1;
                        results.push((link.target_id.clone(), hop));
                        queue.push_back((link.target_id.clone(), hop));
                    }
                }
            }
        }

        results
    }

    /// Find the shortest path between two entries. Returns `None` if no
    /// path exists, otherwise returns the hop count.
    pub fn shortest_path(&self, from: &EntryId, to: &EntryId) -> Option<u32> {
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
                    if link.target_id == *to {
                        return Some(depth + 1);
                    }
                    if visited.insert(link.target_id.clone()) {
                        queue.push_back((link.target_id.clone(), depth + 1));
                    }
                }
            }
        }

        None
    }

    /// Compute a distance-based score in [0.0, 1.0] for the see-also
    /// relationship between `from` and `to`.
    ///
    /// Direct links = 1.0, 2 hops = 0.5, 3 hops = 0.33, no path = 0.0.
    pub fn distance_score(&self, from: &EntryId, to: &EntryId) -> f64 {
        match self.shortest_path(from, to) {
            Some(0) => 1.0,
            Some(hops) => 1.0 / (hops as f64 + 1.0),
            None => 0.0,
        }
    }

    /// Number of entries in the graph.
    pub fn entry_count(&self) -> usize {
        self.edges.len()
    }

    /// Total number of directed edges.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }
}

/// Invert a relationship for the reverse direction.
pub fn invert_relationship(rel: Relationship) -> Relationship {
    match rel {
        Relationship::RelatedTo => Relationship::RelatedTo,
        Relationship::DependsOn => Relationship::DependsOn,
        Relationship::ContrastsWith => Relationship::ContrastsWith,
        Relationship::SupersededBy => Relationship::SupersededBy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidirectional_linking() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = EntryId("e1".into());
        let b = EntryId("e2".into());

        assert!(graph.add_link(
            a.clone(),
            b.clone(),
            Relationship::RelatedTo,
            "related work".into(),
        ));

        assert_eq!(graph.get_links(&a).len(), 1);
        assert_eq!(graph.get_links(&a)[0].target_id, b);
        assert_eq!(graph.get_links(&b).len(), 1);
        assert_eq!(graph.get_links(&b)[0].target_id, a);
    }

    #[test]
    fn traversal() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = EntryId("a".into());
        let b = EntryId("b".into());
        let c = EntryId("c".into());

        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());

        let one_hop = graph.traverse(&a, 1);
        assert_eq!(one_hop.len(), 1);

        let two_hops = graph.traverse(&a, 2);
        assert_eq!(two_hops.len(), 2);
    }

    #[test]
    fn distance_scoring() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = EntryId("a".into());
        let b = EntryId("b".into());
        let c = EntryId("c".into());

        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());

        assert!(graph.distance_score(&a, &b) > 0.4); // 1 hop: 1/(1+1) = 0.5
        assert!(graph.distance_score(&a, &c) > 0.3); // 2 hops: 1/(2+1) = 0.33
        assert_eq!(graph.distance_score(&a, &EntryId("x".into())), 0.0);
    }

    #[test]
    fn remove_entry_cleans_reverse() {
        let mut graph = SeeAlsoGraph::new(5);
        let a = EntryId("a".into());
        let b = EntryId("b".into());
        let c = EntryId("c".into());

        graph.add_link(a.clone(), b.clone(), Relationship::RelatedTo, "".into());
        graph.add_link(b.clone(), c.clone(), Relationship::RelatedTo, "".into());

        graph.remove_entry(&b);
        assert_eq!(graph.get_links(&a).len(), 0);
        assert_eq!(graph.get_links(&c).len(), 0);
    }
}
