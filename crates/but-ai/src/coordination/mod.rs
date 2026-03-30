//! Cross-repo and cross-agent coordination.
//!
//! - [`forge`] -- `ForgeAdapter` trait implementation and `InMemoryForge` test double.
//! - [`messages`] -- PR comment schema: serialize/deserialize `CoordinationMessage` in code fences.
//! - [`gossip`] -- CRDT-based memory sync with `VectorClock` and `GossipEngine`.
//! - [`dependency`] -- Cross-repo dependency DAG with topological sort (Kahn's algorithm).

pub mod dependency;
pub mod forge;
pub mod gossip;
pub mod messages;

pub use dependency::{DependencyGraph, DependencyNode};
pub use forge::InMemoryForge;
pub use gossip::{GossipEngine, GossipRequest, GossipResponse, VectorClock};
pub use messages::{is_coordination_comment, parse, parse_first, render, SCHEMA_VERSION};
