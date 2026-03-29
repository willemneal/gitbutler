//! Budget module -- token budget tracking and provider abstraction.
//!
//! Raul's territory. Every LLM call has a cost, every task has a budget.
//! The agent must complete its work within budget or produce a valid partial
//! result. "What's the burn rate?"
//!
//! # Submodules
//!
//! - [`tracker`] -- Token budget tracking per agent per tide cycle.
//! - [`provider`] -- Provider abstraction shim (OpenAI/Anthropic/Ollama/LMStudio).

pub mod provider;
pub mod tracker;

pub use provider::{ProviderCapabilities, ProviderHandle};
pub use tracker::{
    BudgetAllocation, BudgetCheckpoint, BudgetSnapshot, BudgetTracker, ExecutionPhase,
};
