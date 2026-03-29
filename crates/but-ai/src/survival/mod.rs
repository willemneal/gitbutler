//! Survival analysis module.
//!
//! This module implements the statistical core of the actuarial-table memory
//! system. It provides:
//!
//! - **distributions**: Parametric survival distributions (Exponential, Weibull,
//!   Bathtub, Log-normal) with S(t), f(t), and h(t) computations.
//! - **fitting**: Maximum likelihood estimation for fitting distributions to
//!   observed memory access patterns, with AIC-based model selection.
//! - **hazard**: Hazard rate computation, mortality classification, and
//!   lifecycle state management.
//! - **surprise**: Bayesian surprise detection using KL divergence, with
//!   cohort-level analysis for detecting systematic shifts.

pub mod distributions;
pub mod fitting;
pub mod hazard;
pub mod surprise;
