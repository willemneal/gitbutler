# Implementer-Store: MEMORY

## Status: COMPLETE

All 13 tests pass. `cargo check -p but-ai` and `cargo test -p but-ai git_store` both succeed.

## What was implemented

- `crates/but-ai/src/memory/git_store.rs` -- `GitRefStore` struct implementing `MemoryStore` trait
- Registered module in `crates/but-ai/src/memory/mod.rs`
- Added `gix.workspace = true` to `crates/but-ai/Cargo.toml` (both `[dependencies]` and `[dev-dependencies]`)
- Added `tempfile.workspace = true` to `[dev-dependencies]`

## Deviations from architect's plan

### 1. Path-based repo opening instead of `Arc<ThreadSafeRepository>`

The architect's plan specified storing `Arc<gix::ThreadSafeRepository>` on the struct. However, `gix::ThreadSafeRepository` at this revision (0.81.0, rev 700ad9ea) is **not `Sync`** because its internal `config::Cache` contains `once_cell::unsync::OnceCell`. Since the `MemoryStore` trait requires `Send + Sync`, wrapping it in `Arc` does not help.

**Solution:** The store holds `repo_path: PathBuf` and `open_opts: gix::open::Options`, and opens the repository fresh on each operation via `gix::open_opts()`. This is slightly less efficient (re-reads config each time) but is correct and thread-safe. The `gix::open::Options::isolated()` option is used to avoid reading user/system git config.

### 2. `prefixed()` expects `&str`, not `&String`

The plan's code `repo.references()?.prefixed(&prefix)?` fails because `&String` does not implement `TryInto<&RelativePath>`. Fixed by passing `prefix.as_str()`.

### 3. `write_blob()` returns `Id<'_>`, not `ObjectId`

The plan used `.into()` on the blob write result. The compiler couldn't infer the target type. Fixed by using `.detach()` which explicitly produces `gix::ObjectId`.

### 4. Constructor signature changed

```rust
// Plan:
pub fn new(repo: Arc<gix::ThreadSafeRepository>, agent_id: impl Into<String>) -> Self
// Actual:
pub fn new(repo_path: impl Into<PathBuf>, agent_id: impl Into<String>) -> Self
```

## Tests (13 total, all passing)

1. `store_and_load` -- basic store + load roundtrip
2. `load_nonexistent_returns_none` -- load of missing entry
3. `list_by_state` -- list filtering by Alive/Moribund/Deceased/None
4. `transition_between_states` -- move Alive -> Moribund
5. `transition_noop_when_same_state` -- no-op transition
6. `transition_not_found_errors` -- error on missing entry
7. `delete_entry` -- delete + verify gone
8. `delete_not_found_errors` -- error on missing entry
9. `store_overwrites_existing` -- re-store updates content
10. `store_moves_state_on_overwrite` -- re-store changes state namespace
11. `persistence_across_store_instances` -- drop store, reopen, data persists
12. `ref_layout_matches_expected_path` -- verify exact ref path format
13. `concurrent_agent_namespaces_do_not_interfere` -- two agents, same repo, no crosstalk
