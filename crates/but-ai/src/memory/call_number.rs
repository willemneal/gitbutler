//! Call number generation and hierarchy tree.
//!
//! A call number is a hierarchical address in the knowledge structure,
//! like a Library of Congress call number positions a book on the shelf.
//! `ARCH.AUTH.MIDDLEWARE.TOKEN-VALIDATION` places a memory at increasing
//! specificity within the architecture domain.

use crate::types::CallNumber;

/// Well-known top-level call number categories.
pub const TOP_LEVEL_CATEGORIES: &[(&str, &str)] = &[
    ("ARCH", "Architecture decisions"),
    ("TEST", "Testing knowledge"),
    ("TOOL", "Tooling and build knowledge"),
    ("SEC", "Security knowledge"),
    ("DOM", "Domain-specific knowledge"),
    ("API", "API layer knowledge"),
    ("DB", "Database knowledge"),
    ("OPS", "Operations and deployment"),
    ("DOC", "Documentation patterns"),
    ("REF", "Reference and standards"),
];

/// A node in the call number hierarchy tree.
#[derive(Debug, Clone)]
pub struct HierarchyNode {
    /// The segment name at this level.
    pub segment: String,
    /// Human-readable description.
    pub description: String,
    /// Children in the hierarchy.
    pub children: Vec<HierarchyNode>,
}

/// The full call number hierarchy (classification scheme).
#[derive(Debug, Clone)]
pub struct CallNumberHierarchy {
    /// Root nodes (top-level categories).
    pub roots: Vec<HierarchyNode>,
    /// Maximum allowed depth.
    pub max_depth: usize,
}

impl CallNumberHierarchy {
    /// Create a default hierarchy with the well-known top-level categories.
    pub fn default_hierarchy(max_depth: usize) -> Self {
        let roots = TOP_LEVEL_CATEGORIES
            .iter()
            .map(|(seg, desc)| HierarchyNode {
                segment: seg.to_string(),
                description: desc.to_string(),
                children: Vec::new(),
            })
            .collect();
        Self { roots, max_depth }
    }

    /// Register a call number in the hierarchy, creating intermediate nodes
    /// as needed. Returns `false` if the call number exceeds max depth.
    pub fn register(&mut self, call_number: &CallNumber) -> bool {
        if call_number.depth() > self.max_depth {
            tracing::warn!(
                call_number = %call_number,
                max_depth = self.max_depth,
                "call number exceeds maximum depth"
            );
            return false;
        }

        if call_number.segments.is_empty() {
            return false;
        }

        let mut current_level = &mut self.roots;
        for segment in &call_number.segments {
            let pos = current_level.iter().position(|n| n.segment == *segment);
            let idx = match pos {
                Some(i) => i,
                None => {
                    current_level.push(HierarchyNode {
                        segment: segment.clone(),
                        description: String::new(),
                        children: Vec::new(),
                    });
                    current_level.len() - 1
                }
            };
            current_level = &mut current_level[idx].children;
        }
        true
    }

    /// Look up a call number and return the matching node's description.
    pub fn lookup(&self, call_number: &CallNumber) -> Option<&str> {
        let mut current_level = &self.roots;
        let mut last_desc = None;

        for segment in &call_number.segments {
            match current_level.iter().find(|n| n.segment == *segment) {
                Some(node) => {
                    if !node.description.is_empty() {
                        last_desc = Some(node.description.as_str());
                    }
                    current_level = &node.children;
                }
                None => return last_desc,
            }
        }
        last_desc
    }

    /// Find all call numbers that share a prefix with the given call number.
    /// Returns call numbers of sibling nodes.
    pub fn find_neighbors(&self, call_number: &CallNumber) -> Vec<CallNumber> {
        let mut results = Vec::new();
        if call_number.segments.is_empty() {
            return results;
        }

        let parent_segments = &call_number.segments[..call_number.segments.len() - 1];
        let mut current_level = &self.roots;

        for segment in parent_segments {
            match current_level.iter().find(|n| n.segment == *segment) {
                Some(node) => current_level = &node.children,
                None => return results,
            }
        }

        let self_segment = call_number.segments.last().unwrap();
        for node in current_level {
            if node.segment != *self_segment {
                let mut segs: Vec<String> = parent_segments.to_vec();
                segs.push(node.segment.clone());
                results.push(CallNumber { segments: segs });
            }
        }

        results
    }

