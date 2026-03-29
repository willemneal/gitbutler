//! Cross-repo coordination.
//!
//! The shuttle carries coordination threads between looms (repositories).
//! Communication happens via PR comments containing structured JSON messages
//! in `but-ai-shuttle` code fences.

mod bridge;
mod coordination;

pub use bridge::{CoordinationPattern, FabricStatus, LoomBridge, ShuttleMessage, ShuttleMessageType};
pub use coordination::{CutPlan, CutPlanEntry, ShuttleService};
