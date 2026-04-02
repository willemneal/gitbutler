# Plan: Git-Ref-Backed MemoryStore

## Overview

Replace the ephemeral `InMemoryStore` (HashMap-based) with `GitRefStore`, a
persistence layer that stores each `MemoryEntry` as a JSON blob in the Git
object database and indexes it via refs under `refs/but-ai/memory/`. The
`MemoryStore` trait is not modified; `GitRefStore` is a drop-in replacement.

---

## 1. Ref Layout

Each memory entry gets a single ref whose path encodes agent, lifecycle state,
and entry ID:

```
refs/but-ai/memory/<agent_id>/<state>/<entry_id>
```

Where `<state>` is one of `alive`, `moribund`, `deceased`.

### Examples

```
refs/but-ai/memory/architect-01/alive/e3f7a9c2
refs/but-ai/memory/architect-01/moribund/b1d4e8f0
refs/but-ai/memory/validator-02/deceased/a0c3d5e7
```

### Rationale

- **State in the path** makes listing by state a prefix scan:
  `refs/but-ai/memory/<agent>/alive/` returns all alive entries for an agent.
- **No need for a tree object per entry** -- the ref points directly to a blob
  containing the serialized `MemoryEntry`. This keeps the mapping 1:1 and avoids
  tree manipulation overhead.
- The existing `memory_ref()` helper in `types.rs` uses a flat layout without
  state. The new store will use its own ref-path functions rather than that
  helper, leaving the existing helper intact for backward compatibility.

### Ref Path Functions

```rust
/// Ref prefix for a specific agent.
fn agent_prefix(agent_id: &str) -> String {
    format!("refs/but-ai/memory/{agent_id}/")
}

/// Ref prefix for a specific agent + state.
fn state_prefix(agent_id: &str, state: MemoryState) -> String {
    format!("refs/but-ai/memory/{agent_id}/{}/", state_slug(state))
}

/// Full ref path for a specific entry.
fn entry_ref(agent_id: &str, state: MemoryState, entry_id: &str) -> String {
    format!("refs/but-ai/memory/{agent_id}/{}/{entry_id}", state_slug(state))
}

/// Map MemoryState to its ref path segment.
fn state_slug(state: MemoryState) -> &'static str {
    match state {
        MemoryState::Alive => "alive",
        MemoryState::Moribund => "moribund",
        MemoryState::Deceased => "deceased",
    }
}

/// Parse a MemoryState from its ref path segment.
fn parse_state_slug(s: &str) -> Option<MemoryState> {
    match s {
        "alive" => Some(MemoryState::Alive),
        "moribund" => Some(MemoryState::Moribund),
        "deceased" => Some(MemoryState::Deceased),
        _ => None,
    }
}
```

---

## 2. Serialization

**Format: JSON** (via `serde_json`).

- `MemoryEntry` already derives `Serialize` and `Deserialize`.
- JSON is human-readable (inspectable with `git cat-file -p`), diff-friendly,
  and the `serde_json` dependency is already in `Cargo.toml`.
- No need for MessagePack -- the entries are small (typically < 4 KB), and
  readability outweighs the ~2x size overhead vs binary formats.

### Serialization Flow

```
MemoryEntry -> serde_json::to_vec_pretty(&entry) -> &[u8] -> repo.write_blob(bytes) -> gix::ObjectId
```

### Deserialization Flow

```
ref -> peel_to_id() -> repo.find_blob(oid) -> blob.data -> serde_json::from_slice::<MemoryEntry>(data)
```

---

## 3. Operations: MemoryStore Trait Method Mapping

### 3.1 `store(&self, entry: &MemoryEntry) -> Result<()>`

1. Serialize `entry` to JSON bytes.
2. Write the blob: `repo.write_blob(&json_bytes)` -- returns an `ObjectId`.
3. Delete any existing ref for this entry (in any state) by scanning
   `refs/but-ai/memory/<entry.agent>/{alive,moribund,deceased}/<entry.id>`.
   Use `repo.try_find_reference(ref_path)` for each state; if found, call
   `reference.delete()`.
