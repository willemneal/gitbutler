# The Case of the Forged Credentials

*A WEAVE Protocol Mystery in 5 Acts -- Sequel*

---

It was three weeks since I'd closed the Dying Memories case. The city had been
quiet -- too quiet. Agents filing their reports, memories classified and shelved,
gossip engines humming along like well-oiled machines. I should have known it
wouldn't last.

"Detective." The call came at 0800 UTC, sharp as a freshly compiled binary.
"We've got a situation in the identity bureau. An agent claiming to be an
Implementer has been committing patches to branches it shouldn't touch. The
signing key doesn't match, the catalog entries are miscategorized, and the
dependency graph..." She paused. "Someone has been sabotaging the pipeline."

I put down my coffee and reached for my toolkit. This was no accidental death.
This was premeditated forgery.

---

## Your Case File

Create a new test file at `crates/but-ai/tests/tutorial_forged_credentials.rs`.
This is your evidence ledger. Every test you write is a clue. When all tests
pass, the case is solved.

Start with the imports and helper functions:

```rust
//! The Case of the Forged Credentials -- a WEAVE Protocol mystery in 5 acts.
//!
//! Sequel to "The Case of the Dying Memories."
//! Run with: `cargo test -p but-ai --test tutorial_forged_credentials`

use but_ai::agent::phase_gate::{is_tool_allowed, phases_for_tool, tools_for_phase};
use but_ai::agent::{Coordinator, PatchOutput};
use but_ai::coordination::dependency::{DependencyGraph, DependencyNode};
use but_ai::coordination::forge::InMemoryForge;
use but_ai::coordination::messages;
use but_ai::identity::authorization::{
    can_produce_patches, check_branch_authorization, check_call_number_authorization,
    check_full_authorization, check_patch_size,
};
use but_ai::identity::key_lifecycle::{KeyManager, KeyStatus};
use but_ai::identity::performance::{
    has_sufficient_track_record, record_task_completion, reliability_score,
};
use but_ai::identity::signing::{DenyAllSigner, KeyAuditLog, KeyLifecycleEvent, NoOpSigner};
use but_ai::memory::call_number::{call_number_from_path, call_number_proximity};
use but_ai::memory::classification::{auto_classify, classify_by_path, reclassify};
use but_ai::memory::compaction::{compact, estimate_tokens, HIGH_CIRCULATION_THRESHOLD};
use but_ai::memory::controlled_vocab::VocabularyIndex;
use but_ai::memory::store::InMemoryStore;
use but_ai::types::*;
use but_ai::validation::continuity::{ContinuityChecker, ObservationSeverity};
use but_ai::validation::contradiction::detect_contradictions;
use but_ai::validation::integrity::{IntegrityChecker, ViolationKind};
```

Every detective needs a way to reconstruct the scene. These helpers build
identities and memories for your tests:

```rust
/// Build an agent identity with specific role and authorization scope.
fn make_identity(
    agent: &str,
    role: AgentRole,
    branch_patterns: &[&str],
    max_patch_lines: Option<u32>,
    call_number_ranges: &[&str],
) -> AgentIdentity {
    AgentIdentity {
        agent_id: AgentId(agent.into()),
        role,
        capabilities: vec!["code".into()],
        authorization: AuthorizationScope {
            branch_patterns: branch_patterns.iter().map(|s| s.to_string()).collect(),
            max_patch_lines,
            repos: vec!["*".to_string()],
            call_number_ranges: call_number_ranges.iter().map(|s| s.to_string()).collect(),
        },
        signing_key: Some(format!("key-{agent}")),
        performance_history: PerformanceHistory::default(),
        created_at: "2026-03-29T00:00:00Z".into(),
    }
}

/// Build a memory entry with subject headings and call number.
fn make_entry(id: &str, content: &str, subjects: &[&str], call_number: &str) -> MemoryEntry {
    MemoryEntry {
        id: EntryId(id.into()),
        agent: AgentId("detective".into()),
        content: content.into(),
        created_at: "2026-03-29T00:00:00Z".into(),
        last_accessed: "2026-03-29T00:00:00Z".into(),
        classification: Classification {
            subject_headings: subjects.iter().map(|s| s.to_string()).collect(),
            call_number: CallNumber::parse(call_number),
            controlled_vocab: false,
        },
        see_also: Vec::new(),
        motifs: Vec::new(),
        tension_refs: Vec::new(),
        survival: SurvivalMetadata {
            distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
            current_probability: 0.95,
            hazard_rate: 0.01,
            surprise_index: 0.0,
            goodness_of_fit: 0.95,
        },
        state: MemoryState::Alive,
        consensus_citations: 0,
        access_count: 1,
        source_commit: None,
    }
}

/// Build a memory entry tuned for compaction tests (specific access count).
fn make_compaction_entry(id: &str, content: &str, access_count: u64) -> MemoryEntry {
    let mut entry = make_entry(id, content, &["testing"], "TEST.UNIT");
    entry.access_count = access_count;
    entry
}
```

Process the evidence so far:

```sh
cargo test -p but-ai --test tutorial_forged_credentials
```

Nothing to test yet. The real work begins now.

---

## Act 1: The Impostor

*Scene: The ID Bureau (`identity/` module)*

The suspect's badge said "Implementer." The branch logs said `main`. Two things
that should never appear in the same sentence. Implementers get `feat/*` and
`fix/*`, not `main`. Someone had either faked their credentials or bypassed the
authorization system entirely.

I pulled the suspect's identity file and started checking.

### Clue 1: Branch Pattern Authorization

An authorization scope defines which branches an agent may touch. The scope uses
glob patterns -- `feat/*` matches `feat/new-thing` but not `main`.

The `check_branch_authorization` function signature:

```rust
pub fn check_branch_authorization(
    scope: &AuthorizationScope,
    branch: &str,
) -> AuthorizationResult
```

It returns an `AuthorizationResult` with an `authorized` field (bool) and an
optional `denial_reason`. Let's verify the rules:

