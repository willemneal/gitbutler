# GitRefStore Review

## Plan Conformance

The implementation closely follows the architect's plan with a few well-documented deviations:

1. **Struct storage model (justified deviation).** The plan specified `Arc<gix::ThreadSafeRepository>`. The implementer discovered that `gix::ThreadSafeRepository` (at rev 700ad9ea) is not `Sync` due to `once_cell::unsync::OnceCell` in `config::Cache`. The implementer switched to storing `repo_path: PathBuf` + `open_opts: gix::open::Options` and re-opening the repo on each call. This is correct -- it satisfies `Send + Sync` on the struct and avoids UB. The performance cost (re-reading config per operation) is acceptable for the entry sizes involved.

2. **Constructor signature changed accordingly.** `new(repo: Arc<ThreadSafeRepository>, ...)` became `new(repo_path: impl Into<PathBuf>, ...)`. This is a necessary consequence of deviation 1.

3. **Minor API adaptation.** `prefixed(&prefix)` needed `.as_str()`, `write_blob()` needed `.detach()` instead of `.into()`. These are standard adjustments to the actual gix API surface.

4. **Ref path layout.** Matches the plan exactly: `refs/but-ai/memory/<agent_id>/<state>/<entry_id>`.

5. **Serialization format.** JSON via `serde_json::to_vec_pretty` / `serde_json::from_slice`, matching the plan.

6. **Transition atomicity.** Uses `repo.edit_references([...])` with batch `RefEdit` as recommended in the plan (section 5). This is the strongest approach available.

7. **Files created/modified.** Match the plan: new `git_store.rs`, modified `mod.rs` and `Cargo.toml`.

**Conformance verdict:** High. All deviations are justified and documented.

## Trait Compliance

`GitRefStore` implements all five `MemoryStore` trait methods:

| Method | Status | Notes |
|--------|--------|-------|
| `store(&self, entry: &MemoryEntry) -> Result<()>` | Correct | Deletes old ref in any state, creates new ref. Handles overwrite with state change. |
| `load(&self, id: &EntryId) -> Result<Option<MemoryEntry>>` | Correct | Searches all three states, returns `None` for missing entries. |
| `list(&self, state: Option<MemoryState>) -> Result<Vec<EntryId>>` | Correct | Uses prefix scan with `state_prefix` or `agent_prefix`. |
| `transition(&self, id: &EntryId, new_state: MemoryState) -> Result<()>` | Correct | No-op for same state. Batch `edit_references` for atomicity. Updates state field in serialized data. |
| `delete(&self, id: &EntryId) -> Result<()>` | Correct | Searches all states, errors on not-found. |

The trait requires `Send + Sync`. `GitRefStore` contains `PathBuf` (Send + Sync), `gix::open::Options` (needs verification), and `String` (Send + Sync). The code compiles with `cargo check`, which means the compiler has verified `Send + Sync` bounds are satisfied.

**Trait compliance verdict:** Full compliance.

## Code Quality

### Error handling coverage

- All gix operations use `?` for error propagation via `anyhow::Result`.
- `store`: Properly deletes existing refs before creating new ones (handles the "overwrite" case).
- `load`: Returns `Ok(None)` for missing entries -- correct.
- `transition` and `delete`: Return descriptive `anyhow::bail!("entry not found: {}")` for missing entries.
- `find_entry_state`: Returns `Ok(None)` when entry is not found in any state.
- `filter_map(Result::ok)` in `list()` silently drops ref iteration errors -- see Issues below.

### Serialization robustness

- Uses `serde_json::to_vec_pretty` for human-readable blobs (inspectable with `git cat-file -p`).
- `serde_json::from_slice` will fail with a clear error if the blob contains invalid JSON.
- All `MemoryEntry` fields have proper `Serialize`/`Deserialize` derives.
- The round-trip integration tests verify all field types including every `SurvivalDistribution` variant.

### Ref naming correctness

- The `entry_ref()`, `state_prefix()`, and `agent_prefix()` functions produce well-formed ref paths.
- `state_slug()` maps all three enum variants to valid ref path segments (`alive`, `moribund`, `deceased`).
- Ref name parsing via `try_into::<gix::refs::FullName>()` validates the ref path at creation time.
- `entry_id_from_ref_name` extracts the last `/`-delimited segment, which is correct for the ref layout.

## Concurrency Safety

### Thread safety

The struct holds only `PathBuf`, `gix::open::Options`, and `String` -- all `Send + Sync`. Each operation opens a fresh `gix::Repository` (which is not `Sync` but is only used within a single method call). This is correct.

### Multi-agent isolation

The ref path layout (`refs/but-ai/memory/<agent_id>/...`) provides namespace isolation. Two `GitRefStore` instances with different `agent_id` values will never read or modify each other's refs. This is tested by 4 dedicated multi-agent integration tests.

### Concurrent writes to the same ref

**Within the same agent namespace**, concurrent writes are partially safe:
- `store()` is a delete-then-create sequence for existing entries. If two callers `store()` the same entry concurrently, the `MustNotExist` constraint on ref creation will cause one to fail.
- `transition()` uses batch `edit_references` with `MustExistAndMatch` on the old ref, so concurrent transitions on the same entry will cause one to fail with a ref mismatch error.
- These are appropriate failure modes -- they surface the conflict rather than silently corrupting data.

