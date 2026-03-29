//! Identity module -- peer agent identity and signing.
//!
//! Each agent is a peer in the collective -- no hierarchy, no harbormaster.
//! Identity records are stored in Git refs and versioned. Signing is handled
//! through an OpenWallet abstraction layer.
//!
//! # Submodules
//!
//! - [`agent`] -- Peer agent identity, registry, and authorization checking.
//! - [`signing`] -- Commit signing abstraction (OpenWallet placeholder).

pub mod agent;
pub mod signing;

pub use agent::{AgentIdentityBuilder, AgentRegistry, AuthorizationCheck};
pub use signing::{
    CommitSigner, DenyAllSigner, KeyAuditLog, KeyLifecycleEvent, NoOpSigner, SignedPayload,
    SigningAuthorization, SigningRequest,
};
