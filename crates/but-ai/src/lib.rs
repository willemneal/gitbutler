//! `but-ai` plugin for GitButler -- Tidal Protocol Collective.
//!
//! This crate implements the AI agent system using a CRDT manifest memory
//! architecture where coordination is a protocol problem, not a hierarchy problem.
//!
//! Three principles anchor the design:
//!
//! 1. **Consensus over command.** Agents negotiate through structured PR comments,
//!    not through a central orchestrator. Patches require consensus validation.
//! 2. **The manifest is the memory.** Agent memory is stored as a distributed
//!    manifest in Git refs, synchronized using a CRDT-based gossip protocol.
//! 3. **The tide is the clock.** All coordination uses a fixed 6-hour cycle.
//!    Decisions that cannot reach consensus within one tide are deferred.
//!
//! *"No harbormaster. The protocol is the authority."*
//!
//! # Module Overview
//!
//! - [`types`] -- Shared types (ManifestEntry, TidePhase, ConsensusWeight, etc.).
//! - [`manifest`] -- CRDT manifest memory: entries, gossip, relevance scoring, tidal clock.
//! - [`protocol`] -- Inter-agent coordination: PR messages, consensus, negotiation.
//! - [`identity`] -- Peer agent identity, authorization, and commit signing.
//! - [`budget`] -- Token budget tracking and provider abstraction.

pub mod budget;
pub mod identity;
pub mod manifest;
pub mod protocol;
pub mod types;