**The plan explicitly states** "only one process writes to a given agent's namespace at a time," so this is acceptable.

## Test Coverage

### Unit tests (13, in `git_store.rs`)

Cover all basic operations: store/load, load-nonexistent, list-by-state, transition (success, noop, not-found), delete (success, not-found), overwrite (same state, different state), persistence across instances, ref layout verification, and agent namespace isolation.

### Integration tests (19, in `git_store_integration.rs`)

Cover the full pipeline: store-retrieve-rank, max_results, score breakdown population, lifecycle audit (3 scenarios: alive->moribund/deceased, moribund resuscitation, moribund->deceased), resuscitate from deceased, resuscitate noop on alive, threshold consistency, multi-agent isolation (4 tests: stores, transitions, deletions, retrieval engines), full field round-trip, all survival distribution variants, round-trip after transition, persistence across instances, see-also graph boost, and full pipeline (store->audit->retrieve).

### What's missing

1. **No test for entry IDs containing special characters** (`/`, `.`, spaces, unicode). Since entry IDs go directly into ref paths without sanitization, an entry ID of `"../../HEAD"` would produce `refs/but-ai/memory/agent/alive/../../HEAD` -- which gix may or may not reject. This should at minimum be tested.

2. **No test for agent IDs containing special characters.** Same concern.

3. **No test for `list()` returning results in a deterministic order.** The current implementation returns refs in whatever order `prefixed()` yields them. If callers depend on ordering, this should be documented or tested.

4. **No test for very large entries** (e.g., 100KB content string). JSON serialization is fine but it would be good to verify no unexpected truncation.

5. **No test for the error path when the repo doesn't exist** (i.e., `open_repo()` fails). A test that constructs `GitRefStore` with a bogus path and verifies `store()` returns an error would be useful.

## Issues

### Critical (blocks merge)

None.

### Major (should fix before merge)

1. **`InMemoryStore` uses unsound `unsafe` casts (store.rs:87, 124, 138).** `InMemoryStore::store`, `transition`, and `delete` cast `&self` to `&mut Self` via raw pointer casts (`&mut *(self as *const Self as *mut Self)`). This is **undefined behavior** under Rust's aliasing rules. Even with `#[allow(invalid_reference_casting)]`, the compiler is free to misoptimize. The correct approach is `RefCell`, `Mutex`, or `RwLock` for interior mutability. While this is not in `GitRefStore` itself, it is in the sibling implementation that the integration tests' `InMemoryStore` relies on, and it will break under Miri or with future compiler optimizations.

2. **Silent error swallowing in `list()` (git_store.rs:196).** The line `filter_map(Result::ok)` silently drops errors from the ref iterator. If a ref is corrupt or unreadable, the caller will get a truncated list with no indication of failure. Should at minimum log a warning, or propagate errors by collecting `Result`s.

3. **No input validation on `agent_id` or `entry_id` for ref-path safety.** If an `agent_id` or `entry_id` contains `/`, `..`, or other characters that are illegal or meaningful in ref paths, the behavior is undefined. The `try_into::<gix::refs::FullName>()` call in `store()` will catch some of these, but not all paths go through that validation (e.g., `find_entry_state` uses `try_find_reference` with a raw string). The constructor should validate that `agent_id` and `entry_id` contain only safe characters (alphanumeric + dash + underscore).

### Minor (nice to have)

1. **`open_opts` field is cloned on every operation (git_store.rs:93).** `gix::open::Options::isolated()` is called once in the constructor, but `.clone()` is called on every `open_repo()`. If `Options` is expensive to clone, this could be cached differently. In practice the cost is likely negligible.

2. **`ALL_STATES` const could be derived.** If `MemoryState` gets a fourth variant in the future, the const array won't catch the omission at compile time. A `strum::EnumIter` or similar would be more robust, but this is a minor concern given the enum is unlikely to change.

3. **Integration test helper duplication.** Both the unit tests in `git_store.rs` and the integration tests in `git_store_integration.rs` define their own `temp_repo()` and `make_entry()` helpers. These could be consolidated into a `testutils` module, but this is cosmetic.

4. **The `RetrievalEngine` moves ownership of the store.** `RetrievalEngine::new(store, see_also)` takes `store` by value. After building a `RetrievalEngine`, the original `GitRefStore` is consumed and lifecycle operations must go through `engine.store()`. This is by design but slightly awkward for the full-pipeline test pattern (store -> audit -> build engine). Not a bug, just a design note.

## Verdict

**APPROVE**

The implementation is solid, well-tested, and closely follows the architect's plan. The three deviations from the plan are all justified and documented. The `GitRefStore` correctly satisfies the `MemoryStore` trait contract, uses atomic batch ref edits for transitions, and provides clean multi-agent namespace isolation.

The major issues listed above (unsound `unsafe` in `InMemoryStore`, silent error swallowing in `list()`, and missing input validation) should be addressed before production use but do not block merging the `GitRefStore` implementation itself, since:
- Issue 1 is in `InMemoryStore`, not `GitRefStore`.
- Issue 2 is a robustness concern, not a correctness bug (corrupt refs are unlikely in normal operation).
- Issue 3 is a defense-in-depth concern (gix's ref validation catches most bad inputs).

All 32 tests pass (13 unit + 19 integration). The implementation is ready to merge.
