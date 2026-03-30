//! Agent identity and key management.
//!
//! Provides:
//! - **signing**: `CommitSigner` implementations (NoOp, DenyAll) and key audit log.
//! - **key_lifecycle**: Key provision/rotate/compromise/decommission with
//!   survival-based rotation scheduling.
//! - **authorization**: Role-based authorization checking with branch pattern
//!   matching and patch size validation.
//! - **performance**: `PerformanceHistory` tracking with running mean calculations.

pub mod authorization;
pub mod key_lifecycle;
pub mod performance;
pub mod signing;

pub use authorization::{
    check_branch_authorization, check_full_authorization, check_patch_size,
    check_repo_authorization, AuthorizationResult,
};
pub use key_lifecycle::{KeyManager, KeyRecord, KeyStatus};
pub use performance::{record_task_completion, reliability_score};
pub use signing::{DenyAllSigner, KeyAuditLog, KeyLifecycleEvent, NoOpSigner};
