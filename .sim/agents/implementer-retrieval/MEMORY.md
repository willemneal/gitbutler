# Implementer-Retrieval: Memory

## Status
Complete. All 19 integration tests pass.

## What was done
Created `crates/but-ai/tests/git_store_integration.rs` with 19 integration tests covering:

### Test categories
1. **Store -> Retrieve -> Ranking (3 tests):** Entries stored via GitRefStore are retrievable through RetrievalEngine with correct ranking (auth entries rank above db entries for "authentication" query), max_results is respected, and ScoreBreakdown fields are populated correctly.

2. **Lifecycle audit (6 tests):** `audit_lifecycle()` correctly transitions alive entries with low survival probability to moribund (sp < 0.25) or deceased (sp < 0.10), resuscitates recovered moribund entries (sp >= 0.25) back to alive, demotes moribund to deceased, and the `resuscitate()` function works for deceased entries. Threshold constants are verified consistent.

3. **Multi-agent isolation (4 tests):** Two GitRefStores with different agent_ids in the same repo maintain complete isolation -- same entry IDs do not collide, transitions in one agent's namespace do not affect the other, deletions are scoped, and RetrievalEngines built on different stores only see their own entries.

4. **Round-trip field preservation (3 tests):** All MemoryEntry fields survive a store-load round-trip including classification, see_also links, motifs, tension_refs, survival metadata, provenance fields, and source_commit. All four SurvivalDistribution variants (Exponential, Weibull, Bathtub, LogNormal) round-trip correctly. Fields are preserved through state transitions.

5. **Persistence (1 test):** Entries survive store instance drop and reopen against the same repo.

6. **See-also graph retrieval (1 test):** Linked entries in the SeeAlsoGraph get boosted in retrieval ranking.

7. **Full pipeline (1 test):** store -> lifecycle audit -> retrieve demonstrates the complete flow with correct audit transitions and ranking of surviving entries.

## Key findings
- GitRefStore correctly implements MemoryStore trait -- all lifecycle and retrieval code works unchanged with Git-backed storage.
- The `gix::init()` + `gix::open_opts()` with `Options::isolated()` works well for test repos in tempdir.
- Multi-agent namespace isolation is solid -- the ref path encoding `refs/but-ai/memory/<agent_id>/<state>/<entry_id>` provides clean separation.
- Transition preserves all non-state fields correctly (the implementation re-serializes the full entry with only the state field updated).

## Branch
`sim/implementer-retrieval` -- commit `8cce042c73`