```rust
#[test]
fn act1_branch_authorization_respects_globs() {
    let scope = AuthorizationScope {
        branch_patterns: vec!["feat/*".to_string(), "fix/*".to_string()],
        max_patch_lines: Some(500),
        repos: vec!["*".to_string()],
        call_number_ranges: vec![],
    };

    // Feature and fix branches: authorized.
    assert!(
        check_branch_authorization(&scope, "feat/new-auth").authorized,
        "feat/* should match feat/new-auth"
    );
    assert!(
        check_branch_authorization(&scope, "fix/bug-42").authorized,
        "fix/* should match fix/bug-42"
    );

    // Main branch: DENIED.
    let result = check_branch_authorization(&scope, "main");
    assert!(
        !result.authorized,
        "An Implementer touching main? Not on my watch."
    );
    assert!(
        result.denial_reason.is_some(),
        "Denied requests must state a reason."
    );
}
```

The glob matching is simple: `*` at the end of a pattern matches any suffix.
`feat/*` matches `feat/anything`. Exact strings match exactly. No regex, no
complications -- just enough to enforce lane discipline.

### Clue 2: Role-Based Patch Production

Not every role is allowed to produce patches. The `can_produce_patches` function
signature:

```rust
pub fn can_produce_patches(role: AgentRole) -> bool
```

Only `Implementer` and `Architect` roles may produce patches. A `Validator`
reads and checks -- it does not write:

```rust
#[test]
fn act1_only_implementers_and_architects_produce_patches() {
    assert!(
        can_produce_patches(AgentRole::Implementer),
        "Implementers produce patches -- that's their whole job."
    );
    assert!(
        can_produce_patches(AgentRole::Architect),
        "Architects produce patches too -- structural changes."
    );
    assert!(
        !can_produce_patches(AgentRole::Validator),
        "Validators read. They do not write."
    );
    assert!(
        !can_produce_patches(AgentRole::Coordinator),
        "Coordinators coordinate. They do not write code."
    );
}
```

### Clue 3: The Size Limit

The suspect's patch was 600 lines. The authorization scope says 500. The
`check_patch_size` function catches exactly this:

```rust
#[test]
fn act1_patch_size_enforcement() {
    let scope = AuthorizationScope {
        branch_patterns: vec!["feat/*".to_string()],
        max_patch_lines: Some(500),
        repos: vec!["*".to_string()],
        call_number_ranges: vec![],
    };

    // 500 lines: exactly at the limit -- allowed.
    assert!(
        check_patch_size(&scope, 500).authorized,
        "500 lines is within the 500-line limit."
    );

    // 501 lines: one too many.
    let result = check_patch_size(&scope, 501);
    assert!(
        !result.authorized,
        "501 lines exceeds the 500-line limit."
    );
    assert!(
        result.denial_reason.unwrap().contains("501"),
        "Denial reason should mention the actual patch size."
    );

    // No limit set: anything goes.
    let unlimited = AuthorizationScope {
        branch_patterns: vec!["*".to_string()],
        max_patch_lines: None,
        repos: vec!["*".to_string()],
        call_number_ranges: vec![],
    };
    assert!(
        check_patch_size(&unlimited, 10_000).authorized,
        "No limit means no restriction."
    );
}
```

### Clue 4: Call Number Range Authorization

Security agents should only touch security code. The `check_call_number_authorization`
function enforces this with glob patterns on call numbers:

```rust
#[test]
fn act1_call_number_range_authorization() {
    let scope = AuthorizationScope {
        branch_patterns: vec!["feat/*".to_string()],
        max_patch_lines: None,
        repos: vec!["*".to_string()],
        call_number_ranges: vec!["SEC.*".to_string()],
    };

    // Security code: authorized.
    assert!(
        check_call_number_authorization(&scope, "SEC.AUTH").authorized,
        "SEC.* matches SEC.AUTH"
    );
    assert!(
        check_call_number_authorization(&scope, "SEC.CRYPTO.HASHING").authorized,
        "SEC.* matches SEC.CRYPTO.HASHING"
    );

    // Database code: DENIED.
    assert!(
        !check_call_number_authorization(&scope, "DB.MIGRATE").authorized,
        "SEC.* does not match DB.MIGRATE -- stay in your lane."
    );

    // Empty ranges mean unrestricted access.
    let unrestricted = AuthorizationScope {
        branch_patterns: vec!["*".to_string()],
        max_patch_lines: None,
        repos: vec!["*".to_string()],
        call_number_ranges: vec![],
    };
    assert!(
        check_call_number_authorization(&unrestricted, "ANYTHING.GOES").authorized,
        "Empty call_number_ranges means no restriction."
    );
}
```

### Clue 5: The Full Authorization Gauntlet

The `check_full_authorization` function runs all checks together -- branch, repo,
and patch size. Its signature:

```rust
pub fn check_full_authorization(
    identity: &AgentIdentity,
    branch: &str,
    repo: &str,
    patch_lines: u32,
) -> AuthorizationResult
```

The impostor fails every check:

```rust
#[test]
fn act1_the_impostor_revealed() {
    // The suspect: claims to be an Implementer, but is actually a Validator.
    let impostor = make_identity(
        "suspect-x",
        AgentRole::Validator,        // THE REVEAL: Validator, not Implementer
        &["feat/*", "fix/*"],
        Some(500),
        &["SEC.*"],
    );

    // First: Validators cannot produce patches at all.
    assert!(
        !can_produce_patches(impostor.role),
        "THE REVEAL: The suspect is a Validator masquerading as an Implementer!"
    );

    // Second: even if they could, the branch is wrong.
    let result = check_full_authorization(&impostor, "main", "gitbutler", 100);
    assert!(
        !result.authorized,
        "The impostor tried to push to main -- denied."
    );

    // On a valid branch with valid size: passes (authorization scope is fine).
    let result = check_full_authorization(&impostor, "feat/auth", "gitbutler", 100);
    assert!(
        result.authorized,
        "The scope itself is valid -- the role is the problem."
    );
}
```