4. Create the new ref: `repo.reference(full_ref_name, blob_oid, PreviousValue::MustNotExist, "but-ai: store memory entry")`.

The scan in step 3 checks exactly 3 ref paths (one per state), so it is O(1),
not a glob/prefix scan.

### 3.2 `load(&self, id: &EntryId) -> Result<Option<MemoryEntry>>`

1. For each state in `[Alive, Moribund, Deceased]`, construct the ref path
   `refs/but-ai/memory/<agent_id>/<state>/<id>` and try
   `repo.try_find_reference(path)`.
2. On the first hit, peel to blob id: `reference.id()`.
3. Read the blob: `repo.find_object(blob_oid)?.into_blob()`.
4. Deserialize: `serde_json::from_slice::<MemoryEntry>(&blob.data)`.
5. Return `Ok(Some(entry))`. If no ref matched, return `Ok(None)`.

**Problem: agent_id is needed but the trait only passes `EntryId`.** The
`GitRefStore` must know which agent's namespace to search. This is solved by
storing the `agent_id` on the struct itself (see section 6). The store is
scoped to a single agent.

### 3.3 `list(&self, state: Option<MemoryState>) -> Result<Vec<EntryId>>`

**When `state` is `Some(s)`:**
1. Use `repo.references()?.prefixed(state_prefix(agent_id, s))`.
2. For each ref in the iterator, extract the entry ID from the ref name
   (the last path segment).
3. Collect into `Vec<EntryId>`.

**When `state` is `None`:**
1. Use `repo.references()?.prefixed(agent_prefix(agent_id))`.
2. For each ref, extract the entry ID from the last path segment.
3. Collect into `Vec<EntryId>`.

### 3.4 `transition(&self, id: &EntryId, new_state: MemoryState) -> Result<()>`

1. Find the entry's current ref by checking all three state prefixes (same as
   `load` step 1). Error if not found.
2. Parse the current state from the ref path.
3. If `current_state == new_state`, return `Ok(())` (no-op).
4. Read the blob from the current ref.
5. Deserialize the entry, update `entry.state = new_state`, re-serialize.
6. Write the updated blob: `repo.write_blob(&new_json_bytes)`.
7. Delete the old ref: `old_reference.delete()`.
8. Create the new ref in the target state namespace:
   `repo.reference(new_ref_path, new_blob_oid, PreviousValue::MustNotExist, "but-ai: transition state")`.

Steps 7-8 are **not** transactional with gix's current API for loose refs.
This is acceptable because:
- Only one process writes to a given agent's namespace at a time.
- If the process crashes between 7 and 8, the entry is temporarily missing but
  the blob is still in the object database. A future compaction/repair pass can
  detect orphaned blobs.

### 3.5 `delete(&self, id: &EntryId) -> Result<()>`

1. Find the entry's ref (same as `load` step 1). Error if not found.
2. Delete the ref: `reference.delete()`.
3. The blob remains in the object database (normal Git behavior -- it will be
   garbage-collected by `git gc` when unreachable).

---

## 4. Listing Efficiency

Listing by state is a **prefix scan** using `repo.references()?.prefixed(prefix)`.

| Operation                       | Ref prefix scanned                                  | Cost         |
|---------------------------------|-----------------------------------------------------|--------------|
| List alive entries for agent    | `refs/but-ai/memory/<agent>/alive/`                 | O(n_alive)   |
| List moribund entries for agent | `refs/but-ai/memory/<agent>/moribund/`               | O(n_moribund)|
| List all entries for agent      | `refs/but-ai/memory/<agent>/`                        | O(n_total)   |
| Load single entry               | 3 x `try_find_reference` (one per state)            | O(1)         |

With packed refs, prefix scans are binary-search based, so even with thousands
of entries the cost is logarithmic to find the start of the range, then linear
in the number of results.

---

## 5. Atomicity

### Single-ref operations (store, delete)

These are atomic at the filesystem level -- gix writes a lockfile and renames
it, so each ref update is atomic.

### Transition (move between states)

A transition is a delete-old + create-new pair. These are **not** jointly
atomic. The failure modes are:

