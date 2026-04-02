//! A Week with WEAVE -- integration tests for the developer workflow tutorial.
//!
//! Each test simulates one day of a developer's week, verifying the behind-the-
//! scenes behavior that but-ai performs invisibly. These are product-level tests:
//! they care about outcomes ("does the AI remember?"), not internals ("did the
//! hash function work?").
//!
//! Run with: `cargo test -p but-ai --test tutorial_workflow`

use but_ai::agent::{budget_mode, BudgetMode};
use but_ai::agent::budget::should_proceed;
use but_ai::coordination::gossip::GossipEngine;
use but_ai::memory::lifecycle::{audit_lifecycle, state_for_probability};
use but_ai::memory::retrieval::RetrievalEngine;
use but_ai::memory::see_also::SeeAlsoGraph;
use but_ai::memory::store::InMemoryStore;
use but_ai::narrative::motif::MotifIndex;
use but_ai::narrative::tension::TensionRegistry;
use but_ai::types::*;

// ---------------------------------------------------------------------------
// Helpers -- building realistic memory entries
// ---------------------------------------------------------------------------

/// Create a memory entry that looks like a real task output.
fn task_entry(
    id: &str,
    agent: &str,
    content: &str,
    headings: &[&str],
    call_number: &str,
) -> MemoryEntry {
    MemoryEntry {
        id: EntryId(id.into()),
        agent: AgentId(agent.into()),
        content: content.into(),
        created_at: "2026-03-24T09:00:00Z".into(),
        last_accessed: "2026-03-24T09:00:00Z".into(),
        classification: Classification {
            subject_headings: headings.iter().map(|s| s.to_string()).collect(),
            call_number: CallNumber::parse(call_number),
            controlled_vocab: true,
        },
        see_also: Vec::new(),
        motifs: Vec::new(),
        tension_refs: Vec::new(),
        survival: SurvivalMetadata {
            distribution: SurvivalDistribution::Weibull { k: 2.0, lambda: 100.0 },
            current_probability: 0.95,
            hazard_rate: 0.01,
            surprise_index: 0.0,
            goodness_of_fit: 0.90,
        },
        state: MemoryState::Alive,
        consensus_citations: 0,
        access_count: 1,
        source_commit: None,
    }
}

// ---------------------------------------------------------------------------
// Day 1: Your First Memory -- two tasks, linked via see-also
// ---------------------------------------------------------------------------

#[test]
fn day_1_first_memory_created_and_linked() {
    // Monday: "Set up the database layer with PostgreSQL"
    let db_setup = task_entry(
        "db-setup",
        "alice",
        "Set up PostgreSQL database layer with migrations, schema definition, \
         and initial connection configuration using sqlx.",
        &["database", "postgresql", "setup", "migrations"],
        "INFRA.DB.SETUP",
    );

    // Later Monday: "Add connection pooling to the database layer"
    let db_pooling = task_entry(
        "db-pooling",
        "alice",
        "Added connection pooling to the PostgreSQL database layer using \
         sqlx::PgPool with configurable max connections and idle timeout.",
        &["database", "postgresql", "connection-pooling"],
        "INFRA.DB.POOL",
    );

    // Store both entries.
    let store = InMemoryStore::new();
    store.store(&db_setup).unwrap();
    store.store(&db_pooling).unwrap();

    // but-ai creates a see-also link between related entries.
    let mut graph = SeeAlsoGraph::new(10);
    graph.add_link(
        EntryId("db-setup".into()),
        EntryId("db-pooling".into()),
        Relationship::RelatedTo,
        "Connection pooling builds on the database setup".into(),
    );

    // Verify: querying for "database" finds both entries.
    let engine = RetrievalEngine::new(store, graph);
    let results = engine
        .retrieve("database connection", 10, &RelevanceWeights::default())
        .unwrap();

    assert_eq!(results.len(), 2, "both database entries should be retrieved");

    // Verify: the entries are linked via see-also.
    let links = engine.see_also().get_links(&EntryId("db-setup".into()));
    assert_eq!(links.len(), 1, "setup entry should link to pooling entry");
    assert_eq!(links[0].target_id, EntryId("db-pooling".into()));

    // Verify: the link is bidirectional.
    let reverse = engine.see_also().get_links(&EntryId("db-pooling".into()));
    assert_eq!(reverse.len(), 1, "pooling entry should link back to setup");
    assert_eq!(reverse[0].target_id, EntryId("db-setup".into()));
}