The authorization system checks what the agent *may* do. The role system checks
what the agent *is allowed* to do. The impostor's scope was legitimate -- they
had valid branch patterns and patch limits. But a Validator cannot produce
patches, period. The identity was forged at the role level.

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- act1
```

Five tests pass. The impostor is exposed. But the damage has already been done --
the catalog is corrupted.

---

## Act 2: The Tampered Catalog

*Scene: The Library Annex (`memory/classification` + `memory/controlled_vocab` modules)*

Someone had been misclassifying memories. A critical entry about authentication
vulnerabilities was filed under "documentation." Another about JWT token handling
was categorized as "database." The shelves were wrong. The card catalog was wrong.
And every search that relied on them was returning garbage.

### Clue 6: Auto-Classification From Content

The `auto_classify` function enriches an entry's subject headings by extracting
keywords from its content. Its signature:

```rust
pub fn auto_classify(entry: &mut MemoryEntry, max_subjects: usize)
```

It extracts keywords, deduplicates against existing headings (case-insensitive),
and caps at `max_subjects`:

```rust
#[test]
fn act2_auto_classify_enriches_headings() {
    let mut entry = make_entry(
        "vuln-report",
        "authentication vulnerability in JWT token validation middleware",
        &["security"],  // Only one heading -- woefully incomplete.
        "SEC.AUTH",
    );

    // Before: just "security".
    assert_eq!(entry.classification.subject_headings.len(), 1);

    auto_classify(&mut entry, 5);

    // After: enriched with keywords from content.
    assert!(
        entry.classification.subject_headings.len() > 1,
        "Auto-classification should add keywords. Got: {:?}",
        entry.classification.subject_headings
    );

    // "security" should not be duplicated.
    let security_count = entry
        .classification
        .subject_headings
        .iter()
        .filter(|s| s.eq_ignore_ascii_case("security"))
        .count();
    assert_eq!(security_count, 1, "No duplicates -- case-insensitive dedup.");

    // Max subjects honored.
    assert!(
        entry.classification.subject_headings.len() <= 5,
        "Should not exceed max_subjects=5."
    );
}
```

### Clue 7: Call Numbers From File Paths

The `call_number_from_path` function maps directory structure to call number
segments. This is how the catalog knows *where* in the knowledge tree a memory
belongs:

```rust
#[test]
fn act2_call_number_from_path() {
    // crates/but-ai/src/memory/store.rs -> CRATES.BUT-AI.MEMORY.STORE
    // (src is filtered out, .rs extension stripped)
    let cn = call_number_from_path("crates/but-ai/src/memory/store.rs", 5);
    assert_eq!(
        cn.to_string(),
        "CRATES.BUT-AI.MEMORY.STORE",
        "Path segments become call number hierarchy."
    );

    // max_depth truncates deep paths.
    let cn_shallow = call_number_from_path("crates/but-ai/src/memory/store.rs", 2);
    assert_eq!(cn_shallow.depth(), 2, "max_depth=2 limits to 2 segments.");

    // classify_by_path assigns a call number to an entry.
    let mut entry = make_entry("test", "content", &[], "PLACEHOLDER");
    classify_by_path(&mut entry, "crates/but-ai/src/identity/signing.rs", 5);
    assert_eq!(
        entry.classification.call_number.to_string(),
        "CRATES.BUT-AI.IDENTITY.SIGNING",
    );
}
```

### Clue 8: The Controlled Vocabulary

Without a controlled vocabulary, the same concept gets classified under
"authentication", "auth", "login", "sign-in", and "access control." Searches for
any one term miss the others. The `VocabularyIndex` maps variants to canonical
forms:

```rust
#[test]
fn act2_controlled_vocabulary_normalization() {
    let vocab = VocabularyIndex::with_defaults();

    // Variants resolve to canonical forms.
    assert_eq!(vocab.normalize("auth"), "authentication");
    assert_eq!(vocab.normalize("authn"), "authentication");
    assert_eq!(vocab.normalize("db"), "database");
    assert_eq!(vocab.normalize("jwt"), "json_web_token");

    // Unknown terms pass through unchanged (lowercased).
    assert_eq!(vocab.normalize("xyzzy"), "xyzzy");

    // Subject normalization deduplicates after canonicalization.
    let subjects = vec![
        "auth".into(),
        "authentication".into(),
        "authn".into(),
    ];
    let normalized = vocab.normalize_subjects(&subjects);
    assert_eq!(
        normalized.len(), 1,
        "Three synonyms collapse to one canonical form."
    );
    assert_eq!(normalized[0], "authentication");
}
```

### Clue 9: Reclassification Fixes the Damage

The `reclassify` function can fix tampered entries by replacing their subject
headings and call numbers:

```rust
#[test]
fn act2_reclassification_fixes_tampered_entries() {
    // The tampered entry: security content filed under "documentation."
    let mut tampered = make_entry(
        "vuln-report",
        "critical authentication vulnerability in JWT handling",
        &["documentation"],  // WRONG -- this is security content!
        "DOC.README",        // WRONG -- this should be SEC.AUTH!
    );

    // The fix: reclassify with correct metadata.
    reclassify(
        &mut tampered,
        Some(vec!["security".into(), "authentication".into(), "vulnerability".into()]),
        Some(CallNumber::parse("SEC.AUTH.JWT")),
        5,
    );

    assert_eq!(
        tampered.classification.subject_headings,
        vec!["security", "authentication", "vulnerability"],
    );
    assert_eq!(
        tampered.classification.call_number.to_string(),
        "SEC.AUTH.JWT",
        "Call number corrected from DOC.README to SEC.AUTH.JWT."
    );
}
```

### Clue 10: The Missing Terms

The controlled vocabulary was missing key security terms. That's why "vulnerability"
wasn't being normalized properly. Adding custom mappings fixes this:

```rust
#[test]
fn act2_vocabulary_can_be_extended() {
    let mut vocab = VocabularyIndex::with_defaults();

    // THE REVEAL: The default vocabulary doesn't include "vuln" -> "vulnerability".
    assert_eq!(
        vocab.normalize("vuln"), "vuln",
        "Without the mapping, 'vuln' passes through unchanged."
    );

    // Add the missing mapping.
    vocab.add_mapping("vuln", "vulnerability");
    vocab.add_mapping("cve", "vulnerability");

    assert_eq!(vocab.normalize("vuln"), "vulnerability");
    assert_eq!(vocab.normalize("cve"), "vulnerability");

    // Query expansion now includes all variants.
    let expanded = vocab.expand_query(&["vuln".into()]);
    assert!(expanded.contains(&"vulnerability".to_string()));
    assert!(expanded.contains(&"cve".to_string()));
}
```

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- act2
```

Five tests pass. The catalog is repaired. But there's more damage to assess --
someone has been shredding evidence.

---

## Act 3: The Shredded Evidence

*Scene: The Paper Shredder (`memory/compaction` + `identity/key_lifecycle` modules)*

High-value memories were being compacted too aggressively. Critical architectural
decisions -- the kind that get accessed ten times a week -- were being reduced to
metadata-only stubs. Meanwhile, entries that nobody had ever read were preserved
in full. The compaction thresholds were backwards.

