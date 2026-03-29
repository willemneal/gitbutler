//! The shelf — where code is placed.
//!
//! Shelver's job is to put code in the right place. This module
//! provides:
//! - **Patch generation**: Building patches with catalog metadata
//! - **Placement**: Determining where new code belongs
//! - **Organization**: Assessing module/file structural health

pub mod organization;
pub mod patch;
pub mod placement;
