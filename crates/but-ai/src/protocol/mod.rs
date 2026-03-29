//! Protocol module -- inter-agent coordination via structured PR comments.
//!
//! PRs are datagrams. Comments are structured messages. The forge is the
//! network layer. This module defines the message format, consensus
//! mechanisms, and bounded negotiation protocol.
//!
//! # Submodules
//!
//! - [`message`] -- Structured PR comment messages (task assignment, status, dependency).
//! - [`consensus`] -- Consensus mechanisms for distributed decisions (3-of-5 quorum).
//! - [`negotiation`] -- Bounded negotiation within tide cycles (propose/counter/resolve).

pub mod consensus;
pub mod message;
pub mod negotiation;

pub use consensus::{equal_weights, ConsensusEngine, ConsensusRound};
pub use message::{
    BudgetReportBody, ConsensusRequestBody, ConsensusVoteBody, DependencyDeclarationBody,
    MessageBody, ParsedHeader, PatchHandoffBody, ProtocolMessage, StatusReportBody,
    TaskAssignmentBody,
};
pub use negotiation::{
    CounterProposal, Negotiation, NegotiationOutcome, NegotiationStage, Proposal,
};
