//! ForgeAdapter implementations.
//!
//! `InMemoryForge` provides a test double for the `ForgeAdapter` trait.
//! Production implementations (GitHub, GitLab, etc.) will be separate modules.

use crate::types::{ForgeAdapter, ForgeType, PrRef, PrStatus, RepoRef};
use std::sync::Mutex;

/// A PR stored in the in-memory forge.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StoredPr {
    pr: PrRef,
    title: String,
    body: String,
    head: String,
    base: String,
    status: PrStatus,
    labels: Vec<String>,
    comments: Vec<String>,
}

/// In-memory forge adapter for testing. All state is held behind a mutex
/// so it can satisfy the `Send + Sync` requirement of `ForgeAdapter`.
pub struct InMemoryForge {
    state: Mutex<ForgeState>,
}

#[derive(Debug, Default)]
struct ForgeState {
    prs: Vec<StoredPr>,
    next_number: u64,
}

impl InMemoryForge {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ForgeState {
                prs: Vec::new(),
                next_number: 1,
            }),
        }
    }

    /// Get all stored PRs (for test assertions).
    pub fn prs(&self) -> Vec<PrRef> {
        self.state
            .lock()
            .unwrap()
            .prs
            .iter()
            .map(|p| p.pr.clone())
            .collect()
    }

    /// Get all comments on a PR (for test assertions).
    pub fn comments_for(&self, pr: &PrRef) -> Vec<String> {
        self.state
            .lock()
            .unwrap()
            .prs
            .iter()
            .find(|p| p.pr == *pr)
            .map(|p| p.comments.clone())
            .unwrap_or_default()
    }

    /// Set the status of a PR (for test setup).
    pub fn set_status(&self, pr: &PrRef, status: PrStatus) {
        let mut state = self.state.lock().unwrap();
        if let Some(stored) = state.prs.iter_mut().find(|p| p.pr == *pr) {
            stored.status = status;
        }
    }
}

impl Default for InMemoryForge {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeAdapter for InMemoryForge {
    fn create_pr(
        &self,
        repo: &RepoRef,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> anyhow::Result<PrRef> {
        let mut state = self.state.lock().unwrap();
        let number = state.next_number;
        state.next_number += 1;

        let pr = PrRef {
            repo: repo.clone(),
            number,
        };

        state.prs.push(StoredPr {
            pr: pr.clone(),
            title: title.to_string(),
            body: body.to_string(),
            head: head.to_string(),
            base: base.to_string(),
            status: PrStatus::Open,
            labels: Vec::new(),
            comments: Vec::new(),
        });

        Ok(pr)
    }

    fn comment(&self, pr: &PrRef, body: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let stored = state
            .prs
            .iter_mut()
            .find(|p| p.pr == *pr)
            .ok_or_else(|| anyhow::anyhow!("PR {} not found", pr))?;
        stored.comments.push(body.to_string());
        Ok(())
    }

    fn list_comments(&self, pr: &PrRef) -> anyhow::Result<Vec<String>> {
        let state = self.state.lock().unwrap();
        let stored = state
            .prs
            .iter()
            .find(|p| p.pr == *pr)
            .ok_or_else(|| anyhow::anyhow!("PR {} not found", pr))?;
        Ok(stored.comments.clone())
    }

    fn pr_status(&self, pr: &PrRef) -> anyhow::Result<PrStatus> {
        let state = self.state.lock().unwrap();
        let stored = state
            .prs
            .iter()
            .find(|p| p.pr == *pr)
            .ok_or_else(|| anyhow::anyhow!("PR {} not found", pr))?;
        Ok(stored.status)
    }

    fn add_label(&self, pr: &PrRef, label: &str) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        let stored = state
            .prs
            .iter_mut()
            .find(|p| p.pr == *pr)
            .ok_or_else(|| anyhow::anyhow!("PR {} not found", pr))?;
        if !stored.labels.contains(&label.to_string()) {
            stored.labels.push(label.to_string());
        }
        Ok(())
    }

    fn list_prs(&self, repo: &RepoRef, labels: &[&str]) -> anyhow::Result<Vec<PrRef>> {
        let state = self.state.lock().unwrap();
        let result = state
            .prs
            .iter()
            .filter(|p| {
                p.pr.repo == *repo
                    && (labels.is_empty()
                        || labels.iter().any(|l| p.labels.contains(&l.to_string())))
            })
            .map(|p| p.pr.clone())
            .collect();
        Ok(result)
    }

    fn forge_type(&self) -> ForgeType {
        ForgeType::GitHub
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_repo() -> RepoRef {
        RepoRef {
            forge: ForgeType::GitHub,
            owner: "test-org".to_string(),
            repo: "test-repo".to_string(),
        }
    }

    #[test]
    fn create_and_query_pr() {
        let forge = InMemoryForge::new();
        let repo = test_repo();

        let pr = forge
            .create_pr(&repo, "test PR", "body", "feat/x", "main")
            .unwrap();
        assert_eq!(pr.number, 1);
        assert_eq!(forge.pr_status(&pr).unwrap(), PrStatus::Open);
    }

    #[test]
    fn comment_round_trip() {
        let forge = InMemoryForge::new();
        let repo = test_repo();
        let pr = forge
            .create_pr(&repo, "test", "body", "feat/x", "main")
            .unwrap();

        forge.comment(&pr, "hello").unwrap();
        forge.comment(&pr, "world").unwrap();

        let comments = forge.list_comments(&pr).unwrap();
        assert_eq!(comments, vec!["hello", "world"]);
    }

    #[test]
    fn labels_and_list_prs() {
        let forge = InMemoryForge::new();
        let repo = test_repo();

        let pr1 = forge
            .create_pr(&repo, "PR 1", "", "a", "main")
            .unwrap();
        let pr2 = forge
            .create_pr(&repo, "PR 2", "", "b", "main")
            .unwrap();

        forge.add_label(&pr1, "but-ai").unwrap();

        let labeled = forge.list_prs(&repo, &["but-ai"]).unwrap();
        assert_eq!(labeled.len(), 1);
        assert_eq!(labeled[0], pr1);

        let all = forge.list_prs(&repo, &[]).unwrap();
        assert_eq!(all.len(), 2);
        let _ = pr2; // used above
    }

    #[test]
    fn set_status() {
        let forge = InMemoryForge::new();
        let repo = test_repo();
        let pr = forge
            .create_pr(&repo, "test", "", "a", "main")
            .unwrap();

        forge.set_status(&pr, PrStatus::Merged);
        assert_eq!(forge.pr_status(&pr).unwrap(), PrStatus::Merged);
    }
}
