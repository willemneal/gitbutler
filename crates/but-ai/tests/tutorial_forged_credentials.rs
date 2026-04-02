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
use but_ai::identity::signing::{KeyAuditLog, KeyLifecycleEvent};
use but_ai::memory::call_number::call_number_from_path;
use but_ai::memory::classification::{auto_classify, classify_by_path, reclassify};
use but_ai::memory::compaction::{compact, estimate_tokens};
use but_ai::memory::controlled_vocab::VocabularyIndex;
use but_ai::memory::store::InMemoryStore;
use but_ai::types::*;
use but_ai::validation::contradiction::detect_contradictions;
use but_ai::validation::integrity::{IntegrityChecker, ViolationKind};

// ---------------------------------------------------------------------------
// Helpers -- the detective's toolkit
// ---------------------------------------------------------------------------

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

// ===========================================================================
// Act 1: The Impostor (Identity & Authorization)
// ===========================================================================

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

#[test]
fn act1_the_impostor_revealed() {
    // The suspect: claims to be an Implementer, but is actually a Validator.
    let impostor = make_identity(
        "suspect-x",
        AgentRole::Validator, // THE REVEAL: Validator, not Implementer
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

// ===========================================================================
// Act 2: The Tampered Catalog (Classification & Controlled Vocabulary)
// ===========================================================================

#[test]
fn act2_auto_classify_enriches_headings() {
    let mut entry = make_entry(
        "vuln-report",
        "authentication vulnerability in JWT token validation middleware",
        &["security"], // Only one heading -- woefully incomplete.
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

#[test]
fn act2_call_number_from_path() {
    // crates/but-ai/src/memory/store.rs -> CRATES.BUT-AI.MEMORY.STORE
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
    let subjects = vec!["auth".into(), "authentication".into(), "authn".into()];
    let normalized = vocab.normalize_subjects(&subjects);
    assert_eq!(
        normalized.len(),
        1,
        "Three synonyms collapse to one canonical form."
    );
    assert_eq!(normalized[0], "authentication");
}

#[test]
fn act2_reclassification_fixes_tampered_entries() {
    // The tampered entry: security content filed under "documentation."
    let mut tampered = make_entry(
        "vuln-report",
        "critical authentication vulnerability in JWT handling",
        &["documentation"], // WRONG -- this is security content!
        "DOC.README",       // WRONG -- this should be SEC.AUTH!
    );

    // The fix: reclassify with correct metadata.
    reclassify(
        &mut tampered,
        Some(vec![
            "security".into(),
            "authentication".into(),
            "vulnerability".into(),
        ]),
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

#[test]
fn act2_vocabulary_can_be_extended() {
    let mut vocab = VocabularyIndex::with_defaults();

    // THE REVEAL: The default vocabulary doesn't include "vuln" -> "vulnerability".
    assert_eq!(
        vocab.normalize("vuln"),
        "vuln",
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

// ===========================================================================
// Act 3: The Shredded Evidence (Compaction & Lifecycle)
// ===========================================================================

#[test]
fn act3_compaction_tiers_by_circulation() {
    let store = InMemoryStore::new();

    // High-circulation: architectural decision accessed 10 times.
    store
        .store(&make_compaction_entry(
            "arch-decision",
            "Use event sourcing for the audit trail",
            10,
        ))
        .unwrap();

    // Medium-circulation: implementation note accessed 3 times.
    store
        .store(&make_compaction_entry(
            "impl-note",
            "Handler uses async middleware pattern",
            3,
        ))
        .unwrap();

    // Low-circulation: never accessed.
    store
        .store(&make_compaction_entry(
            "dead-letter",
            "Placeholder entry for future work",
            0,
        ))
        .unwrap();

    let tiers = compact(&store).unwrap();

    assert_eq!(tiers.high.len(), 1, "10 accesses > 5 = high tier.");
    assert_eq!(tiers.medium.len(), 1, "3 accesses in 1..=5 = medium tier.");
    assert_eq!(tiers.low.len(), 1, "0 accesses = low tier.");

    // High tier preserves full content.
    assert_eq!(
        tiers.high[0].content,
        "Use event sourcing for the audit trail"
    );

    // Medium tier truncates to summary.
    assert!(tiers.medium[0].content_summary.len() <= 100);

    // Low tier: no content at all -- just call number and headings.
    assert_eq!(tiers.low[0].call_number.to_string(), "TEST.UNIT");
}

#[test]
fn act3_token_estimation() {
    let store = InMemoryStore::new();
    store
        .store(&make_compaction_entry("h1", "high", 10))
        .unwrap();
    store
        .store(&make_compaction_entry("h2", "high", 8))
        .unwrap();
    store
        .store(&make_compaction_entry("m1", "med", 3))
        .unwrap();
    store
        .store(&make_compaction_entry("l1", "low", 0))
        .unwrap();

    let tiers = compact(&store).unwrap();
    let tokens = estimate_tokens(&tiers);

    // 2*200 + 1*50 + 1*15 = 465
    assert_eq!(
        tokens, 465,
        "2 high (2*200=400) + 1 medium (50) + 1 low (15) = 465 tokens."
    );
}

#[test]
fn act3_backwards_compaction_reveals_sabotage() {
    let store = InMemoryStore::new();

    // The saboteur zeroed out the access count on a critical entry.
    let mut critical = make_compaction_entry(
        "critical-arch",
        "Authentication must use bcrypt with minimum 12 rounds",
        0, // SABOTAGED: should be high, but access_count was zeroed.
    );
    critical.consensus_citations = 5;
    store.store(&critical).unwrap();

    let tiers = compact(&store).unwrap();

    // THE REVEAL: The critical entry ended up in the low tier.
    assert_eq!(
        tiers.low.len(),
        1,
        "Sabotaged entry dropped to low tier -- it would be shredded!"
    );
    assert_eq!(
        tiers.high.len(),
        0,
        "No high-tier entries -- the evidence was almost destroyed."
    );

    // Fix: restore the access count and recompact.
    let mut fixed = critical;
    fixed.access_count = 10;
    let fix_store = InMemoryStore::new();
    fix_store.store(&fixed).unwrap();
    let fixed_tiers = compact(&fix_store).unwrap();

    assert_eq!(
        fixed_tiers.high.len(),
        1,
        "With correct access count, the entry is preserved in full."
    );
}

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
    )
    .unwrap();

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
    )
    .unwrap();

    let key = mgr.keys_for(&agent)[0];
    assert_eq!(
        key.status,
        KeyStatus::Compromised,
        "Compromised keys must not be used for signing."
    );
    assert!(
        mgr.active_key(&agent).is_none(),
        "No active key after compromise -- agent is locked out."
    );

    // Survival-based rotation.
    assert!(
        KeyManager::should_rotate_based_on_survival(10.0, 30.0),
        "10 days < 30 days threshold -- rotate."
    );
    assert!(
        !KeyManager::should_rotate_based_on_survival(60.0, 30.0),
        "60 days > 30 days threshold -- no rotation needed."
    );
}

// ===========================================================================
// Act 4: The Sabotaged Pipeline (Dependency DAG & Phase Gating)
// ===========================================================================

#[test]
fn act4_topological_sort_correct_order() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef {
        repo: repo.clone(),
        number: 1,
    };
    let pr_b = PrRef {
        repo: repo.clone(),
        number: 2,
    };
    let pr_c = PrRef {
        repo: repo.clone(),
        number: 3,
    };

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

#[test]
fn act4_cycle_detection() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef {
        repo: repo.clone(),
        number: 1,
    };
    let pr_b = PrRef {
        repo: repo.clone(),
        number: 2,
    };

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

#[test]
fn act4_ready_only_when_deps_merged() {
    let mut graph = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "org".into(),
        repo: "repo".into(),
    };

    let pr_a = PrRef {
        repo: repo.clone(),
        number: 1,
    };
    let pr_b = PrRef {
        repo: repo.clone(),
        number: 2,
    };
    let pr_c = PrRef {
        repo: repo.clone(),
        number: 3,
    };

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
        graph.ready().len(),
        2,
        "After merging #1, both #2 and #3 are ready."
    );
    assert!(graph.blocked().is_empty(), "No more blocked PRs.");
}

#[test]
fn act4_phase_gates_enforce_least_privilege() {
    // Classify phase: read-only.
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
    assert!(
        !read_phases.contains(&TaskPhase::Catalog),
        "Catalog has its own tools."
    );
    assert!(
        !read_phases.contains(&TaskPhase::Coordinate),
        "Coordinate has forge tools."
    );

    // forge-create-pr: only in Coordinate phase.
    assert!(is_tool_allowed("forge-create-pr", TaskPhase::Coordinate));
    assert!(!is_tool_allowed("forge-create-pr", TaskPhase::Implement));
}

// ===========================================================================
// Act 5: The Forgery Ring (Forge Adapter & Coordination)
// ===========================================================================

#[test]
fn act5_forge_pr_lifecycle() {
    let forge = InMemoryForge::new();
    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };

    // Create a PR.
    let pr = forge
        .create_pr(&repo, "Fix auth handler", "Body text", "feat/auth", "main")
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
    let forged = CoordinationMessage {
        schema: "but-ai/coordination/v999".to_string(), // WRONG VERSION
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

#[test]
fn act5_coordinator_creates_pr_and_posts_message() {
    let forge = InMemoryForge::new();
    let deps = DependencyGraph::new();

    let repo = RepoRef {
        forge: ForgeType::GitHub,
        owner: "test-org".into(),
        repo: "test-repo".into(),
    };

    let coordinator = Coordinator::new(AgentId("detective".into()), repo, "main");

    let patch = PatchOutput {
        patch: "diff content here".into(),
        commit_msg: "Fix authentication vulnerability\n\nDetails.".into(),
        files_touched: vec!["src/auth/handler.rs".into()],
        tokens_used: 2000,
    };

    let result = coordinator.coordinate(&patch, &forge, &deps).unwrap();

    assert!(result.pr_created.is_some(), "Coordinator should create a PR.");
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

// ===========================================================================
// Epilogue: The Audit Trail
// ===========================================================================

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
    bad.state = MemoryState::Alive; // Should be Deceased!
    store.store(&bad).unwrap();

    let report = IntegrityChecker::check(&store).unwrap();
    assert!(!report.passes, "Store should fail integrity check.");
    assert_eq!(
        report.count_by_kind(ViolationKind::StateMismatch),
        1,
        "One state mismatch: Alive entry with S(t) = 0.05."
    );
    assert_eq!(report.entries_checked, 2);
}

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

#[test]
fn epilogue_performance_and_audit_trail() {
    // Track the detective's performance across the case.
    let mut history = PerformanceHistory::default();
    assert_eq!(
        reliability_score(&history),
        0.0,
        "No tasks = zero reliability."
    );
    assert!(
        !has_sufficient_track_record(&history, 5, 0.8),
        "No track record yet."
    );

    // Record task completions: confidence and patch survival.
    record_task_completion(&mut history, 0.95, 90.0); // Task 1
    record_task_completion(&mut history, 0.90, 85.0); // Task 2
    record_task_completion(&mut history, 0.85, 80.0); // Task 3

    assert_eq!(history.tasks_completed, 3);
    // Mean confidence: (0.95 + 0.90 + 0.85) / 3 = 0.90
    assert!(
        (history.mean_confidence - 0.90).abs() < 1e-10,
        "Mean confidence should be 0.90, got {}.",
        history.mean_confidence
    );

    let score = reliability_score(&history);
    assert!(
        score > 0.0,
        "Three tasks means nonzero reliability: {score}."
    );

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
        audit.events_for(&agent).len(),
        2,
        "Two key lifecycle events: Provisioned + Rotated."
    );

    // THE MORAL: Trust, but verify. And always check the audit log.
    assert!(!audit.is_empty());
}