    /// Collect all registered call numbers as a flat list.
    pub fn all_call_numbers(&self) -> Vec<CallNumber> {
        let mut result = Vec::new();
        fn collect(nodes: &[HierarchyNode], prefix: &[String], out: &mut Vec<CallNumber>) {
            for node in nodes {
                let mut path = prefix.to_vec();
                path.push(node.segment.clone());
                out.push(CallNumber {
                    segments: path.clone(),
                });
                collect(&node.children, &path, out);
            }
        }
        collect(&self.roots, &[], &mut result);
        result
    }
}

/// Generate a call number from a file path.
///
/// Maps directory structure to call number segments:
/// `crates/but-ai/src/catalog/classification.rs` -> `CRATES.BUT-AI.CATALOG.CLASSIFICATION`
pub fn call_number_from_path(path: &str, max_depth: usize) -> CallNumber {
    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|s| *s != "src" && *s != "lib" && *s != "mod.rs")
        .map(|s| {
            s.rsplit_once('.')
                .map(|(name, _ext)| name)
                .unwrap_or(s)
                .to_uppercase()
        })
        .take(max_depth)
        .collect();

    CallNumber { segments }
}

/// Compute the distance between two call numbers.
///
/// The distance is the number of segments that differ after the shared
/// prefix. Two identical call numbers have distance 0.
pub fn call_number_distance(a: &CallNumber, b: &CallNumber) -> usize {
    let shared = a.shared_depth(b);
    (a.depth() - shared) + (b.depth() - shared)
}

/// Compute a proximity score in [0.0, 1.0] between two call numbers.
///
/// Returns `shared_depth / max(a.depth, b.depth)`. Identical call numbers
/// score 1.0; completely unrelated call numbers score 0.0.
pub fn call_number_proximity(a: &CallNumber, b: &CallNumber) -> f64 {
    let max_depth = a.depth().max(b.depth());
    if max_depth == 0 {
        return 1.0;
    }
    a.shared_depth(b) as f64 / max_depth as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display() {
        let cn = CallNumber::parse("ARCH.AUTH.MIDDLEWARE");
        assert_eq!(cn.segments, vec!["ARCH", "AUTH", "MIDDLEWARE"]);
        assert_eq!(cn.to_string(), "ARCH.AUTH.MIDDLEWARE");
    }

    #[test]
    fn hierarchy_registration() {
        let mut h = CallNumberHierarchy::default_hierarchy(5);
        assert!(h.register(&CallNumber::parse("ARCH.AUTH.MIDDLEWARE")));
        assert!(h.register(&CallNumber::parse("ARCH.AUTH.SESSION")));

        let neighbors = h.find_neighbors(&CallNumber::parse("ARCH.AUTH.MIDDLEWARE"));
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].to_string(), "ARCH.AUTH.SESSION");
    }

    #[test]
    fn path_to_call_number() {
        let cn = call_number_from_path("crates/but-ai/src/memory/store.rs", 5);
        assert_eq!(cn.to_string(), "CRATES.BUT-AI.MEMORY.STORE");
    }

    #[test]
    fn distance_calculation() {
        let a = CallNumber::parse("ARCH.AUTH.MIDDLEWARE");
        let b = CallNumber::parse("ARCH.AUTH.SESSION");
        assert_eq!(call_number_distance(&a, &b), 2);
    }

    #[test]
    fn proximity_scoring() {
        let a = CallNumber::parse("ARCH.AUTH.MIDDLEWARE");
        let b = CallNumber::parse("ARCH.AUTH.SESSION");
        let score = call_number_proximity(&a, &b);
        // shared=2, max_depth=3: 2/3 ≈ 0.667
        assert!(score > 0.6 && score < 0.7);
    }

    #[test]
    fn max_depth_enforcement() {
        let mut h = CallNumberHierarchy::default_hierarchy(3);
        assert!(!h.register(&CallNumber::parse("A.B.C.D")));
        assert!(h.register(&CallNumber::parse("A.B.C")));
    }
}