// ---------------------------------------------------------------------------
// Day 2: The Pattern Forms -- motif emergence after 3 database tasks
// ---------------------------------------------------------------------------

#[test]
fn day_2_motif_emerges_after_three_tasks() {
    let mut motif_index = MotifIndex::new();
    let motif_id = MotifId("database-infrastructure".into());

    // Task 1: database setup (Monday).
    let promoted = motif_index.record_appearance(
        &motif_id,
        "database infrastructure and configuration",
        &EntryId("db-setup".into()),
    );
    assert!(!promoted, "one appearance is not enough for a motif");
    assert!(!motif_index.is_emerged(&motif_id));

    // Task 2: connection pooling (Monday).
    let promoted = motif_index.record_appearance(
        &motif_id,
        "database infrastructure and configuration",
        &EntryId("db-pooling".into()),
    );
    assert!(!promoted, "two appearances is still a proto-motif");
    assert!(!motif_index.is_emerged(&motif_id));

    // Task 3: query logging (Tuesday).
    let promoted = motif_index.record_appearance(
        &motif_id,
        "database infrastructure and configuration",
        &EntryId("db-logging".into()),
    );
    assert!(promoted, "three appearances should promote the proto-motif");
    assert!(motif_index.is_emerged(&motif_id));

    // The motif now connects all three entries.
    let motif = motif_index.get(&motif_id).unwrap();
    assert_eq!(motif.appearances.len(), 3);

    // Retrieval via motif: all 3 entries are reachable.
    let entries = motif_index.entries_from_motifs(&[motif_id.clone()]);
    assert_eq!(entries.len(), 3, "motif should surface all 3 entries");

    // Resonance scoring: an entry tagged with this motif scores positively.
    let mut entry = task_entry(
        "db-logging",
        "alice",
        "Added query logging to the database layer with structured output.",
        &["database", "logging"],
        "INFRA.DB.LOG",
    );
    entry.motifs.push(motif_id);
    let resonance = motif_index.resonance("database infrastructure", &entry);
    assert!(
        resonance > 0.0,
        "query mentioning 'database infrastructure' should resonate with the motif"
    );
}

// ---------------------------------------------------------------------------
// Day 3: The Contradiction -- tension detection
// ---------------------------------------------------------------------------

#[test]
fn day_3_contradiction_detected_automatically() {
    let mut registry = TensionRegistry::new();

    // Wednesday: switching to SQLite for tests creates a tension.
    let tension = Tension {
        id: TensionId("pg-vs-sqlite".into()),
        description: "Production uses PostgreSQL but test environment uses SQLite -- \
                      connection pooling and query syntax may diverge."
            .into(),
        severity: TensionSeverity::Low,
        introduced_in: EntryId("db-sqlite-test".into()),
        resolved_in: None,
    };

    // Timestamp: Wednesday morning, ~3 days into the project.
    let wednesday_secs: u64 = 3 * 24 * 3600;
    registry.introduce(tension, wednesday_secs);

    // Verify: the tension exists and is unresolved.
    let active = registry.active_tensions();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, TensionId("pg-vs-sqlite".into()));
    assert!(active[0].resolved_in.is_none());

    // Urgency starts low (just introduced).
    let urgency_now = registry.urgency_score(
        &TensionId("pg-vs-sqlite".into()),
        wednesday_secs,
    );
    assert!(
        urgency_now < 0.1,
        "urgency should be very low at introduction (got {urgency_now})"
    );

    // A week later, urgency has grown.
    let one_week_later = wednesday_secs + 7 * 24 * 3600;
    let urgency_later = registry.urgency_score(
        &TensionId("pg-vs-sqlite".into()),
        one_week_later,
    );
    assert!(
        urgency_later > urgency_now,
        "urgency should grow over time (was {urgency_now}, now {urgency_later})"
    );

    // After 14+ days, escalation kicks in.
    let three_weeks_later = wednesday_secs + 21 * 24 * 3600;
    let escalated = registry.escalate_overdue(three_weeks_later);
    assert_eq!(escalated.len(), 1, "the tension should be escalated");
    assert_eq!(escalated[0], TensionId("pg-vs-sqlite".into()));

    // Escalated tensions are now Critical.
    let tension = registry.get(&TensionId("pg-vs-sqlite".into())).unwrap();
    assert_eq!(tension.severity, TensionSeverity::Critical);
}

// ---------------------------------------------------------------------------
// Day 4: The AI Remembers (for someone else) -- gossip sync
// ---------------------------------------------------------------------------