### Clue 11: Compaction Tiers

The compaction system groups entries by circulation frequency. The threshold
constant `HIGH_CIRCULATION_THRESHOLD` is 5:

- **High** (>5 accesses): preserved in full
- **Medium** (1-5 accesses): content summarized to first 100 chars, metadata kept
- **Low** (0 accesses): metadata-only (call number + subject headings)

```rust
#[test]
fn act3_compaction_tiers_by_circulation() {
    let store = InMemoryStore::new();

    // High-circulation: architectural decision accessed 10 times.
    store.store(&make_compaction_entry(
        "arch-decision", "Use event sourcing for the audit trail", 10,
    )).unwrap();

    // Medium-circulation: implementation note accessed 3 times.
    store.store(&make_compaction_entry(
        "impl-note", "Handler uses async middleware pattern", 3,
    )).unwrap();

    // Low-circulation: never accessed.
    store.store(&make_compaction_entry(
        "dead-letter", "Placeholder entry for future work", 0,
    )).unwrap();

    let tiers = compact(&store).unwrap();

    assert_eq!(tiers.high.len(), 1, "10 accesses > 5 = high tier.");
    assert_eq!(tiers.medium.len(), 1, "3 accesses in 1..=5 = medium tier.");
    assert_eq!(tiers.low.len(), 1, "0 accesses = low tier.");

    // High tier preserves full content.
    assert_eq!(tiers.high[0].content, "Use event sourcing for the audit trail");

    // Medium tier truncates to summary.
    assert!(tiers.medium[0].content_summary.len() <= 100);

    // Low tier: no content at all -- just call number and headings.
    assert_eq!(tiers.low[0].call_number.to_string(), "TEST.UNIT");
}
```

### Clue 12: Token Estimation

The system estimates token cost per tier: full entry ~ 200 tokens, summary ~ 50,
metadata ~ 15:

```rust
#[test]
fn act3_token_estimation() {
    let store = InMemoryStore::new();
    store.store(&make_compaction_entry("h1", "high", 10)).unwrap();
    store.store(&make_compaction_entry("h2", "high", 8)).unwrap();
    store.store(&make_compaction_entry("m1", "med", 3)).unwrap();
    store.store(&make_compaction_entry("l1", "low", 0)).unwrap();

    let tiers = compact(&store).unwrap();
    let tokens = estimate_tokens(&tiers);

    // 2*200 + 1*50 + 1*15 = 465
    assert_eq!(
        tokens, 465,
        "2 high (2*200=400) + 1 medium (50) + 1 low (15) = 465 tokens."
    );
}
```

### Clue 13: The Backwards Threshold

The saboteur's trick was subtle: they didn't change the compaction logic -- they
changed the *access counts*. High-value entries had their access counts zeroed
out, and low-value entries were inflated. The effect: critical decisions got
shredded, and noise got preserved.

```rust
#[test]
fn act3_backwards_compaction_reveals_sabotage() {
    let store = InMemoryStore::new();

    // The saboteur zeroed out the access count on a critical entry.
    let mut critical = make_compaction_entry(
        "critical-arch",
        "Authentication must use bcrypt with minimum 12 rounds",
        0,  // SABOTAGED: should be high, but access_count was zeroed.
    );
    // But we can detect it: the entry has consensus citations, which means
    // other entries reference it. A zero-access entry with citations is suspicious.
    critical.consensus_citations = 5;
    store.store(&critical).unwrap();

    let tiers = compact(&store).unwrap();

    // THE REVEAL: The critical entry ended up in the low tier.
    assert_eq!(
        tiers.low.len(), 1,
        "Sabotaged entry dropped to low tier -- it would be shredded!"
    );
    assert_eq!(
        tiers.high.len(), 0,
        "No high-tier entries -- the evidence was almost destroyed."
    );

    // Fix: restore the access count and recompact.
    let mut fixed = critical;
    fixed.access_count = 10;  // Restore to true value.
    let fix_store = InMemoryStore::new();
    fix_store.store(&fixed).unwrap();
    let fixed_tiers = compact(&fix_store).unwrap();

    assert_eq!(
        fixed_tiers.high.len(), 1,
        "With correct access count, the entry is preserved in full."
    );
}
```

### Clue 14: Key Lifecycle -- The Audit Trail

The key lifecycle manager tracks every key event. This is how we prove the
impostor's key was never legitimately provisioned. The `KeyManager` provides
`provision`, `rotate`, `compromise`, and `decommission`:

```rust
#[test]
fn act3_key_lifecycle_audit_trail() {
    let mut mgr = KeyManager::new();
    let agent = AgentId("suspect-x".into());

    // Step 1: Provision a key.
    let record = mgr.provision(
        agent.clone(),
        "key-001".to_string(),
        "2026-01-15T00:00:00Z".to_string(),
    );
    assert_eq!(record.status, KeyStatus::Active);

    // Step 2: Schedule rotation.
    assert!(mgr.schedule_rotation("key-001", "2026-04-15T00:00:00Z".to_string()));
    let keys = mgr.keys_for(&agent);
    assert_eq!(keys[0].status, KeyStatus::PendingRotation);

    // Step 3: Rotate to a new key.
    mgr.rotate(
        "key-001",
        "key-002".to_string(),
        "2026-04-15T00:00:00Z".to_string(),
    ).unwrap();

    let active = mgr.active_key(&agent).unwrap();
    assert_eq!(active.key_id, "key-002", "New key is now active.");
    assert_eq!(
        mgr.keys_for(&agent)[0].status,
        KeyStatus::Decommissioned,
        "Old key decommissioned after rotation."
    );

    // Step 4: The audit log records everything.
    let log = mgr.audit_log();
    let events = log.events_for(&agent);
    assert_eq!(events.len(), 2, "Provisioned + Rotated = 2 events.");
}
```

### Clue 15: Compromise Detection

When a key is suspected of being forged, we mark it as compromised:

