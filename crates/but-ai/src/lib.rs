//! `but-ai` — Card-Catalog Memory Architecture for GitButler.
//!
//! Built on the principles of library science: **classify** thoroughly,
//! **shelve** accurately, **circulate** efficiently.
//!
//! The core insight is that the agent memory problem is a cataloging
//! problem — the same problem librarians solved in the 19th century
//! with the card catalog and have been refining ever since.
//!
//! # Architecture
//!
//! The crate is organized into three modules mirroring a library system:
//!
//! - **[`catalog`]** — The card catalog: five simultaneous classification
//!   systems, "see also" cross-reference graph, call number hierarchy,
//!   and controlled vocabulary. This is where memories are classified
//!   and retrieved.
//!
//! - **[`shelf`]** — The stacks: patch generation with catalog metadata,
//!   code placement logic, and module organization assessment. This is
//!   where code is placed in the right location.
//!
//! - **[`circulation`]** — The circulation desk: memory access tracking,
//!   task dependency holds queue, and forge adapter for PR/comment
//!   operations. This is where work flows between agents and repos.
//!
//! All shared types live in [`types`].

pub mod catalog;
pub mod circulation;
pub mod shelf;
pub mod types;
