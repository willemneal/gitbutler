//! Colophon: identity and metadata.
//!
//! The colophon module manages agent identity (modeled on the colophon pages
//! of hand-printed books) and commit metadata (narrative trailers that provide
//! structured information for memory retrieval).

mod identity;
mod metadata;

pub use identity::IdentityManager;
pub use metadata::{
    branch_name, extract_metadata, CommitMessageBuilder, NarrativeTrailer,
};