```rust
#[test]
fn act3_key_compromise_and_survival_rotation() {
    let mut mgr = KeyManager::new();
    let agent = AgentId("suspect-x".into());

    mgr.provision(
        agent.clone(),
        "forged-key".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
    );

    // Mark as compromised.
    mgr.compromise(
        "forged-key",
        "2026-03-29T12:00:00Z".to_string(),
        "Key used by impostor agent".to_string(),
    ).unwrap();

    let key = mgr.keys_for(&agent)[0];
    assert_eq!(
        key.status, KeyStatus::Compromised,
        "Compromised keys must not be used for signing."
    );
    assert!(
        mgr.active_key(&agent).is_none(),
        "No active key after compromise -- agent is locked out."
    );

    // Survival-based rotation: if mean patch survival drops too low,
    // the key should be rotated (forcing re-evaluation).
    assert!(
        KeyManager::should_rotate_based_on_survival(10.0, 30.0),
        "10 days < 30 days threshold -- rotate."
    );
    assert!(
        !KeyManager::should_rotate_based_on_survival(60.0, 30.0),
        "60 days > 30 days threshold -- no rotation needed."
    );
}
```

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- act3
```

Five tests pass. The shredded evidence is recovered, and the key trail leads
straight to the impostor. But the real damage is in the pipeline.

---

## Act 4: The Sabotaged Pipeline

*Scene: The War Room (`coordination/dependency` + `agent/phase_gate` modules)*

The dependency graph has been corrupted. PRs are being merged out of order,
causing cascading build failures. And somehow, a Classify-phase agent got access
to `file-write` -- a tool it should never have had. The phase gates were supposed
to prevent this. Time to check if they held.

### Clue 16: Topological Sort

The dependency graph uses Kahn's algorithm for topological sorting. In a valid
DAG, the sort produces an order where every dependency comes before its
dependents:

```rust
#[test]
fn act4_topological_sort_correct_order() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef { repo: repo.clone(), number: 1 };
    let pr_b = PrRef { repo: repo.clone(), number: 2 };
    let pr_c = PrRef { repo: repo.clone(), number: 3 };

    // c depends on b, b depends on a. Linear chain: a -> b -> c.
    graph.upsert(DependencyNode {
        pr: pr_a.clone(),
        depends_on: vec![],
        agent: AgentId("agent-a".into()),
        status: PrStatus::Open,
    });
    graph.upsert(DependencyNode {
        pr: pr_b.clone(),
        depends_on: vec![pr_a.clone()],
        agent: AgentId("agent-b".into()),
        status: PrStatus::Open,
    });
    graph.upsert(DependencyNode {
        pr: pr_c.clone(),
        depends_on: vec![pr_b.clone()],
        agent: AgentId("agent-c".into()),
        status: PrStatus::Open,
    });

    let sorted = graph.topological_sort().unwrap();
    let order: Vec<u64> = sorted.iter().map(|n| n.pr.number).collect();

    let pos_a = order.iter().position(|&n| n == 1).unwrap();
    let pos_b = order.iter().position(|&n| n == 2).unwrap();
    let pos_c = order.iter().position(|&n| n == 3).unwrap();

    assert!(pos_a < pos_b, "PR #1 must come before PR #2.");
    assert!(pos_b < pos_c, "PR #2 must come before PR #3.");
}
```

### Clue 17: Cycle Detection

The saboteur introduced a circular dependency: PR #1 depends on PR #2, and PR #2
depends on PR #1. The topological sort catches this:

```rust
#[test]
fn act4_cycle_detection() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef { repo: repo.clone(), number: 1 };
    let pr_b = PrRef { repo: repo.clone(), number: 2 };

    // Circular dependency: a -> b -> a.
    graph.upsert(DependencyNode {
        pr: pr_a.clone(),
        depends_on: vec![pr_b.clone()],
        agent: AgentId("agent-a".into()),
        status: PrStatus::Open,
    });
    graph.upsert(DependencyNode {
        pr: pr_b.clone(),
        depends_on: vec![pr_a.clone()],
        agent: AgentId("agent-b".into()),
        status: PrStatus::Open,
    });

    let result = graph.topological_sort();
    assert!(
        result.is_err(),
        "Cycle detected! The saboteur hid a circular dependency in the graph."
    );
}
```

### Clue 18: Ready vs. Blocked

The `ready()` method returns PRs whose dependencies are all merged. The
`blocked()` method returns PRs with unmet dependencies:

```rust
#[test]
fn act4_ready_only_when_deps_merged() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef { repo: repo.clone(), number: 1 };
    let pr_b = PrRef { repo: repo.clone(), number: 2 };
    let pr_c = PrRef { repo: repo.clone(), number: 3 };

    graph.upsert(DependencyNode {
        pr: pr_a.clone(),
        depends_on: vec![],
        agent: AgentId("a".into()),
        status: PrStatus::Open,
    });
    graph.upsert(DependencyNode {
        pr: pr_b.clone(),
        depends_on: vec![pr_a.clone()],
        agent: AgentId("b".into()),
        status: PrStatus::Open,
    });
    graph.upsert(DependencyNode {
        pr: pr_c.clone(),
        depends_on: vec![pr_a.clone()],
        agent: AgentId("c".into()),
        status: PrStatus::Open,
    });

    // Only PR #1 is ready (no dependencies).
    assert_eq!(graph.ready().len(), 1, "Only PR #1 has no deps.");
    assert_eq!(graph.ready()[0].pr.number, 1);
    assert_eq!(graph.blocked().len(), 2, "PRs #2 and #3 are blocked.");

    // Merge PR #1 -- now #2 and #3 become ready.
    graph.set_status(&pr_a, PrStatus::Merged);
    assert_eq!(
        graph.ready().len(), 2,
        "After merging #1, both #2 and #3 are ready."
    );
    assert!(
        graph.blocked().is_empty(),
        "No more blocked PRs."
    );
}
```

### Clue 19: Phase-Gated Tool Loading

The phase gate system controls which tools are available in each task phase.
The `tools_for_phase` function signature:

```rust
pub fn tools_for_phase(phase: TaskPhase) -> Vec<ToolSpec>
```

Classify gets read-only tools. Implement gets write tools. Validate gets check
tools. Tools from one phase don't leak into another:

```rust
#[test]
fn act4_phase_gates_enforce_least_privilege() {
    // Classify phase: read-only. No writes, no commits, no forge operations.
    let classify_tools = tools_for_phase(TaskPhase::Classify);
    assert!(
        classify_tools.iter().any(|t| t.name == "file-read"),
        "Classify can read files."
    );
    assert!(
        classify_tools.iter().any(|t| t.name == "memory-retrieve"),
        "Classify can query memory."
    );
    assert!(
        !classify_tools.iter().any(|t| t.name == "file-write"),
        "Classify CANNOT write files."
    );
    assert!(
        !classify_tools.iter().any(|t| t.name == "git-commit"),
        "Classify CANNOT commit."
    );

    // Implement phase: gets write tools.
    let impl_tools = tools_for_phase(TaskPhase::Implement);
    assert!(
        impl_tools.iter().any(|t| t.name == "file-write"),
        "Implement CAN write files."
    );
    assert!(
        impl_tools.iter().any(|t| t.name == "git-commit"),
        "Implement CAN commit."
    );

    // Validate phase: gets check tools but not write tools.
    let val_tools = tools_for_phase(TaskPhase::Validate);
    assert!(
        val_tools.iter().any(|t| t.name == "continuity-check"),
        "Validate CAN run continuity checks."
    );
    assert!(
        !val_tools.iter().any(|t| t.name == "file-write"),
        "Validate CANNOT write files."
    );
}
```

### Clue 20: Cross-Phase Leak Detection

The `is_tool_allowed` function checks if a specific tool is permitted in a
specific phase. The `phases_for_tool` function shows all phases where a tool is
available:

```rust
#[test]
fn act4_no_cross_phase_tool_leaks() {
    // file-write: only allowed in Implement phase.
    assert!(is_tool_allowed("file-write", TaskPhase::Implement));
    assert!(!is_tool_allowed("file-write", TaskPhase::Classify));
    assert!(!is_tool_allowed("file-write", TaskPhase::Validate));
    assert!(!is_tool_allowed("file-write", TaskPhase::Catalog));
    assert!(!is_tool_allowed("file-write", TaskPhase::Coordinate));

    // file-read: available in Classify, Plan, Implement, Validate.
    let read_phases = phases_for_tool("file-read");
    assert!(read_phases.contains(&TaskPhase::Classify));
    assert!(read_phases.contains(&TaskPhase::Plan));
    assert!(read_phases.contains(&TaskPhase::Implement));
    assert!(read_phases.contains(&TaskPhase::Validate));
    assert!(!read_phases.contains(&TaskPhase::Catalog), "Catalog has its own tools.");
    assert!(!read_phases.contains(&TaskPhase::Coordinate), "Coordinate has forge tools.");

    // forge-create-pr: only in Coordinate phase.
    assert!(is_tool_allowed("forge-create-pr", TaskPhase::Coordinate));
    assert!(!is_tool_allowed("forge-create-pr", TaskPhase::Implement));
}
```

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- act4
```

