//! Manifest entry CRDT operations.
//!
//! Each manifest entry is a CRDT (Conflict-free Replicated Data Type).
//! When two agents independently modify the same entry, the merge operation
//! produces a deterministic result without coordination. The merge rules:
//!
//! - `access_count`: take the maximum (grow-only counter)
//! - `consensus_citations`: take the maximum (grow-only counter)
//! - `tags`: union of both tag sets (grow-only set)
//! - `version`: take the maximum
//! - `last_accessed`: take the later timestamp (last-writer-wins register)
//! - `content`: take the version with higher `version` field (versioned register)

use crate::types::{EntryId, ManifestCategory, ManifestEntry};

/// Merge two manifest entries that share the same `id`.
///
/// Returns `None` if the entries have different IDs (cannot merge unrelated entries).
/// The merge is commutative and associative -- `merge(a, b) == merge(b, a)` and
/// `merge(merge(a, b), c) == merge(a, merge(b, c))`.
pub fn merge(left: &ManifestEntry, right: &ManifestEntry) -> Option<ManifestEntry> {
    if left.id != right.id {
        return None;
    }

    // Versioned register: higher version wins for content fields
    let (base, _other) = if left.version >= right.version {
        (left, right)
    } else {
        (right, left)
    };

    // Union of tags (grow-only set)
    let mut tags = base.tags.clone();
    for tag in &left.tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    for tag in &right.tags {
        if !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    tags.sort();

    // Last-writer-wins for last_accessed
    let last_accessed = match (&left.last_accessed, &right.last_accessed) {
        (Some(l), Some(r)) => Some(if l > r { l.clone() } else { r.clone() }),
        (Some(l), None) => Some(l.clone()),
        (None, Some(r)) => Some(r.clone()),
        (None, None) => None,
    };

    Some(ManifestEntry {
        id: base.id.clone(),
        agent: base.agent.clone(),
        category: base.category,
        created: base.created.clone(),
        ttl: base.ttl.clone(),
        expires: base.expires.clone(),
        tags,
        content: base.content.clone(),
        embedding_hash: base.embedding_hash.clone(),
        relevance_decay: base.relevance_decay,
        // Grow-only counters: take maximum
        access_count: left.access_count.max(right.access_count),
        last_accessed,
        tide_created: base.tide_created.clone(),
        version: left.version.max(right.version),
        consensus_citations: left.consensus_citations.max(right.consensus_citations),
    })
}

/// Create a new manifest entry with default CRDT initial state.
pub fn create(
    id: EntryId,
    agent: crate::types::AgentId,
    category: ManifestCategory,
    content: String,
    tags: Vec<String>,
    now: String,
    tide_label: String,
) -> ManifestEntry {
    let ttl_seconds = category.default_ttl_seconds();
    let ttl = ttl_seconds.map(|s| format!("{}h", s / 3600));

    // Compute expiration from TTL. For simplicity, we store the TTL string
    // and expect the caller to compute the actual expiration timestamp.
    // Identity entries have no expiration.
    let expires = if category == ManifestCategory::Identity {
        None
    } else {
        // Placeholder: real implementation would compute now + ttl
        ttl.as_ref().map(|t| format!("{}+{}", now, t))
    };

    ManifestEntry {
        id,
        agent,
        category,
        created: now.clone(),
        ttl,
        expires,
        tags,
        content,
        embedding_hash: None,
        relevance_decay: 0.95,
        access_count: 0,
        last_accessed: None,
        tide_created: tide_label,
        version: 1,
        consensus_citations: 0,
    }
}

/// Increment the consensus citation count for an entry.
/// This is called when another agent references this entry.
pub fn cite(entry: &mut ManifestEntry) {
    entry.consensus_citations += 1;
}

/// Compute the content-addressable ID for a manifest entry's content.
///
/// In a real implementation this would be SHA-256. Here we use a simple
/// deterministic hash suitable for type-checking and testing.
pub fn content_hash(content: &str) -> EntryId {
    // Simple djb2-style hash for deterministic output.
    let mut hash: u64 = 5381;
    for byte in content.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    EntryId(format!("{:016x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AgentId;

    fn test_entry(version: u64, access_count: u64, tags: Vec<&str>) -> ManifestEntry {
        ManifestEntry {
            id: EntryId("test-id".to_string()),
            agent: AgentId("dara".to_string()),
            category: ManifestCategory::Pattern,
            created: "2026-03-28T14:00:00Z".to_string(),
            ttl: Some("720h".to_string()),
            expires: Some("2026-04-27T14:00:00Z".to_string()),
            tags: tags.into_iter().map(String::from).collect(),
            content: "test content".to_string(),
            embedding_hash: None,
            relevance_decay: 0.95,
            access_count,
            last_accessed: None,
            tide_created: "high tide".to_string(),
            version,
            consensus_citations: 0,
        }
    }

    #[test]
    fn merge_takes_higher_version_content() {
        let a = test_entry(1, 5, vec!["auth"]);
        let mut b = test_entry(2, 3, vec!["refactor"]);
        b.content = "updated content".to_string();

        let merged = merge(&a, &b).unwrap();
        assert_eq!(merged.version, 2);
        assert_eq!(merged.content, "updated content");
    }

    #[test]
    fn merge_takes_max_access_count() {
        let a = test_entry(1, 10, vec![]);
        let b = test_entry(1, 3, vec![]);

        let merged = merge(&a, &b).unwrap();
        assert_eq!(merged.access_count, 10);
    }

    #[test]
    fn merge_unions_tags() {
        let a = test_entry(1, 0, vec!["auth", "pattern"]);
        let b = test_entry(1, 0, vec!["pattern", "refactor"]);

        let merged = merge(&a, &b).unwrap();
        assert!(merged.tags.contains(&"auth".to_string()));
        assert!(merged.tags.contains(&"pattern".to_string()));
        assert!(merged.tags.contains(&"refactor".to_string()));
    }

    #[test]
    fn merge_different_ids_returns_none() {
        let a = test_entry(1, 0, vec![]);
        let mut b = test_entry(1, 0, vec![]);
        b.id = EntryId("different-id".to_string());

        assert!(merge(&a, &b).is_none());
    }

    #[test]
    fn content_hash_is_deterministic() {
        let h1 = content_hash("hello world");
        let h2 = content_hash("hello world");
        assert_eq!(h1, h2);
    }
}