1. **Crash after delete, before create**: Entry is temporarily lost. The blob
   persists. A repair function can scan for orphaned blobs and re-create refs.
2. **Crash before delete**: No change. Idempotent retry is safe.
3. **Create fails after delete**: Same as case 1.

**Mitigation**: For production use, implement `transition` using gix's
`edit_reference` with a `RefEdit` transaction that performs both changes in one
call. The gix crate supports batched ref edits via `Repository::edit_references`
(plural), which applies multiple `RefEdit` operations atomically:

```rust
repo.edit_references([
    // Delete old ref
    RefEdit {
        change: Change::Delete {
            expected: PreviousValue::MustExistAndMatch(Target::Object(old_blob_oid)),
            log: RefLog::AndReference,
        },
        name: old_ref_name,
        deref: false,
    },
    // Create new ref
    RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: "but-ai: transition state".into(),
            },
            expected: PreviousValue::MustNotExist,
            new: Target::Object(new_blob_oid),
        },
        name: new_ref_name,
        deref: false,
    },
])?;
```

This is the recommended approach. Implementers should use `edit_references`
(batch) for transitions.

---

## 6. File Structure

### New files to create

| File | Contents |
|------|----------|
| `crates/but-ai/src/memory/git_store.rs` | `GitRefStore` struct and `MemoryStore` impl |

### Files to modify

| File | Change |
|------|--------|
| `crates/but-ai/src/memory/mod.rs` | Add `pub mod git_store;` |
| `crates/but-ai/Cargo.toml` | Add `gix.workspace = true` to `[dependencies]` |

### No changes to

- `crates/but-ai/src/types.rs` -- trait is unchanged
- `crates/but-ai/src/memory/store.rs` -- `InMemoryStore` remains as-is
- `crates/but-ai/src/memory/lifecycle.rs` -- works with any `MemoryStore`

---

## 7. Struct and Function Signatures

### `GitRefStore`

```rust
use gix::Repository;
use std::sync::Arc;

/// Git-ref-backed implementation of `MemoryStore`.
///
/// Each `MemoryEntry` is stored as a JSON blob in the Git object database,
/// indexed by a ref under `refs/but-ai/memory/<agent_id>/<state>/<entry_id>`.
///
/// The store is scoped to a single agent -- all operations work within
/// that agent's ref namespace.
pub struct GitRefStore {
    /// The gix repository handle.
    repo: Arc<gix::ThreadSafeRepository>,
    /// The agent ID this store is scoped to.
    agent_id: String,
}
```

### Constructor

```rust
impl GitRefStore {
    /// Create a new `GitRefStore` for the given agent.
    ///
    /// # Arguments
    /// * `repo` - A thread-safe gix repository handle.
    /// * `agent_id` - The agent whose memory namespace to use.
    pub fn new(repo: Arc<gix::ThreadSafeRepository>, agent_id: impl Into<String>) -> Self {
        Self {
            repo,
            agent_id: agent_id.into(),
        }
    }
}
```

### Internal helpers

