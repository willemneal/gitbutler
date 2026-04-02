# Task: Implement GitRefStore

## Background
The architect has designed a Git-ref-backed MemoryStore. Your job is to implement it.

## What to Read
- The architect's plan: `../../architect/PLAN.md`
- `crates/but-ai/src/types.rs` — the `MemoryStore` trait
- `crates/but-ai/src/memory/store.rs` — the existing `InMemoryStore` for reference

## Deliverable
Create `crates/but-ai/src/memory/git_store.rs` containing:
- `GitRefStore` struct implementing `MemoryStore`
- All 5 trait methods: store, load, list, transition, delete
- Unit tests proving each method works

## Constraints
- Must implement the `MemoryStore` trait exactly as defined
- Use `gix` for all Git operations
- Serialization format must match the architect's plan
- Must compile: `cargo check -p but-ai`