#[test]
fn day_4_teammate_gets_context_via_gossip() {
    let alice = AgentId("alice".into());
    let bob = AgentId("bob".into());

    let mut alice_engine = GossipEngine::new(alice.clone());
    let mut bob_engine = GossipEngine::new(bob.clone());

    // Alice has been working all week -- she has 4 memories.
    let entries = vec![
        task_entry(
            "db-setup", "alice",
            "Set up PostgreSQL database layer with migrations.",
            &["database", "postgresql"], "INFRA.DB.SETUP",
        ),
        task_entry(
            "db-pooling", "alice",
            "Added connection pooling to the PostgreSQL database layer.",
            &["database", "connection-pooling"], "INFRA.DB.POOL",
        ),
        task_entry(
            "db-logging", "alice",
            "Added query logging to the database layer.",
            &["database", "logging"], "INFRA.DB.LOG",
        ),
        task_entry(
            "db-sqlite", "alice",
            "Switched to SQLite for the test environment.",
            &["database", "sqlite", "testing"], "INFRA.DB.TEST",
        ),
    ];

    for entry in entries {
        alice_engine.ingest_local(entry);
    }

    // Bob has nothing. He sends a gossip request.
    let request = bob_engine.build_request();

    // Alice responds with everything Bob is missing.
    let response = alice_engine.handle_request(&request);
    assert_eq!(
        response.entries.len(),
        4,
        "Alice should send all 4 entries to Bob"
    );

    // Bob processes the response.
    let updated = bob_engine.process_response(response);
    assert_eq!(
        updated.len(),
        4,
        "Bob should have received all 4 entries"
    );

    // Bob can now retrieve any of Alice's memories.
    let setup = bob_engine.get(&EntryId("db-setup".into()));
    assert!(setup.is_some(), "Bob should have the db-setup entry");
    assert!(
        setup.unwrap().content.contains("PostgreSQL"),
        "the entry content should be intact"
    );

    // Bob can retrieve the SQLite entry too -- including the tension context.
    let sqlite = bob_engine.get(&EntryId("db-sqlite".into()));
    assert!(sqlite.is_some(), "Bob should have the db-sqlite entry");
}

// ---------------------------------------------------------------------------
// Day 5: Things Get Old -- lifecycle transitions
// ---------------------------------------------------------------------------

#[test]
fn day_5_memories_age_and_transition() {
    let store = InMemoryStore::new();

    // The connection pooling entry has been accessed many times -- still fresh.
    let mut pooling = task_entry(
        "db-pooling", "alice",
        "Connection pooling with configurable max connections.",
        &["database", "connection-pooling"], "INFRA.DB.POOL",
    );
    pooling.access_count = 15;
    pooling.survival.current_probability = 0.80; // healthy

    // The query logging entry has barely been touched -- it is aging.
    let mut logging = task_entry(
        "db-logging", "alice",
        "Query logging with structured output.",
        &["database", "logging"], "INFRA.DB.LOG",
    );
    logging.access_count = 2;
    logging.survival.current_probability = 0.18; // below 0.25 threshold

    // The initial setup note is ancient and barely surviving.
    let mut old_note = task_entry(
        "db-initial-note", "alice",
        "Initial brainstorming about database choices.",
        &["database", "notes"], "INFRA.DB.NOTES",
    );
    old_note.access_count = 0;
    old_note.survival.current_probability = 0.05; // below 0.10 threshold

    store.store(&pooling).unwrap();
    store.store(&logging).unwrap();
    store.store(&old_note).unwrap();

    // Run the lifecycle audit.
    let transitions = audit_lifecycle(&store).unwrap();

    // Verify: the logging entry became Moribund (0.10 <= 0.18 < 0.25).
    let logging_result = transitions.iter().find(|r| r.entry_id.0 == "db-logging");
    assert!(logging_result.is_some(), "logging entry should have transitioned");
    assert_eq!(logging_result.unwrap().new_state, MemoryState::Moribund);

    // Verify: the old note became Deceased (0.05 < 0.10).
    let note_result = transitions.iter().find(|r| r.entry_id.0 == "db-initial-note");
    assert!(note_result.is_some(), "old note should have transitioned");
    assert_eq!(note_result.unwrap().new_state, MemoryState::Deceased);

    // Verify: the pooling entry did NOT transition (0.80 >= 0.25).
    let pooling_result = transitions.iter().find(|r| r.entry_id.0 == "db-pooling");
    assert!(pooling_result.is_none(), "pooling entry should remain Alive");

    // Verify: retrieval ranks Alive entries above Moribund ones.
    let graph = SeeAlsoGraph::new(10);
    let engine = RetrievalEngine::new(store, graph);
    let results = engine
        .retrieve("database", 10, &RelevanceWeights::default())
        .unwrap();

    // The alive entry (pooling) should rank higher than the moribund one (logging).
    let alive_entries: Vec<_> = results.iter()
        .filter(|r| r.entry.state == MemoryState::Alive)
        .collect();
    let moribund_entries: Vec<_> = results.iter()
        .filter(|r| r.entry.state == MemoryState::Moribund)
        .collect();

    assert!(!alive_entries.is_empty(), "should have alive results");
    assert!(!moribund_entries.is_empty(), "should have moribund results");

    // Alive entries should score higher because their survival_probability is higher.
    let best_alive_score = alive_entries.iter().map(|r| r.score).fold(0.0_f64, f64::max);
    let best_moribund_score = moribund_entries.iter().map(|r| r.score).fold(0.0_f64, f64::max);
    assert!(
        best_alive_score > best_moribund_score,
        "alive entries should rank above moribund (alive={best_alive_score}, moribund={best_moribund_score})"
    );
}

