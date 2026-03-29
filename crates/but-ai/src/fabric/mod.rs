//! Output production.
//!
//! The fabric module produces the final output of a weaving operation:
//! an INDEX.patch (unified diff) and COMMIT.msg (with loom metadata trailers).

mod commit;
mod patch;
mod progress;

pub use commit::{compose_commit_message, CommitMessageBuilder, LoomTrailer};
pub use patch::{FileChange, PatchBuilder, PatchGenerator, PatchStats};
pub use progress::ProgressReporter;

use crate::types::{AgentId, Fabric, FabricMetadata, WeavePattern};

/// Produces the final fabric (INDEX.patch + COMMIT.msg) from a weaving operation.
pub struct FabricProducer {
    agent: AgentId,
    pattern: WeavePattern,
}

impl FabricProducer {
    pub fn new(agent: AgentId, pattern: WeavePattern) -> Self {
        Self { agent, pattern }
    }

    /// Assemble a complete fabric from patch content and task description.
    pub fn produce(
        &self,
        patch_content: &str,
        task_description: &str,
        warp_count: u32,
        weft_count: u32,
        inspector: Option<AgentId>,
    ) -> anyhow::Result<Fabric> {
        let patch = PatchBuilder::new(patch_content).validate()?.build();

        let metadata = FabricMetadata {
            thread_count: format!("{}W/{}F", warp_count, weft_count),
            weave_pattern: self.pattern,
            inspected_by: inspector,
        };

        let commit_message = CommitMessageBuilder::new()
            .with_description(task_description)
            .with_trailer(LoomTrailer::from_metadata(&metadata))
            .build()?;

        Ok(Fabric {
            patch,
            commit_message,
            metadata,
        })
    }
}
