//! Memory storage, classification, retrieval, and lifecycle management.
//!
//! This module implements the unified memory system combining:
//! - **Store**: Three-state in-memory storage (alive/moribund/deceased)
//! - **Classification**: Five classification systems for memory entries
//! - **Call numbers**: Hierarchical knowledge addressing
//! - **See also**: Bidirectional cross-reference graph
//! - **Controlled vocabulary**: Term normalization and query expansion
//! - **Retrieval**: 6-component scoring engine
//! - **Lifecycle**: Three-state transitions with survival-based audit
//! - **Compaction**: Circulation-based tiered compaction

pub mod call_number;
pub mod classification;
pub mod compaction;
pub mod controlled_vocab;
pub mod lifecycle;
pub mod retrieval;
pub mod see_also;
pub mod store;