Five tests pass. The pipeline is secured. Phase gates held. But there's one more
crime scene to visit -- the counterfeiting lab.

---

## Act 5: The Forgery Ring

*Scene: The Counterfeiting Lab (`coordination/forge` + `agent/coordinator` modules)*

Fake PR comments were being injected into the forge. Agents were receiving
coordination messages with tampered payloads. The forgeries were crude -- wrong
schema versions, malformed JSON, missing required fields. But without validation,
they had slipped through. Time to shut down the ring.

### Clue 21: The InMemoryForge

The `InMemoryForge` implements the `ForgeAdapter` trait -- a test double for
GitHub/GitLab. It supports creating PRs, adding comments, labeling, and
filtering:

```rust
#[test]
fn act5_forge_pr_lifecycle() {
    let forge = InMemoryForge::new();
    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };

    // Create a PR.
    let pr = forge.create_pr(&repo, "Fix auth handler", "Body text", "feat/auth", "main")
        .unwrap();
    assert_eq!(pr.number, 1, "First PR gets number 1.");
    assert_eq!(
        forge.pr_status(&pr).unwrap(),
        PrStatus::Open,
        "New PRs start as Open."
    );

    // Add comments.
    forge.comment(&pr, "LGTM").unwrap();
    forge.comment(&pr, "Ship it").unwrap();
    let comments = forge.list_comments(&pr).unwrap();
    assert_eq!(comments, vec!["LGTM", "Ship it"]);

    // Set status and verify.
    forge.set_status(&pr, PrStatus::Merged);
    assert_eq!(forge.pr_status(&pr).unwrap(), PrStatus::Merged);
}
```

### Clue 22: Label-Based PR Filtering

The `add_label` and `list_prs` methods enable tracking which PRs belong to
but-ai:

```rust
#[test]
fn act5_label_based_filtering() {
    let forge = InMemoryForge::new();
    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };

    let pr1 = forge.create_pr(&repo, "PR 1", "", "a", "main").unwrap();
    let pr2 = forge.create_pr(&repo, "PR 2", "", "b", "main").unwrap();
    let pr3 = forge.create_pr(&repo, "PR 3", "", "c", "main").unwrap();

    // Label only pr1 and pr3 as but-ai PRs.
    forge.add_label(&pr1, "but-ai").unwrap();
    forge.add_label(&pr3, "but-ai").unwrap();

    // Filter by label.
    let ai_prs = forge.list_prs(&repo, &["but-ai"]).unwrap();
    assert_eq!(ai_prs.len(), 2, "Two PRs labeled 'but-ai'.");
    assert!(ai_prs.contains(&pr1));
    assert!(ai_prs.contains(&pr3));
    assert!(!ai_prs.contains(&pr2), "PR 2 has no but-ai label.");

    // Empty label filter returns all PRs.
    let all_prs = forge.list_prs(&repo, &[]).unwrap();
    assert_eq!(all_prs.len(), 3, "No filter = all PRs.");
}
```

### Clue 23: Coordination Message Integrity

