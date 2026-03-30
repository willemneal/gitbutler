//! Phase-gated tool loading.
//!
//! For each `TaskPhase`, defines which tools are available. This prevents
//! implementation-phase tools from being accessible during classification,
//! and validation tools from being used during implementation.

use crate::types::TaskPhase;

use super::ToolSpec;

/// Return the tools available for a given task phase.
///
/// Tool availability follows the principle of least privilege:
/// - **Classify/Plan**: read-only tools for memory retrieval and analysis.
/// - **Implement**: write tools for file modification and patch creation.
/// - **Validate**: read + check tools for verification.
/// - **Catalog**: write tools for memory storage only.
/// - **Coordinate**: network tools for forge interaction.
pub fn tools_for_phase(phase: TaskPhase) -> Vec<ToolSpec> {
    match phase {
        TaskPhase::Classify => classify_tools(),
        TaskPhase::Plan => plan_tools(),
        TaskPhase::Implement => implement_tools(),
        TaskPhase::Validate => validate_tools(),
        TaskPhase::Catalog => catalog_tools(),
        TaskPhase::Coordinate => coordinate_tools(),
    }
}

/// Check whether a tool is allowed in the given phase.
pub fn is_tool_allowed(tool_name: &str, phase: TaskPhase) -> bool {
    tools_for_phase(phase)
        .iter()
        .any(|t| t.name == tool_name)
}

/// Return all phases that allow a given tool.
pub fn phases_for_tool(tool_name: &str) -> Vec<TaskPhase> {
    let all_phases = [
        TaskPhase::Classify,
        TaskPhase::Plan,
        TaskPhase::Implement,
        TaskPhase::Validate,
        TaskPhase::Catalog,
        TaskPhase::Coordinate,
    ];
    all_phases
        .iter()
        .filter(|phase| is_tool_allowed(tool_name, **phase))
        .copied()
        .collect()
}

fn classify_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "memory-retrieve".into(),
            description: "Query memory store for relevant entries".into(),
            phase: TaskPhase::Classify,
        },
        ToolSpec {
            name: "memory-list".into(),
            description: "List memory entry IDs by state".into(),
            phase: TaskPhase::Classify,
        },
        ToolSpec {
            name: "git-log".into(),
            description: "View git commit history".into(),
            phase: TaskPhase::Classify,
        },
        ToolSpec {
            name: "git-diff".into(),
            description: "View file differences".into(),
            phase: TaskPhase::Classify,
        },
        ToolSpec {
            name: "file-read".into(),
            description: "Read file contents".into(),
            phase: TaskPhase::Classify,
        },
        ToolSpec {
            name: "file-search".into(),
            description: "Search for files by pattern".into(),
            phase: TaskPhase::Classify,
        },
    ]
}

fn plan_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "memory-retrieve".into(),
            description: "Query memory store for relevant entries".into(),
            phase: TaskPhase::Plan,
        },
        ToolSpec {
            name: "file-read".into(),
            description: "Read file contents".into(),
            phase: TaskPhase::Plan,
        },
        ToolSpec {
            name: "file-search".into(),
            description: "Search for files by pattern".into(),
            phase: TaskPhase::Plan,
        },
        ToolSpec {
            name: "git-log".into(),
            description: "View git commit history".into(),
            phase: TaskPhase::Plan,
        },
        ToolSpec {
            name: "git-blame".into(),
            description: "Annotate file with commit info".into(),
            phase: TaskPhase::Plan,
        },
    ]
}

fn implement_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "file-read".into(),
            description: "Read file contents".into(),
            phase: TaskPhase::Implement,
        },
        ToolSpec {
            name: "file-write".into(),
            description: "Write file contents".into(),
            phase: TaskPhase::Implement,
        },
        ToolSpec {
            name: "file-edit".into(),
            description: "Edit file with replacements".into(),
            phase: TaskPhase::Implement,
        },
        ToolSpec {
            name: "git-diff".into(),
            description: "View current changes".into(),
            phase: TaskPhase::Implement,
        },
        ToolSpec {
            name: "git-commit".into(),
            description: "Create a commit".into(),
            phase: TaskPhase::Implement,
        },
        ToolSpec {
            name: "shell-run".into(),
            description: "Execute shell commands".into(),
            phase: TaskPhase::Implement,
        },
    ]
}

