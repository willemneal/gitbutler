//! `but-ai` plugin for GitButler.
//!
//! This crate implements the AI agent system using a woven memory architecture
//! where persistent context (warp) and task-specific context (weft) are interlaced
//! according to weave patterns (plain, twill, satin) to produce output fabrics
//! (INDEX.patch + COMMIT.msg).
//!
//! # Module Overview
//!
//! - [`types`] -- Shared types used across all modules.
//! - [`loom`] -- Memory weaving engine (warp, weft, pattern selection, heddle control).
//! - [`shuttle`] -- Cross-repo coordination via forge PR comments.
//! - [`selvedge`] -- Validation and boundary checking of produced fabrics.
//! - [`fabric`] -- Output production (patch generation, commit messages, progress).

pub mod fabric;
pub mod loom;
pub mod selvedge;
pub mod shuttle;
pub mod types;