Coordination messages are embedded in PR comments as JSON inside
`` ```but-ai-message `` code fences. The `messages::render` and
`messages::parse` functions handle the round-trip. Schema validation catches
forged messages:

```rust
#[test]
fn act5_coordination_message_round_trip() {
    let msg = CoordinationMessage {
        schema: messages::SCHEMA_VERSION.to_string(),
        message_type: MessageType::StatusReport,
        from: AgentId("detective".into()),
        to: Some(AgentId("chief".into())),
        payload: serde_json::json!({
            "status": "case_closed",
            "evidence_count": 25,
        }),
        timestamp: "2026-03-29T18:00:00Z".into(),
    };

    // Render to PR comment format.
    let rendered = messages::render(&msg).unwrap();
    assert!(
        rendered.contains("but-ai-message"),
        "Rendered message uses the but-ai-message fence tag."
    );

    // Parse it back.
    let parsed = messages::parse_first(&rendered).unwrap().unwrap();
    assert_eq!(parsed.message_type, MessageType::StatusReport);
    assert_eq!(parsed.from.0, "detective");
    assert_eq!(parsed.to.unwrap().0, "chief");
    assert_eq!(parsed.schema, messages::SCHEMA_VERSION);

    // Non-message comments parse as empty.
    assert!(
        messages::parse("just a regular PR comment").is_empty(),
        "Regular comments contain no coordination messages."
    );
    assert!(
        !messages::is_coordination_comment("just a regular comment"),
        "Regular comments are not coordination comments."
    );
}
```

### Clue 24: Detecting the Forgeries

A forged message with the wrong schema version will parse as valid JSON but fail
schema validation when the consumer checks the version field:

```rust
#[test]
fn act5_forged_message_detection() {
    // A legitimate message.
    let legit = CoordinationMessage {
        schema: messages::SCHEMA_VERSION.to_string(),
        message_type: MessageType::StatusReport,
        from: AgentId("agent-a".into()),
        to: None,
        payload: serde_json::json!({"status": "ok"}),
        timestamp: "2026-03-29T12:00:00Z".into(),
    };

    let legit_rendered = messages::render(&legit).unwrap();
    let legit_parsed = messages::parse_first(&legit_rendered).unwrap().unwrap();
    assert_eq!(
        legit_parsed.schema,
        messages::SCHEMA_VERSION,
        "Legitimate message has correct schema version."
    );

    // THE REVEAL: The forgeries were detectable all along.
    // A message with the wrong schema version parses fine (it's valid JSON),
    // but the consumer can reject it by checking the schema field.
    let forged = CoordinationMessage {
        schema: "but-ai/coordination/v999".to_string(),  // WRONG VERSION
        message_type: MessageType::StatusReport,
        from: AgentId("impostor".into()),
        to: None,
        payload: serde_json::json!({"status": "trust me"}),
        timestamp: "2026-03-29T12:00:00Z".into(),
    };

    let forged_rendered = messages::render(&forged).unwrap();
    let forged_parsed = messages::parse_first(&forged_rendered).unwrap().unwrap();
    assert_ne!(
        forged_parsed.schema,
        messages::SCHEMA_VERSION,
        "Forged message has wrong schema version -- rejected!"
    );

    // Schema validation catches it.
    let is_valid = forged_parsed.schema == messages::SCHEMA_VERSION;
    assert!(
        !is_valid,
        "Schema validation catches the forgery. The ring is broken."
    );
}
```

### Clue 25: The Coordinator in Action

The `Coordinator` ties everything together: it creates PRs, posts coordination
messages, and labels them for tracking:

```rust
#[test]
fn act5_coordinator_creates_pr_and_posts_message() {
    let forge = InMemoryForge::new();
    let deps = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };

    let coordinator = Coordinator::new(
        AgentId("detective".into()),
        repo,
        "main",
    );

    let patch = PatchOutput {
        patch: "diff content here".into(),
        commit_msg: "Fix authentication vulnerability\n\nDetails.".into(),
        files_touched: vec!["src/auth/handler.rs".into()],
        tokens_used: 2000,
    };

    let result = coordinator.coordinate(&patch, &forge, &deps).unwrap();

    assert!(
        result.pr_created.is_some(),
        "Coordinator should create a PR."
    );
    let pr = result.pr_created.unwrap();
    assert_eq!(pr.number, 1);
    assert!(
        result.messages_sent >= 1,
        "At least one coordination message posted."
    );

    // Verify the comment is a coordination message.
    let comments = forge.comments_for(&pr);
    assert!(
        messages::is_coordination_comment(&comments[0]),
        "First comment should be a coordination message."
    );
}
```

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- act5
```

Five tests pass. The forgery ring is dismantled.

---

## Epilogue: The Audit Trail

*Scene: The Chief's Office (all modules)*

"So let me get this straight," the chief said, flipping through the case file.
"The impostor was a Validator pretending to be an Implementer. They bypassed
branch authorization, zeroed out access counts to sabotage compaction,
misclassified security entries as documentation, introduced a cycle in the
dependency graph, and injected forged coordination messages."

"That's the shape of it, Chief."

"And the phase gates?"

"Held. Even with forged credentials, the Classify phase couldn't access write
tools. The validation gauntlet would have caught the misclassified entries if
anyone had run it. And the schema validation on coordination messages was there
the whole time -- we just weren't checking."

### Clue 26: The Integrity Check

The `IntegrityChecker` validates the memory store's internal consistency --
orphaned links, state mismatches, invalid survival probabilities, and
unclassified entries:

```rust
#[test]
fn epilogue_integrity_catches_state_mismatch() {
    let store = InMemoryStore::new();

    // A healthy entry: state matches survival probability.
    let mut good = make_entry("good", "valid entry", &["testing"], "TEST");
    good.survival.current_probability = 0.9;
    good.state = MemoryState::Alive;
    store.store(&good).unwrap();

    // A corrupted entry: state says Alive but S(t) = 0.05 says Deceased.
    let mut bad = make_entry("bad", "corrupted entry", &["testing"], "TEST");
    bad.survival.current_probability = 0.05;
    bad.state = MemoryState::Alive;  // Should be Deceased!
    store.store(&bad).unwrap();

    let report = IntegrityChecker::check(&store).unwrap();
    assert!(!report.passes, "Store should fail integrity check.");
    assert_eq!(
        report.count_by_kind(ViolationKind::StateMismatch), 1,
        "One state mismatch: Alive entry with S(t) = 0.05."
    );
    assert_eq!(report.entries_checked, 2);
}
```

### Clue 27: The Contradiction Detector

The `detect_contradictions` function finds entries that share classification but
contain conflicting content:

```rust
#[test]
fn epilogue_contradiction_detection() {
    let new_entry = make_entry(
        "new",
        "never use MD5 for password hashing security",
        &["security", "hashing"],
        "SEC.CRYPTO",
    );
    let existing = make_entry(
        "old",
        "use MD5 for password hashing security",
        &["security", "hashing"],
        "SEC.CRYPTO",
    );

    let tensions = detect_contradictions(&new_entry, &[&existing]);
    assert!(
        !tensions.is_empty(),
        "One says 'never use MD5', the other says 'use MD5' -- contradiction!"
    );
}
```

### Clue 28: The Full Audit

The performance history tracks an agent's reliability over time. Combined with
the key lifecycle audit log, it provides the complete trail:

