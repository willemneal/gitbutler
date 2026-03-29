//! Study protocol module.
//!
//! Every task is a research study conducted by the five-member LRRC team.
//! This module implements:
//!
//! - **protocol**: The six-phase study lifecycle (literature review through
//!   publication), with budget-adaptive protocol modes.
//! - **patch**: INDEX.patch and COMMIT.msg generation with actuarial metadata
//!   (survival estimates, confidence intervals, known limitations).
//! - **validation**: Statistical validation of implementation correctness,
//!   including Okonkwo's practitioner review criteria.

pub mod patch;
pub mod protocol;
pub mod validation;