fn validate_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "file-read".into(),
            description: "Read file contents".into(),
            phase: TaskPhase::Validate,
        },
        ToolSpec {
            name: "git-diff".into(),
            description: "View current changes".into(),
            phase: TaskPhase::Validate,
        },
        ToolSpec {
            name: "memory-retrieve".into(),
            description: "Query memory for contradiction checking".into(),
            phase: TaskPhase::Validate,
        },
        ToolSpec {
            name: "continuity-check".into(),
            description: "Run continuity validation".into(),
            phase: TaskPhase::Validate,
        },
        ToolSpec {
            name: "tool-risk-classify".into(),
            description: "Classify tool risk level".into(),
            phase: TaskPhase::Validate,
        },
    ]
}

fn catalog_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "memory-store".into(),
            description: "Store a memory entry".into(),
            phase: TaskPhase::Catalog,
        },
        ToolSpec {
            name: "memory-transition".into(),
            description: "Transition entry lifecycle state".into(),
            phase: TaskPhase::Catalog,
        },
        ToolSpec {
            name: "memory-classify".into(),
            description: "Classify and catalog a memory entry".into(),
            phase: TaskPhase::Catalog,
        },
    ]
}

fn coordinate_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "forge-create-pr".into(),
            description: "Create a pull request".into(),
            phase: TaskPhase::Coordinate,
        },
        ToolSpec {
            name: "forge-comment".into(),
            description: "Post a PR comment".into(),
            phase: TaskPhase::Coordinate,
        },
        ToolSpec {
            name: "forge-add-label".into(),
            description: "Add label to a PR".into(),
            phase: TaskPhase::Coordinate,
        },
        ToolSpec {
            name: "forge-list-prs".into(),
            description: "List PRs by label".into(),
            phase: TaskPhase::Coordinate,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_has_read_only_tools() {
        let tools = tools_for_phase(TaskPhase::Classify);
        assert!(tools.iter().all(|t| !t.name.contains("write")
            && !t.name.contains("commit")
            && !t.name.contains("create-pr")));
    }

    #[test]
    fn implement_has_write_tools() {
        let tools = tools_for_phase(TaskPhase::Implement);
        assert!(tools.iter().any(|t| t.name == "file-write"));
        assert!(tools.iter().any(|t| t.name == "git-commit"));
    }

    #[test]
    fn validate_has_check_tools() {
        let tools = tools_for_phase(TaskPhase::Validate);
        assert!(tools.iter().any(|t| t.name == "continuity-check"));
        assert!(tools.iter().any(|t| t.name == "tool-risk-classify"));
        // Validate should NOT have write tools.
        assert!(tools.iter().all(|t| t.name != "file-write"));
    }

    #[test]
    fn coordinate_has_forge_tools() {
        let tools = tools_for_phase(TaskPhase::Coordinate);
        assert!(tools.iter().any(|t| t.name == "forge-create-pr"));
        assert!(tools.iter().any(|t| t.name == "forge-comment"));
    }

    #[test]
    fn tool_allowed_check() {
        assert!(is_tool_allowed("file-read", TaskPhase::Classify));
        assert!(!is_tool_allowed("file-write", TaskPhase::Classify));
        assert!(is_tool_allowed("file-write", TaskPhase::Implement));
    }

    #[test]
    fn phases_for_file_read() {
        let phases = phases_for_tool("file-read");
        // file-read should be available in classify, plan, implement, validate.
        assert!(phases.contains(&TaskPhase::Classify));
        assert!(phases.contains(&TaskPhase::Plan));
        assert!(phases.contains(&TaskPhase::Implement));
        assert!(phases.contains(&TaskPhase::Validate));
        // But not in catalog or coordinate.
        assert!(!phases.contains(&TaskPhase::Catalog));
        assert!(!phases.contains(&TaskPhase::Coordinate));
    }
}