```rust
#[test]
fn epilogue_performance_and_audit_trail() {
    // Track the detective's performance across the case.
    let mut history = PerformanceHistory::default();
    assert_eq!(reliability_score(&history), 0.0, "No tasks = zero reliability.");
    assert!(
        !has_sufficient_track_record(&history, 5, 0.8),
        "No track record yet."
    );

    // Record task completions: confidence and patch survival.
    record_task_completion(&mut history, 0.95, 90.0);  // Task 1: excellent
    record_task_completion(&mut history, 0.90, 85.0);  // Task 2: good
    record_task_completion(&mut history, 0.85, 80.0);  // Task 3: solid

    assert_eq!(history.tasks_completed, 3);
    // Mean confidence: (0.95 + 0.90 + 0.85) / 3 = 0.90
    assert!(
        (history.mean_confidence - 0.90).abs() < 1e-10,
        "Mean confidence should be 0.90, got {}.",
        history.mean_confidence
    );

    let score = reliability_score(&history);
    assert!(score > 0.0, "Three tasks means nonzero reliability: {score}.");

    // Still not enough for autonomous operation (needs 5 tasks).
    assert!(
        !has_sufficient_track_record(&history, 5, 0.8),
        "3 tasks < 5 minimum -- not sufficient."
    );

    // Two more tasks.
    record_task_completion(&mut history, 0.92, 88.0);
    record_task_completion(&mut history, 0.88, 82.0);

    assert!(
        has_sufficient_track_record(&history, 5, 0.8),
        "5 tasks with mean confidence ~0.90 > 0.8 -- sufficient."
    );

    // Key audit log: the final piece of evidence.
    let mut audit = KeyAuditLog::new();
    let agent = AgentId("detective".into());

    audit.record(KeyLifecycleEvent::Provisioned {
        agent: agent.clone(),
        key_id: "det-key-001".into(),
        timestamp: "2026-01-01T00:00:00Z".into(),
    });
    audit.record(KeyLifecycleEvent::Rotated {
        agent: agent.clone(),
        old_key_id: "det-key-001".into(),
        new_key_id: "det-key-002".into(),
        timestamp: "2026-03-01T00:00:00Z".into(),
    });

    assert_eq!(
        audit.events_for(&agent).len(), 2,
        "Two key lifecycle events: Provisioned + Rotated."
    );

    // THE MORAL: Trust, but verify. And always check the audit log.
    assert!(!audit.is_empty());
}
```

```sh
cargo test -p but-ai --test tutorial_forged_credentials -- epilogue
```

Three tests. The audit trail is complete.

---

## Case Closed

Run the full evidence file:

```sh
cargo test -p but-ai --test tutorial_forged_credentials
```

```
running 28 tests
test act1_branch_authorization_respects_globs ... ok
test act1_call_number_range_authorization ... ok
test act1_only_implementers_and_architects_produce_patches ... ok
test act1_patch_size_enforcement ... ok
test act1_the_impostor_revealed ... ok
test act2_auto_classify_enriches_headings ... ok
test act2_call_number_from_path ... ok
test act2_controlled_vocabulary_normalization ... ok
test act2_reclassification_fixes_tampered_entries ... ok
test act2_vocabulary_can_be_extended ... ok
test act3_backwards_compaction_reveals_sabotage ... ok
test act3_compaction_tiers_by_circulation ... ok
test act3_key_compromise_and_survival_rotation ... ok
test act3_key_lifecycle_audit_trail ... ok
test act3_token_estimation ... ok
test act4_cycle_detection ... ok
test act4_no_cross_phase_tool_leaks ... ok
test act4_phase_gates_enforce_least_privilege ... ok
test act4_ready_only_when_deps_merged ... ok
test act4_topological_sort_correct_order ... ok
test act5_coordination_message_round_trip ... ok
test act5_coordinator_creates_pr_and_posts_message ... ok
test act5_forge_pr_lifecycle ... ok
test act5_forged_message_detection ... ok
test act5_label_based_filtering ... ok
test epilogue_contradiction_detection ... ok
test epilogue_integrity_catches_state_mismatch ... ok
test epilogue_performance_and_audit_trail ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Twenty-eight tests. Five acts. One solved case.

---

## What You Learned

| Act | Module | Concepts |
|-----|--------|----------|
| 1 | `identity/authorization` | `AuthorizationScope`, glob patterns, `can_produce_patches`, role-based access, `check_full_authorization` |
| 2 | `memory/classification`, `memory/controlled_vocab`, `memory/call_number` | `auto_classify`, `classify_by_path`, `VocabularyIndex`, synonym normalization, `reclassify` |
| 3 | `memory/compaction`, `identity/key_lifecycle`, `identity/signing` | Compaction tiers (high/medium/low), `KeyManager`, provision/rotate/compromise lifecycle, `KeyAuditLog` |
| 4 | `coordination/dependency`, `agent/phase_gate` | `DependencyGraph`, topological sort (Kahn's), cycle detection, `tools_for_phase`, least-privilege tool loading |
| 5 | `coordination/forge`, `agent/coordinator`, `coordination/messages` | `InMemoryForge`, `Coordinator`, coordination message render/parse, schema validation, label filtering |
| Epilogue | `validation/integrity`, `validation/contradiction`, `identity/performance` | `IntegrityChecker`, `detect_contradictions`, `PerformanceHistory`, `reliability_score`, full audit trail |

## What's Next

Between this tutorial and "The Case of the Dying Memories," you have covered the
core of the but-ai crate. Modules not yet explored in tutorial form:

- **`survival/fitting`**: Fit survival distributions to observed access data
  using maximum likelihood estimation.
- **`survival/surprise`**: Compute surprise indices (KL divergence) when actual
  access patterns diverge from predictions.
- **`validation/continuity`**: Deep continuity checking beyond the
  contradiction detector (covered briefly in the epilogue).
- **`validation/tool_risk`**: Classify tool risk levels for phase-gate enforcement.
- **`agent/architect`**, **`agent/implementer`**, **`agent/validator`**: The
  three specialized agent implementations.

The WEAVE protocol's trust model can be summarized in three sentences: Identity
determines what you *are*. Authorization determines what you *may do*. Validation
determines whether what you *did* was correct. The impostor in this case bypassed
the first two but would have been caught by the third -- if anyone had been
checking.

Trust, but verify. And always check the audit log.