```rust
impl GitRefStore {
    /// Get a local (non-thread-safe) repo handle for the current operation.
    fn local_repo(&self) -> anyhow::Result<gix::Repository> {
        Ok(self.repo.to_thread_local())
    }

    /// Construct the full ref path for an entry in a given state.
    fn ref_path(&self, state: MemoryState, entry_id: &EntryId) -> String {
        entry_ref(&self.agent_id, state, &entry_id.0)
    }

    /// Construct the ref prefix for listing entries in a given state.
    fn state_ref_prefix(&self, state: MemoryState) -> String {
        state_prefix(&self.agent_id, state)
    }

    /// Construct the ref prefix for listing all entries.
    fn agent_ref_prefix(&self) -> String {
        agent_prefix(&self.agent_id)
    }

    /// Serialize a MemoryEntry to JSON bytes.
    fn serialize(entry: &MemoryEntry) -> anyhow::Result<Vec<u8>> {
        serde_json::to_vec_pretty(entry).map_err(Into::into)
    }

    /// Deserialize a MemoryEntry from a blob's bytes.
    fn deserialize(data: &[u8]) -> anyhow::Result<MemoryEntry> {
        serde_json::from_slice(data).map_err(Into::into)
    }

    /// Find the ref for an entry, searching all three states.
    /// Returns (reference, current_state) or None.
    fn find_entry_ref(
        &self,
        repo: &gix::Repository,
        entry_id: &EntryId,
    ) -> anyhow::Result<Option<(gix::Reference<'_>, MemoryState)>> {
        // NOTE: The lifetime of the returned Reference is tied to `repo`.
        // The caller must not drop `repo` before using the reference.
        // Implementation will need to work around this -- see note below.
    }

    /// Read and deserialize the entry that a ref points to.
    fn read_entry_from_ref(
        &self,
        repo: &gix::Repository,
        reference: &gix::Reference<'_>,
    ) -> anyhow::Result<MemoryEntry> {
        let blob_id = reference.id();
        let blob = repo.find_object(blob_id)?.into_blob();
        Self::deserialize(&blob.data)
    }

    /// Extract the entry ID from the last segment of a ref name.
    fn entry_id_from_ref_name(ref_name: &gix::refs::FullNameRef) -> Option<EntryId> {
        let name = ref_name.as_bstr().to_str().ok()?;
        let last_segment = name.rsplit('/').next()?;
        Some(EntryId(last_segment.to_string()))
    }
}
```

### Lifetime note on `find_entry_ref`

The gix `Reference` type borrows from the `Repository`. Since our trait methods
take `&self`, we cannot return a reference that borrows from a local variable.
Instead, the implementation should inline the search logic in each method that
needs it, or use an alternative pattern:

```rust
/// Find which state an entry is in, without returning the Reference.
fn find_entry_state(
    &self,
    repo: &gix::Repository,
    entry_id: &EntryId,
) -> anyhow::Result<Option<MemoryState>> {
    for state in [MemoryState::Alive, MemoryState::Moribund, MemoryState::Deceased] {
        let ref_path = self.ref_path(state, entry_id);
        if repo.try_find_reference(&ref_path)?.is_some() {
            return Ok(Some(state));
        }
    }
    Ok(None)
}
```

### `MemoryStore` implementation

