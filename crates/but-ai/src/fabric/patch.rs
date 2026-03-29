//! Patch generation for the fabric output module.
//!
//! Produces unified diffs (INDEX.patch content) from sets of file changes.
//! The PatchGenerator creates diffs from before/after content pairs, while
//! PatchBuilder validates and wraps raw diff content.

use std::collections::BTreeMap;

/// A single file change to be included in a patch.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

impl FileChange {
    /// Create a change representing a new file.
    pub fn addition(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            old_content: None,
            new_content: Some(content.into()),
        }
    }

    /// Create a change representing a deleted file.
    pub fn deletion(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            old_content: Some(content.into()),
            new_content: None,
        }
    }

    /// Create a change representing a modification.
    pub fn modification(
        path: impl Into<String>,
        old: impl Into<String>,
        new: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            old_content: Some(old.into()),
            new_content: Some(new.into()),
        }
    }
}

/// Generates INDEX.patch content from a set of file changes.
///
/// Produces a unified diff with standard `---`/`+++`/`@@` markers.
/// Files are sorted lexicographically for deterministic output.
pub struct PatchGenerator {
    changes: Vec<FileChange>,
    context_lines: usize,
}

impl PatchGenerator {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            context_lines: 3,
        }
    }

    /// Set the number of context lines around each hunk (default: 3).
    pub fn with_context_lines(mut self, n: usize) -> Self {
        self.context_lines = n;
        self
    }

    /// Add a file change to the patch.
    pub fn add_change(mut self, change: FileChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Generate the unified diff string.
    pub fn generate(self) -> anyhow::Result<String> {
        if self.changes.is_empty() {
            anyhow::bail!("No file changes to generate patch from");
        }

        // Sort changes by path for deterministic output.
        let mut sorted: BTreeMap<String, &FileChange> = BTreeMap::new();
        for change in &self.changes {
            sorted.insert(change.path.clone(), change);
        }

        let mut output = String::new();
        for (path, change) in &sorted {
            let file_diff = self.generate_file_diff(path, change)?;
            output.push_str(&file_diff);
        }

        Ok(output)
    }

    fn generate_file_diff(&self, path: &str, change: &FileChange) -> anyhow::Result<String> {
        let old_lines: Vec<&str> = match &change.old_content {
            Some(c) => c.lines().collect(),
            None => Vec::new(),
        };
        let new_lines: Vec<&str> = match &change.new_content {
            Some(c) => c.lines().collect(),
            None => Vec::new(),
        };

        let mut diff = String::new();

        // File header.
        let old_path = if change.old_content.is_some() {
            format!("a/{path}")
        } else {
            "/dev/null".to_string()
        };
        let new_path = if change.new_content.is_some() {
            format!("b/{path}")
        } else {
            "/dev/null".to_string()
        };

        diff.push_str(&format!("diff --git a/{path} b/{path}\n"));
        if change.old_content.is_none() {
            diff.push_str("new file mode 100644\n");
        } else if change.new_content.is_none() {
            diff.push_str("deleted file mode 100644\n");
        }
        diff.push_str(&format!("--- {old_path}\n"));
        diff.push_str(&format!("+++ {new_path}\n"));

        // Generate hunks using a simple longest-common-subsequence diff.
        let hunks = compute_hunks(&old_lines, &new_lines, self.context_lines);
        for hunk in hunks {
            diff.push_str(&hunk);
        }

        Ok(diff)
    }
}

impl Default for PatchGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a patch.
#[derive(Debug, Clone, Default)]
pub struct PatchStats {
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub hunks: usize,
}

/// Builder for validated unified diff patches.
pub struct PatchBuilder {
    content: String,
}

impl PatchBuilder {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
        }
    }

    /// Validate that the patch is well-formed unified diff.
    pub fn validate(self) -> anyhow::Result<Self> {
        if self.content.trim().is_empty() {
            anyhow::bail!("Patch content is empty");
        }
        if !self.content.contains("---") && !self.content.contains("+++") {
            anyhow::bail!("Patch does not appear to be a valid unified diff");
        }
        if self.content.contains("<<<<<<<") {
            anyhow::bail!("Patch contains unresolved conflict markers");
        }
        Ok(self)
    }

    /// Build the final patch string.
    pub fn build(self) -> String {
        self.content
    }

    /// Extract the list of files modified by this patch.
    pub fn modified_files(&self) -> Vec<String> {
        self.content
            .lines()
            .filter_map(|line| line.strip_prefix("+++ b/").map(String::from))
            .collect()
    }

    /// Compute statistics for this patch.
    pub fn stats(&self) -> PatchStats {
        let mut stats = PatchStats::default();
        let mut seen_files = std::collections::HashSet::new();

        for line in self.content.lines() {
            if let Some(path) = line.strip_prefix("+++ b/") {
                seen_files.insert(path.to_string());
            } else if line.starts_with("+++ /dev/null") {
                // File deletion -- count is from the --- line, already tracked.
            } else if line.starts_with("@@") {
                stats.hunks += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                stats.insertions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                stats.deletions += 1;
            }
        }

        // Also count files from --- lines for deletions.
        for line in self.content.lines() {
            if let Some(path) = line.strip_prefix("--- a/") {
                seen_files.insert(path.to_string());
            }
        }

        stats.files_changed = seen_files.len();
        stats
    }
}