// ---------------------------------------------------------------------------
// Day 6: The Budget Crunch -- graceful degradation
// ---------------------------------------------------------------------------

#[test]
fn day_6_budget_degrades_gracefully() {
    // Fresh budget: Full mode -- everything runs.
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget_mode(&budget), BudgetMode::Full);
    assert!(should_proceed(&budget, TaskPhase::Implement));
    assert!(should_proceed(&budget, TaskPhase::Validate));
    assert!(should_proceed(&budget, TaskPhase::Catalog));
    assert!(should_proceed(&budget, TaskPhase::Coordinate));

    // 65% used: MinimumOutput -- validation is skipped.
    let mut tight_budget = TokenBudget::new(20_000);
    tight_budget.used = 13_000;
    assert_eq!(budget_mode(&tight_budget), BudgetMode::MinimumOutput);
    assert!(
        should_proceed(&tight_budget, TaskPhase::Implement),
        "implementation still runs in MinimumOutput"
    );
    assert!(
        !should_proceed(&tight_budget, TaskPhase::Validate),
        "validation is skipped in MinimumOutput"
    );

    // Critical: Catalog and Coordinate ALWAYS proceed.
    assert!(
        should_proceed(&tight_budget, TaskPhase::Catalog),
        "catalog is never skipped -- mandatory reserve"
    );
    assert!(
        should_proceed(&tight_budget, TaskPhase::Coordinate),
        "coordination is never skipped -- mandatory reserve"
    );

    // 90% used: EmergencyHalt -- only catalog and coordinate.
    let mut exhausted = TokenBudget::new(10_000);
    exhausted.used = 9_000;
    assert_eq!(budget_mode(&exhausted), BudgetMode::EmergencyHalt);
    assert!(
        !should_proceed(&exhausted, TaskPhase::Implement),
        "implementation is blocked in EmergencyHalt"
    );
    assert!(
        !should_proceed(&exhausted, TaskPhase::Validate),
        "validation is blocked in EmergencyHalt"
    );
    assert!(
        should_proceed(&exhausted, TaskPhase::Catalog),
        "catalog ALWAYS runs, even in EmergencyHalt"
    );
    assert!(
        should_proceed(&exhausted, TaskPhase::Coordinate),
        "coordination ALWAYS runs, even in EmergencyHalt"
    );

    // Verify: mandatory reserves are protected from work consumption.
    let budget_with_reserves = TokenBudget::new(10_000);
    let work_tokens = budget_with_reserves.available_for_work();
    let reserved = budget_with_reserves.catalog_reserve + budget_with_reserves.coordination_reserve;
    assert_eq!(
        work_tokens,
        budget_with_reserves.total - reserved,
        "available_for_work should exclude mandatory reserves"
    );
}

// ---------------------------------------------------------------------------
// Day 7: Looking Back -- the whole week in review
// ---------------------------------------------------------------------------

