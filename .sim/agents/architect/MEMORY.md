# Architect Agent - Memory

## Key Findings

### MemoryStore Trait
- Defined in `crates/but-ai/src/types.rs` (line 699)
- Five methods: `store`, `load`, `list`, `transition`, `delete`
- All take `&self` (not `&mut self`) -- implementors need interior mutability or external synchronization
- Returns `anyhow::Result`; `load` returns `Option<MemoryEntry>`
- `list` accepts `Option<MemoryState>` filter; `None` means all states

### MemoryEntry
- Large struct with many fields (classification, survival, narrative metadata, etc.)
- Already derives `Serialize` + `Deserialize` -- JSON serialization works out of the box
- `state: MemoryState` field on the entry itself tracks lifecycle state
- `id: EntryId` and `agent: AgentId` identify the entry

### InMemoryStore
- Uses three separate `HashMap<EntryId, MemoryEntry>` -- one per state
- Uses unsafe pointer cast to get `&mut self` from `&self` (the trait constraint)
- `GitRefStore` avoids this because gix operations naturally work through `&self`

### Lifecycle
- Three states: Alive (S(t) >= 0.25), Moribund (0.10 <= S(t) < 0.25), Deceased (S(t) < 0.10)
- `audit_lifecycle` and `resuscitate` are generic over `S: MemoryStore` -- they work with any backend

### gix Usage Patterns in GitButler
- `repo.reference(name, oid, PreviousValue, message)` creates/updates refs
- `repo.try_find_reference(name)` returns `Option<Reference>`; `reference.delete()` removes
- `repo.references()?.prefixed(prefix)` iterates refs by prefix
- `repo.edit_references([RefEdit, ...])` does atomic batch ref updates
- `repo.write_blob(&bytes)` writes an object, returns `ObjectId`
- `repo.find_object(oid)?.into_blob()` reads a blob
- Workspace Cargo.toml pins gix 0.81.0 from gitoxide rev 700ad9ea
- gix is a workspace dependency; crates add `gix.workspace = true`

### Existing REF_PREFIX
- `types.rs` defines `REF_PREFIX = "refs/but-ai"` and `memory_ref()` helper
- The helper uses a flat layout `refs/but-ai/memory/<agent>/<entry_hash>` without state in the path
- The new `GitRefStore` adds state as an intermediate path segment
- The old helper is left intact (not modified per task constraints)

### Dependencies
- `but-ai/Cargo.toml` currently has: anyhow, serde, serde_json, schemars, tracing
- Needs `gix.workspace = true` added for the new store

## Design Decisions Made
1. JSON over MessagePack: human-readable, already in deps, entries are small
2. Blob-per-entry (no tree objects): simpler, 1:1 mapping, avoids tree manipulation
3. State in ref path: enables prefix-scan listing by state
4. Agent-scoped store: `GitRefStore` carries `agent_id`, avoids needing it in every call
5. Atomic transitions via `edit_references` batch: delete old + create new in one call
6. `Arc<gix::ThreadSafeRepository>` for thread safety: `to_thread_local()` per operation
