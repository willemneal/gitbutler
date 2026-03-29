//! Circulation — the flow of work and memory.
//!
//! The circulation desk is where items are checked out, checked in,
//! renewed, and placed on hold. This module manages:
//! - **Tracking**: Memory access/circulation event logging
//! - **Holds**: Task dependency queue and priority management
//! - **Adapter**: Forge-agnostic PR/comment/label operations

pub mod adapter;
pub mod holds;
pub mod tracker;
