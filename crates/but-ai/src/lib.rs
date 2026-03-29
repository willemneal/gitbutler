//! `but-ai` plugin for GitButler.
//!
//! This crate implements the AI agent system using a narrative memory architecture
//! where memory is stored as chapters in an ongoing story. Recurring themes emerge
//! as motifs that serve as retrieval anchors. Contradictions between memories create
//! dramatic tensions that the system flags for resolution.
//!
//! # Module Overview
//!
//! - [`types`] -- Shared types: chapters, motifs, tensions, arcs, colophons.
//! - [`narrative`] -- Narrative memory engine (chapters, motifs, tensions, arc summaries).
//! - [`workshop`] -- Literary workshop (author, editor, continuity checker, publisher).
//! - [`colophon`] -- Agent identity and commit metadata.

pub mod colophon;
pub mod narrative;
pub mod types;
pub mod workshop;