/// Compute unified diff hunks between old and new line slices.
///
/// Uses a simple edit-distance approach: walk both sequences, emit
/// context lines around insertions and deletions.
fn compute_hunks(old: &[&str], new: &[&str], context: usize) -> Vec<String> {
    let edits = compute_edit_script(old, new);

    if edits.is_empty() {
        return Vec::new();
    }

    // Group edits into hunks separated by more than 2*context unchanged lines.
    let mut hunks = Vec::new();
    let mut hunk_edits: Vec<&Edit> = Vec::new();
    let mut last_change_idx: Option<usize> = None;

    for (i, edit) in edits.iter().enumerate() {
        match edit {
            Edit::Equal(_) => {
                if let Some(last) = last_change_idx {
                    if i - last > context * 2 && !hunk_edits.is_empty() {
                        hunks.push(format_hunk(&hunk_edits, old, context));
                        hunk_edits.clear();
                    }
                }
                hunk_edits.push(edit);
            }
            Edit::Insert(_) | Edit::Delete(_) => {
                last_change_idx = Some(i);
                hunk_edits.push(edit);
            }
        }
    }

    if !hunk_edits.is_empty() {
        hunks.push(format_hunk(&hunk_edits, old, context));
    }

    hunks
}

#[derive(Debug)]
enum Edit<'a> {
    Equal(&'a str),
    Insert(&'a str),
    Delete(&'a str),
}

/// Simple O(NM) edit script computation.
fn compute_edit_script<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Edit<'a>> {
    let n = old.len();
    let m = new.len();

    // DP table for LCS length.
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            if old[i - 1] == new[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to produce edit script.
    let mut edits = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            edits.push(Edit::Equal(old[i - 1]));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            edits.push(Edit::Insert(new[j - 1]));
            j -= 1;
        } else {
            edits.push(Edit::Delete(old[i - 1]));
            i -= 1;
        }
    }

    edits.reverse();
    edits
}

/// Format a group of edits into a unified diff hunk string.
fn format_hunk(edits: &[&Edit<'_>], _old: &[&str], context: usize) -> String {
    // Trim leading/trailing equal lines to context size.
    let first_change = edits
        .iter()
        .position(|e| !matches!(e, Edit::Equal(_)))
        .unwrap_or(0);
    let last_change = edits
        .iter()
        .rposition(|e| !matches!(e, Edit::Equal(_)))
        .unwrap_or(edits.len().saturating_sub(1));

    let start = first_change.saturating_sub(context);
    let end = (last_change + context + 1).min(edits.len());
    let trimmed = &edits[start..end];

    // Count old and new line spans.
    let mut old_count = 0u32;
    let mut new_count = 0u32;
    for edit in trimmed {
        match edit {
            Edit::Equal(_) => {
                old_count += 1;
                new_count += 1;
            }
            Edit::Delete(_) => old_count += 1,
            Edit::Insert(_) => new_count += 1,
        }
    }

    // Compute starting line numbers (1-indexed).
    let mut old_start = 1u32;
    let mut new_start = 1u32;
    for edit in &edits[..start] {
        match edit {
            Edit::Equal(_) => {
                old_start += 1;
                new_start += 1;
            }
            Edit::Delete(_) => old_start += 1,
            Edit::Insert(_) => new_start += 1,
        }
    }

    let mut hunk = format!("@@ -{old_start},{old_count} +{new_start},{new_count} @@\n");
    for edit in trimmed {
        match edit {
            Edit::Equal(line) => {
                hunk.push(' ');
                hunk.push_str(line);
                hunk.push('\n');
            }
            Edit::Delete(line) => {
                hunk.push('-');
                hunk.push_str(line);
                hunk.push('\n');
            }
            Edit::Insert(line) => {
                hunk.push('+');
                hunk.push_str(line);
                hunk.push('\n');
            }
        }
    }

    hunk
}
