//! CollectionAdapter trait for forge abstraction.
//!
//! In library terms, a "collection" is a repository and the forge
//! (GitHub, GitLab, etc.) is the building that houses it. The adapter
//! abstracts over forge-specific APIs, presenting a uniform interface
//! for loan management (PRs), notes (comments), and subjects (labels).

use crate::types::{CollectionRef, LoanId, LoanInfo, LoanRequest, LoanSummary, Note, NoteId};

/// The collection adapter: a forge-agnostic interface for managing
/// loans (PRs), notes (comments), and subjects (labels).
///
/// Each method is named in library terms:
/// - `create_loan` = create a pull request
/// - `post_note` = post a comment
/// - `read_notes` = read comments
/// - `add_subject` = add a label
/// - `get_loan` = get PR details
/// - `search_loans` = search PRs
pub trait CollectionAdapter: Send + Sync {
    /// Create a new loan (pull request) in the given collection.
    fn create_loan(
        &self,
        collection: &CollectionRef,
        request: &LoanRequest,
    ) -> anyhow::Result<LoanId>;

    /// Post a note (comment) on an existing loan.
    fn post_note(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
        note: &str,
    ) -> anyhow::Result<NoteId>;

    /// Read all notes (comments) on a loan.
    fn read_notes(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
    ) -> anyhow::Result<Vec<Note>>;

    /// Add a subject (label) to a loan.
    fn add_subject(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
        subject: &str,
    ) -> anyhow::Result<()>;

    /// Get detailed information about a loan.
    fn get_loan(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
    ) -> anyhow::Result<LoanInfo>;

    /// Search for loans matching a query.
    fn search_loans(
        &self,
        collection: &CollectionRef,
        query: &str,
    ) -> anyhow::Result<Vec<LoanSummary>>;
}

/// A no-op adapter for testing and WASI environments where forge
/// operations are not available.
pub struct NullAdapter;

impl CollectionAdapter for NullAdapter {
    fn create_loan(
        &self,
        _collection: &CollectionRef,
        _request: &LoanRequest,
    ) -> anyhow::Result<LoanId> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }

    fn post_note(
        &self,
        _collection: &CollectionRef,
        _loan_id: LoanId,
        _note: &str,
    ) -> anyhow::Result<NoteId> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }

    fn read_notes(
        &self,
        _collection: &CollectionRef,
        _loan_id: LoanId,
    ) -> anyhow::Result<Vec<Note>> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }

    fn add_subject(
        &self,
        _collection: &CollectionRef,
        _loan_id: LoanId,
        _subject: &str,
    ) -> anyhow::Result<()> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }

    fn get_loan(
        &self,
        _collection: &CollectionRef,
        _loan_id: LoanId,
    ) -> anyhow::Result<LoanInfo> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }

    fn search_loans(
        &self,
        _collection: &CollectionRef,
        _query: &str,
    ) -> anyhow::Result<Vec<LoanSummary>> {
        anyhow::bail!("NullAdapter: forge operations not available in this environment")
    }
}

/// An in-memory adapter for testing. Stores loans and notes in memory.
#[derive(Debug, Default)]
pub struct InMemoryAdapter {
    next_loan_id: std::sync::atomic::AtomicU64,
    next_note_id: std::sync::atomic::AtomicU64,
    loans: std::sync::Mutex<Vec<StoredLoan>>,
}

#[derive(Debug, Clone)]
struct StoredLoan {
    id: LoanId,
    collection: CollectionRef,
    info: LoanInfo,
    notes: Vec<Note>,
}

impl InMemoryAdapter {
    /// Create a new in-memory adapter.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CollectionAdapter for InMemoryAdapter {
    fn create_loan(
        &self,
        collection: &CollectionRef,
        request: &LoanRequest,
    ) -> anyhow::Result<LoanId> {
        let id = LoanId(
            self.next_loan_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1,
        );
        let info = LoanInfo {
            loan_id: id.clone(),
            collection: collection.clone(),
            title: request.title.clone(),
            state: crate::types::LoanState::Open,
            labels: Vec::new(),
        };

        let stored = StoredLoan {
            id: id.clone(),
            collection: collection.clone(),
            info,
            notes: Vec::new(),
        };

        self.loans.lock().unwrap().push(stored);
        Ok(id)
    }

