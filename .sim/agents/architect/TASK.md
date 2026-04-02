# Task: Design Git-Ref-Backed MemoryStore

## Background
The but-ai crate currently uses `InMemoryStore` — a HashMap-based store that loses all data when the process exits. The WEAVE protocol specifies that memories should be stored in Git refs so they persist across sessions and can be shared via gossip.

## What to Read
- `crates/but-ai/src/types.rs` — the `MemoryStore` trait and `MemoryEntry` type
- `crates/but-ai/src/memory/store.rs` — the current `InMemoryStore` implementation
- `crates/but-ai/src/memory/lifecycle.rs` — how state transitions work

## Deliverable
Write `PLAN.md` in your agent directory with:

1. **Ref layout**: Exact ref paths for each state (alive/moribund/deceased)
2. **Serialization**: How MemoryEntry becomes a Git blob (JSON? MessagePack?)
3. **Operations**: How each MemoryStore method maps to git operations
4. **Listing**: How to efficiently enumerate entries by state
5. **Atomicity**: How transitions move refs atomically
6. **File structure**: Which new `.rs` files to create, what goes in each
7. **Trait signatures**: Any new helper types needed

## Constraints
- Use `gix` crate (already in workspace deps) for Git operations
- Do NOT modify the existing `MemoryStore` trait
- The new store must pass all existing tests when swapped in
