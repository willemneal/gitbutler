//! `but-ai` plugin for GitButler -- The Longevity & Risk Research Centre.
//!
//! This crate implements an actuarial-table memory architecture for AI agents.
//! Every memory entry has a fitted survival function that models its probability
//! of remaining relevant over time. Different types of memories have different
//! mortality patterns:
//!
//! - **Architectural** memories follow Weibull distributions (slowly increasing hazard).
//! - **Bug-related** memories follow exponential distributions (instant irrelevance once fixed).
//! - **Convention** memories follow bathtub-shaped hazard functions (uncertain early,
//!   stable when established, unreliable as teams change).
//!
//! The plugin operates as a five-member research group: Vassiliev (PI/architect),
//! Okonkwo (practitioner/validator), Petrov (implementer), Abebe (memory curator),
//! and Chen (coordinator). Every task is a research study with a defined protocol.
//!
//! # Modules
//!
//! - [`types`]: Shared types (identifiers, distributions, memory entries, study protocol).
//! - [`survival`]: Survival analysis core (distributions, fitting, hazard rates, surprise).
//! - [`study`]: Study protocol execution (task lifecycle, patch generation, validation).
//! - [`cohort`]: Cohort-based memory management (storage, identity, coordination).

pub mod types;

pub mod survival;

pub mod study;

pub mod cohort;
