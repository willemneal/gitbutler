# Task: Integration Tests for GitRefStore + RetrievalEngine

## Background
The store implementer is building `GitRefStore`. Your job is to write integration tests proving the full pipeline works: store entries via GitRefStore, retrieve them via RetrievalEngine, verify scoring and lifecycle transitions work correctly.

## What to Read
- The architect's plan: `../../architect/PLAN.md`
- `crates/but-ai/src/memory/retrieval.rs` — the `RetrievalEngine`
- `crates/but-ai/src/memory/lifecycle.rs` — lifecycle audit
- `crates/but-ai/src/memory/git_store.rs` — the new store (created by implementer-store)

## Deliverable
Create `crates/but-ai/tests/git_store_integration.rs` containing:
- End-to-end tests: create entries → classify → store → retrieve → verify ranking
- Lifecycle tests: store → age → audit transitions → verify state changes
- Multi-agent tests: two stores in same repo, verify they see each other's refs

## Constraints
- Tests must use a temporary Git repo (tempfile crate)
- Must compile and pass: `cargo test -p but-ai --test git_store_integration`