```rust
impl MemoryStore for GitRefStore {
    fn store(&self, entry: &MemoryEntry) -> anyhow::Result<()> {
        let repo = self.local_repo()?;
        let json_bytes = Self::serialize(entry)?;
        let blob_oid = repo.write_blob(&json_bytes)?;

        // Delete any existing ref for this entry (in any state).
        for state in [MemoryState::Alive, MemoryState::Moribund, MemoryState::Deceased] {
            let ref_path = self.ref_path(state, &entry.id);
            if let Some(reference) = repo.try_find_reference(&ref_path)? {
                reference.delete()?;
            }
        }

        // Create the ref in the correct state namespace.
        let ref_path = self.ref_path(entry.state, &entry.id);
        let ref_name: gix::refs::FullName = ref_path.as_str().try_into()?;
        repo.reference(
            ref_name,
            blob_oid,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "but-ai: store memory entry",
        )?;

        Ok(())
    }

    fn load(&self, id: &EntryId) -> anyhow::Result<Option<MemoryEntry>> {
        let repo = self.local_repo()?;
        for state in [MemoryState::Alive, MemoryState::Moribund, MemoryState::Deceased] {
            let ref_path = self.ref_path(state, id);
            if let Some(reference) = repo.try_find_reference(&ref_path)? {
                let blob_oid = reference.id();
                let object = repo.find_object(blob_oid)?;
                let entry = Self::deserialize(&object.data)?;
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    fn list(&self, state: Option<MemoryState>) -> anyhow::Result<Vec<EntryId>> {
        let repo = self.local_repo()?;
        let prefix = match state {
            Some(s) => self.state_ref_prefix(s),
            None => self.agent_ref_prefix(),
        };
        let mut ids = Vec::new();
        for reference in repo.references()?.prefixed(&prefix)?.filter_map(Result::ok) {
            if let Some(id) = Self::entry_id_from_ref_name(reference.name()) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    fn transition(&self, id: &EntryId, new_state: MemoryState) -> anyhow::Result<()> {
        let repo = self.local_repo()?;

        // Find current state.
        let current_state = self
            .find_entry_state(&repo, id)?
            .ok_or_else(|| anyhow::anyhow!("entry not found: {}", id))?;

        if current_state == new_state {
            return Ok(());
        }

        // Read, update, re-serialize.
        let old_ref_path = self.ref_path(current_state, id);
        let old_ref = repo.find_reference(&old_ref_path)?;
        let old_blob_oid = old_ref.id();
        let object = repo.find_object(old_blob_oid)?;
        let mut entry: MemoryEntry = Self::deserialize(&object.data)?;
        entry.state = new_state;
        let new_json = Self::serialize(&entry)?;
        let new_blob_oid = repo.write_blob(&new_json)?;

        // Atomic batch: delete old ref + create new ref.
        let new_ref_path = self.ref_path(new_state, id);
        let old_ref_name: gix::refs::FullName = old_ref_path.as_str().try_into()?;
        let new_ref_name: gix::refs::FullName = new_ref_path.as_str().try_into()?;

        use gix::refs::transaction::*;
        repo.edit_references([
            RefEdit {
                change: Change::Delete {
                    expected: PreviousValue::MustExistAndMatch(
                        gix::refs::Target::Object(old_blob_oid.detach()),
                    ),
                    log: RefLog::AndReference,
                },
                name: old_ref_name,
                deref: false,
            },
            RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: format!("but-ai: transition {} -> {}",
                            state_slug(current_state), state_slug(new_state)).into(),
                    },
                    expected: PreviousValue::MustNotExist,
                    new: gix::refs::Target::Object(new_blob_oid.detach()),
                },
                name: new_ref_name,
                deref: false,
            },
        ])?;

        Ok(())
    }

    fn delete(&self, id: &EntryId) -> anyhow::Result<()> {
        let repo = self.local_repo()?;
        for state in [MemoryState::Alive, MemoryState::Moribund, MemoryState::Deceased] {
            let ref_path = self.ref_path(state, id);
            if let Some(reference) = repo.try_find_reference(&ref_path)? {
                reference.delete()?;
                return Ok(());
            }
        }
        anyhow::bail!("entry not found: {}", id)
    }
}
```

---

## 8. Testing Strategy

The new `GitRefStore` must pass the **exact same tests** as `InMemoryStore`.
The recommended approach:

1. **Extract test logic into a generic test function** parameterized over
   `impl MemoryStore`. Each test calls a helper that creates the store, runs
   the assertions, and returns.

2. **For `GitRefStore` tests**, create a temporary directory with
   `gix::init::open()` (or `gix_testtools`) to get a real Git repo, then
   construct a `GitRefStore` against it.

3. **Add `GitRefStore`-specific tests** for:
   - Persistence: store, drop the `GitRefStore`, create a new one against the
     same repo, verify `load` returns the entry.
   - Ref layout: after `store`, verify the ref exists at the expected path.
   - Transition atomicity: verify both old ref is gone and new ref exists.
   - Concurrent agent namespaces: two `GitRefStore` instances with different
     `agent_id` values don't interfere.

### Test file

Add tests in `crates/but-ai/src/memory/git_store.rs` as a `#[cfg(test)] mod tests`
block, following the pattern used in `store.rs`.

---

## 9. gix Feature Requirements

The workspace `Cargo.toml` already configures gix with the `sha1` feature.
The operations needed (blob writing, reference CRUD, prefix iteration) are
all part of gix's core API and do not require additional features beyond
what is already enabled.

The `but-ai/Cargo.toml` needs:
```toml
[dependencies]
gix.workspace = true
```

---

## 10. Migration Path

No migration is needed. The `InMemoryStore` and `GitRefStore` implement the
same `MemoryStore` trait. Callers choose which to instantiate:

- Tests and quick experiments: `InMemoryStore::new()`
- Production / persistent sessions: `GitRefStore::new(repo, agent_id)`

The `lifecycle.rs` module and all retrieval code work unchanged with either
backend.
