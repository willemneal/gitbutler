//! Cohort-based memory management module.
//!
//! This module implements:
//!
//! - **memory**: The actuarial-table memory store, managing the lifecycle
//!   of memory entries from alive through moribund to deceased.
//! - **identity**: Agent identity with statistical credentials (life records).
//! - **coordination**: Multi-site study coordination via PR comments,
//!   modeling cross-repo work as multi-site clinical trials.

pub mod coordination;
pub mod identity;
pub mod memory;