    fn post_note(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
        note: &str,
    ) -> anyhow::Result<NoteId> {
        let note_id = NoteId(
            self.next_note_id
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1,
        );

        let mut loans = self.loans.lock().unwrap();
        let loan = loans
            .iter_mut()
            .find(|l| l.id == loan_id && l.collection == *collection)
            .ok_or_else(|| anyhow::anyhow!("loan not found"))?;

        loan.notes.push(Note {
            note_id: note_id.clone(),
            author: "agent".to_string(),
            body: note.to_string(),
            created_at: String::new(),
        });

        Ok(note_id)
    }

    fn read_notes(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
    ) -> anyhow::Result<Vec<Note>> {
        let loans = self.loans.lock().unwrap();
        let loan = loans
            .iter()
            .find(|l| l.id == loan_id && l.collection == *collection)
            .ok_or_else(|| anyhow::anyhow!("loan not found"))?;

        Ok(loan.notes.clone())
    }

    fn add_subject(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
        subject: &str,
    ) -> anyhow::Result<()> {
        let mut loans = self.loans.lock().unwrap();
        let loan = loans
            .iter_mut()
            .find(|l| l.id == loan_id && l.collection == *collection)
            .ok_or_else(|| anyhow::anyhow!("loan not found"))?;

        loan.info.labels.push(subject.to_string());
        Ok(())
    }

    fn get_loan(
        &self,
        collection: &CollectionRef,
        loan_id: LoanId,
    ) -> anyhow::Result<LoanInfo> {
        let loans = self.loans.lock().unwrap();
        let loan = loans
            .iter()
            .find(|l| l.id == loan_id && l.collection == *collection)
            .ok_or_else(|| anyhow::anyhow!("loan not found"))?;

        Ok(loan.info.clone())
    }

    fn search_loans(
        &self,
        collection: &CollectionRef,
        query: &str,
    ) -> anyhow::Result<Vec<LoanSummary>> {
        let loans = self.loans.lock().unwrap();
        let query_lower = query.to_lowercase();

        let results: Vec<LoanSummary> = loans
            .iter()
            .filter(|l| l.collection == *collection)
            .filter(|l| l.info.title.to_lowercase().contains(&query_lower))
            .map(|l| LoanSummary {
                loan_id: l.id.clone(),
                title: l.info.title.clone(),
                state: l.info.state,
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CallNumber;

    fn test_collection() -> CollectionRef {
        CollectionRef {
            uri: "github.com/org/repo".into(),
        }
    }

    fn test_request() -> LoanRequest {
        LoanRequest {
            title: "Add auth middleware".into(),
            body: "Implements JWT validation".into(),
            requesting_collection: None,
            call_number: CallNumber::parse("ARCH.AUTH.MIDDLEWARE"),
            reason: "Feature implementation".into(),
            due_date: None,
            see_also: Vec::new(),
        }
    }

    #[test]
    fn in_memory_create_and_read() {
        let adapter = InMemoryAdapter::new();
        let collection = test_collection();

        let loan_id = adapter.create_loan(&collection, &test_request()).unwrap();
        let info = adapter.get_loan(&collection, loan_id).unwrap();
        assert_eq!(info.title, "Add auth middleware");
    }

    #[test]
    fn in_memory_notes() {
        let adapter = InMemoryAdapter::new();
        let collection = test_collection();

        let loan_id = adapter.create_loan(&collection, &test_request()).unwrap();
        adapter
            .post_note(&collection, loan_id.clone(), "Looks good!")
            .unwrap();

        let notes = adapter.read_notes(&collection, loan_id).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].body, "Looks good!");
    }

    #[test]
    fn in_memory_search() {
        let adapter = InMemoryAdapter::new();
        let collection = test_collection();

        adapter.create_loan(&collection, &test_request()).unwrap();

        let results = adapter.search_loans(&collection, "auth").unwrap();
        assert_eq!(results.len(), 1);

        let no_results = adapter.search_loans(&collection, "database").unwrap();
        assert_eq!(no_results.len(), 0);
    }

    #[test]
    fn null_adapter_errors() {
        let adapter = NullAdapter;
        let collection = test_collection();
        assert!(adapter.create_loan(&collection, &test_request()).is_err());
    }
}