#[test]
fn day_7_week_in_review() {
    // Build the full week of memories.
    let store = InMemoryStore::new();
    let mut graph = SeeAlsoGraph::new(10);
    let mut motif_index = MotifIndex::new();
    let mut tension_registry = TensionRegistry::new();

    let motif_id = MotifId("database-infrastructure".into());

    // -- Monday: setup + pooling --
    let setup = task_entry(
        "db-setup", "alice",
        "Set up PostgreSQL database layer with migrations and schema.",
        &["database", "postgresql", "setup"],
        "INFRA.DB.SETUP",
    );
    let pooling = task_entry(
        "db-pooling", "alice",
        "Added connection pooling to the PostgreSQL database layer.",
        &["database", "postgresql", "connection-pooling"],
        "INFRA.DB.POOL",
    );

    store.store(&setup).unwrap();
    store.store(&pooling).unwrap();
    graph.add_link(
        EntryId("db-setup".into()),
        EntryId("db-pooling".into()),
        Relationship::RelatedTo,
        "pooling builds on setup".into(),
    );
    motif_index.record_appearance(&motif_id, "database infrastructure", &EntryId("db-setup".into()));
    motif_index.record_appearance(&motif_id, "database infrastructure", &EntryId("db-pooling".into()));

    // -- Tuesday: query logging (motif emerges) --
    let logging = task_entry(
        "db-logging", "alice",
        "Added query logging to the database layer with structured log output.",
        &["database", "logging", "observability"],
        "INFRA.DB.LOG",
    );
    store.store(&logging).unwrap();
    graph.add_link(
        EntryId("db-pooling".into()),
        EntryId("db-logging".into()),
        Relationship::RelatedTo,
        "logging observes pooled connections".into(),
    );
    let promoted = motif_index.record_appearance(
        &motif_id,
        "database infrastructure",
        &EntryId("db-logging".into()),
    );
    assert!(promoted, "third appearance should promote the motif");

    // -- Wednesday: SQLite tension --
    let mut sqlite = task_entry(
        "db-sqlite", "alice",
        "Switched test environment to SQLite for faster test execution.",
        &["database", "sqlite", "testing"],
        "INFRA.DB.TEST",
    );
    let tension = Tension {
        id: TensionId("pg-vs-sqlite".into()),
        description: "PostgreSQL in production vs SQLite in tests".into(),
        severity: TensionSeverity::Low,
        introduced_in: EntryId("db-sqlite".into()),
        resolved_in: None,
    };
    sqlite.tension_refs.push(TensionRef {
        tension_id: TensionId("pg-vs-sqlite".into()),
        role: TensionRole::Introduced,
    });
    store.store(&sqlite).unwrap();
    tension_registry.introduce(tension, 3 * 24 * 3600);
    motif_index.record_appearance(&motif_id, "database infrastructure", &EntryId("db-sqlite".into()));

    // -- Verify the week --

    // 1. All entries are present.
    let all_ids = store.list(None).unwrap();
    assert_eq!(all_ids.len(), 4, "should have 4 entries for the week");

    // 2. Motif emerged and covers all entries.
    assert!(motif_index.is_emerged(&motif_id));
    let motif = motif_index.get(&motif_id).unwrap();
    assert_eq!(motif.appearances.len(), 4, "motif should span all 4 entries");

    // 3. Tension exists and is active.
    let active_tensions = tension_registry.active_tensions();
    assert_eq!(active_tensions.len(), 1);
    assert_eq!(active_tensions[0].description, "PostgreSQL in production vs SQLite in tests");

    // 4. See-also graph connects the entries.
    let setup_links = graph.get_links(&EntryId("db-setup".into()));
    assert_eq!(setup_links.len(), 1, "setup should link to pooling");

    let pooling_links = graph.get_links(&EntryId("db-pooling".into()));
    assert_eq!(pooling_links.len(), 2, "pooling should link to setup and logging");

    // 5. Retrieval finds everything for a "database" query.
    let engine = RetrievalEngine::new(store, graph);
    let results = engine
        .retrieve("database", 10, &RelevanceWeights::default())
        .unwrap();
    assert_eq!(results.len(), 4, "all 4 database entries should be retrieved");

    // 6. The entry with the tension reference has a tension boost.
    let sqlite_result = results.iter().find(|r| r.entry.id.0 == "db-sqlite").unwrap();
    assert!(
        sqlite_result.breakdown.tension_boost > 0.0,
        "SQLite entry should have a tension boost from its unresolved tension"
    );

    // 7. All entries start as Alive.
    for result in &results {
        assert_eq!(
            result.entry.state,
            MemoryState::Alive,
            "all entries should be Alive at the end of the week"
        );
    }

    // 8. Lifecycle states match survival probabilities.
    assert_eq!(state_for_probability(0.95), MemoryState::Alive);
    assert_eq!(state_for_probability(0.20), MemoryState::Moribund);
    assert_eq!(state_for_probability(0.05), MemoryState::Deceased);
}
