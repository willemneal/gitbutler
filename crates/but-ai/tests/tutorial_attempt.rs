//! Tutorial attempt: "The Case of the Dying Memories"
//!
//! Written by a first-time reader of the but-ai crate, following
//! the tutorial's instructions and examples.
//!
//! Run with: `cargo test -p but-ai --test tutorial_attempt`

use but_ai::agent::budget::should_proceed;
use but_ai::agent::{budget_mode, BudgetMode};
use but_ai::coordination::gossip::{GossipEngine, VectorClock};
use but_ai::memory::lifecycle::{state_for_probability, DECEASED_THRESHOLD, MORIBUND_THRESHOLD};
use but_ai::memory::retrieval::RetrievalEngine;
use but_ai::memory::see_also::SeeAlsoGraph;
use but_ai::memory::store::InMemoryStore;
use but_ai::narrative::motif::MotifIndex;
use but_ai::narrative::tension::TensionRegistry;
use but_ai::types::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reconstruct a basic memory entry for testing.
fn make_entry(id: &str, content: &str) -> MemoryEntry {
    MemoryEntry {
        id: EntryId(id.into()),
        agent: AgentId("detective".into()),
        content: content.into(),
        created_at: "2026-03-29T00:00:00Z".into(),
        last_accessed: "2026-03-29T00:00:00Z".into(),
        classification: Classification {
            subject_headings: vec![],
            call_number: CallNumber::parse("CASE.EVIDENCE"),
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

/// Build an entry with specific subject headings for retrieval tests.
fn make_classified_entry(id: &str, content: &str, headings: &[&str]) -> MemoryEntry {
    let mut entry = make_entry(id, content);
    entry.classification.subject_headings = headings.iter().map(|h| h.to_string()).collect();
    entry
}

/// Build a gossip-ready entry with a specific agent and access count.
fn make_gossip_entry(id: &str, agent: &str, access_count: u64) -> MemoryEntry {
    let mut entry = make_entry(id, &format!("gossip-content-{id}"));
    entry.agent = AgentId(agent.into());
    entry.access_count = access_count;
    entry
}

// ===========================================================================
// Act 1: The Premature Death (survival/distributions)
// ===========================================================================

/// A Weibull(k=2.0, lambda=100) memory should be practically immortal at day 3.
/// S(3) = exp(-(3/100)^2) ~ 0.9991
#[test]
fn act1_weibull_survival_at_day_3() {
    let dist = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };
    let s = dist.survival_probability(3.0);
    assert!(
        s > 0.99,
        "S(3) = {s:.6} -- the memory should be alive at day 3!"
    );
}

/// Trace the Weibull curve across several checkpoints to verify the shape.
#[test]
fn act1_weibull_curve_checkpoints() {
    let dist = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };

    // (day, minimum expected S(t))
    let checks = [
        (3.0, 0.999),
        (30.0, 0.91),
        (100.0, 0.36),
        (200.0, 0.01),
    ];

    for (day, min) in checks {
        let s = dist.survival_probability(day);
        assert!(s > min, "Day {day}: S(t) = {s:.4}, expected > {min}");
    }

    // Monotonic decrease check.
    let mut prev = 1.0;
    for day in 1..=300 {
        let s = dist.survival_probability(day as f64);
        assert!(s <= prev + 1e-10, "Survival increased at day {day}!");
        prev = s;
    }
}

/// Swapping k and lambda should kill the memory almost instantly.
#[test]
fn act1_swapped_parameters_kill_memory() {
    let correct = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };
    let swapped = SurvivalDistribution::Weibull {
        k: 100.0,
        lambda: 2.0,
    };

    assert!(correct.survival_probability(3.0) > 0.99);
    assert!(
        swapped.survival_probability(3.0) < 0.001,
        "Swapped parameters should kill the memory by day 3."
    );
}

/// The lifecycle module maps survival probabilities to three states.
#[test]
fn act1_lifecycle_state_mapping() {
    // >= 0.25 is Alive
    assert_eq!(state_for_probability(0.999), MemoryState::Alive);
    assert_eq!(state_for_probability(MORIBUND_THRESHOLD), MemoryState::Alive);

    // 0.10 <= sp < 0.25 is Moribund
    assert_eq!(state_for_probability(0.20), MemoryState::Moribund);
    assert_eq!(state_for_probability(DECEASED_THRESHOLD), MemoryState::Moribund);

    // < 0.10 is Deceased
    assert_eq!(state_for_probability(0.05), MemoryState::Deceased);
}

// ===========================================================================
// Act 2: The Missing Connection (memory/see_also, memory/retrieval)
// ===========================================================================

/// Two isolated memories should have zero see-also distance.
#[test]
fn act2_isolated_memories_zero_distance() {
    let store = InMemoryStore::new();
    let jwt = make_classified_entry("jwt", "JWT authentication token validation", &["authentication"]);
    let session = make_classified_entry("session", "session management cookie handling", &["session"]);
    store.store(&jwt).unwrap();
    store.store(&session).unwrap();

    let graph = SeeAlsoGraph::new(5);
    assert_eq!(
        graph.distance_score(&EntryId("jwt".into()), &EntryId("session".into())),
        0.0,
        "No path -- they are strangers."
    );
}

/// A direct see-also link should give a distance score of 0.5 = 1/(1+1).
#[test]
fn act2_direct_link_gives_half_score() {
    let mut graph = SeeAlsoGraph::new(5);
    let jwt_id = EntryId("jwt".into());
    let session_id = EntryId("session".into());

    graph.add_link(
        jwt_id.clone(),
        session_id.clone(),
        Relationship::RelatedTo,
        "Both handle user identity".into(),
    );

    let score = graph.distance_score(&jwt_id, &session_id);
    assert!(
        (score - 0.5).abs() < 0.01,
        "Direct link should score 0.5, got {score}"
    );
}

/// The retrieval engine should boost linked entries via see-also distance.
#[test]
fn act2_retrieval_boosts_linked_entries() {
    let store = InMemoryStore::new();
    let mut jwt = make_classified_entry("jwt", "JWT authentication token validation", &["authentication"]);
    jwt.classification.subject_headings.push("jwt".into());
    let session = make_classified_entry("session", "session management cookie handling", &["session"]);
    let unrelated = make_classified_entry("unrelated", "database migration scripts", &["database"]);

    store.store(&jwt).unwrap();
    store.store(&session).unwrap();
    store.store(&unrelated).unwrap();

    let mut graph = SeeAlsoGraph::new(5);
    graph.add_link(
        EntryId("jwt".into()),
        EntryId("session".into()),
        Relationship::RelatedTo,
        "Both handle user identity".into(),
    );

    let engine = RetrievalEngine::new(store, graph);
    let results = engine
        .retrieve("authentication", 10, &RelevanceWeights::default())
        .unwrap();

    // JWT should rank first (direct keyword match).
    assert_eq!(results[0].entry.id.0, "jwt");

    // Session should have higher see_also_distance than unrelated.
    let session_r = results.iter().find(|r| r.entry.id.0 == "session").unwrap();
    let unrelated_r = results.iter().find(|r| r.entry.id.0 == "unrelated").unwrap();
    assert!(
        session_r.breakdown.see_also_distance > unrelated_r.breakdown.see_also_distance,
        "Session should get a see-also boost from its link to JWT."
    );
}

// ===========================================================================
// Act 3: The Pattern No One Noticed (narrative/motif)
// ===========================================================================

/// One appearance creates a proto-motif, not a full motif.
#[test]
fn act3_one_appearance_is_proto() {
    let mut index = MotifIndex::new();
    let mid = MotifId("error-handling".into());

    let promoted = index.record_appearance(&mid, "error handling patterns", &EntryId("task-1".into()));
    assert!(!promoted, "Single sighting should not promote.");
    assert_eq!(index.motif_count(), 0);
    assert_eq!(index.proto_motif_count(), 1);
    assert!(!index.is_emerged(&mid));
}

/// Two appearances are still not enough.
#[test]
fn act3_two_appearances_still_proto() {
    let mut index = MotifIndex::new();
    let mid = MotifId("error-handling".into());

    index.record_appearance(&mid, "error handling patterns", &EntryId("task-1".into()));
    let promoted = index.record_appearance(&mid, "error handling patterns", &EntryId("task-2".into()));
    assert!(!promoted, "Two sightings still not enough.");
    assert_eq!(index.motif_count(), 0);
    assert_eq!(index.proto_motif_count(), 1);
}

/// Three appearances promote from proto-motif to emerged motif.
#[test]
fn act3_three_appearances_emerges() {
    let mut index = MotifIndex::new();
    let mid = MotifId("error-handling".into());

    index.record_appearance(&mid, "error handling patterns", &EntryId("task-1".into()));
    index.record_appearance(&mid, "error handling patterns", &EntryId("task-2".into()));
    let promoted = index.record_appearance(&mid, "error handling patterns", &EntryId("task-3".into()));

    assert!(promoted, "Third sighting should promote to full motif.");
    assert_eq!(index.motif_count(), 1);
    assert_eq!(index.proto_motif_count(), 0);
    assert!(index.is_emerged(&mid));

    let motif = index.get(&mid).unwrap();
    assert_eq!(motif.appearances.len(), 3);
}

/// An emerged motif should produce nonzero resonance with a matching query.
#[test]
fn act3_emerged_motif_resonance() {
    let mut index = MotifIndex::new();
    let mid = MotifId("error-handling".into());

    for i in 1..=3 {
        index.record_appearance(&mid, "error handling patterns", &EntryId(format!("task-{i}")));
    }

    let mut entry = make_entry("evidence", "robust error handling in auth module");
    entry.motifs.push(mid.clone());

    let resonance = index.resonance("error handling", &entry);
    assert!(resonance > 0.0, "Emerged motif should resonate. Got {resonance}");
}

/// Proto-motifs resonate at reduced (0.3x) weight.
#[test]
fn act3_proto_motif_reduced_resonance() {
    let mut index = MotifIndex::new();
    let mid = MotifId("error-handling".into());

    // Only 1 appearance -- proto-motif.
    index.record_appearance(&mid, "error handling patterns", &EntryId("task-1".into()));

    let mut entry = make_entry("evidence", "error handling");
    entry.motifs.push(mid.clone());

    let proto_resonance = index.resonance("error handling", &entry);
    assert!(proto_resonance > 0.0, "Proto should resonate at reduced weight. Got {proto_resonance}");

    // Promote it.
    index.record_appearance(&mid, "error handling patterns", &EntryId("task-2".into()));
    index.record_appearance(&mid, "error handling patterns", &EntryId("task-3".into()));

    let full_resonance = index.resonance("error handling", &entry);
    assert!(
        full_resonance >= proto_resonance,
        "Full resonance ({full_resonance}) should be >= proto ({proto_resonance})"
    );
}

// ===========================================================================
// Act 4: The Contradiction Cover-Up (narrative/tension)
// ===========================================================================

/// Tension urgency should grow over time following a Weibull CDF.
#[test]
fn act4_urgency_grows_over_time() {
    let mut registry = TensionRegistry::new();
    let tid = TensionId("sync-vs-async".into());

    let tension = Tension {
        id: tid.clone(),
        description: "Synchronous vs asynchronous database calls".into(),
        severity: TensionSeverity::Moderate,
        introduced_in: EntryId("memory-a".into()),
        resolved_in: None,
    };

    let t0 = 1_000_000u64;
    registry.introduce(tension, t0);

    // At introduction: urgency near zero.
    let u0 = registry.urgency_score(&tid, t0);
    assert!(u0 < 0.01, "Urgency at introduction should be near zero, got {u0}");

    // After 7 days: growing.
    let seven_days = 7 * 24 * 3600;
    let u7 = registry.urgency_score(&tid, t0 + seven_days);
    assert!(u7 > u0, "Day 7 ({u7}) should exceed day 0 ({u0})");

    // After 14 days: significant.
    let fourteen_days = 14 * 24 * 3600;
    let u14 = registry.urgency_score(&tid, t0 + fourteen_days);
    assert!(u14 > u7, "Day 14 ({u14}) should exceed day 7 ({u7})");
    assert!(u14 > 0.3, "By day 14 urgency should be substantial. Got {u14}");
}

/// Resolving a tension silences its urgency to zero.
#[test]
fn act4_resolution_zeroes_urgency() {
    let mut registry = TensionRegistry::new();
    let tid = TensionId("sync-vs-async".into());

    let tension = Tension {
        id: tid.clone(),
        description: "Synchronous vs asynchronous database calls".into(),
        severity: TensionSeverity::High,
        introduced_in: EntryId("memory-a".into()),
        resolved_in: None,
    };

    let t0 = 1_000_000u64;
    registry.introduce(tension, t0);

    let resolved = registry.resolve(&tid, &EntryId("memory-resolution".into()));
    assert!(resolved, "Resolution should succeed.");

    let thirty_days = 30 * 24 * 3600;
    let u = registry.urgency_score(&tid, t0 + thirty_days);
    assert!(u < f64::EPSILON, "Resolved tensions have zero urgency. Got {u}");
}

/// Unresolved tensions escalate to Critical after 14 days.
#[test]
fn act4_escalation_at_14_days() {
    let mut registry = TensionRegistry::new();
    let tid = TensionId("sync-vs-async".into());

    let tension = Tension {
        id: tid.clone(),
        description: "Synchronous vs asynchronous database calls".into(),
        severity: TensionSeverity::Moderate,
        introduced_in: EntryId("memory-a".into()),
        resolved_in: None,
    };

    let t0 = 1_000_000u64;
    registry.introduce(tension, t0);

    // Before 14 days: no escalation.
    let thirteen_days = 13 * 24 * 3600;
    let escalated = registry.escalate_overdue(t0 + thirteen_days);
    assert!(escalated.is_empty(), "Should not escalate before 14 days.");

    // At 14 days: escalation triggers.
    let fourteen_days = 14 * 24 * 3600;
    let escalated = registry.escalate_overdue(t0 + fourteen_days);
    assert_eq!(escalated.len(), 1, "Should escalate at 14 days.");
    assert_eq!(escalated[0], tid);

    let t = registry.get(&tid).unwrap();
    assert_eq!(t.severity, TensionSeverity::Critical);
}

// ===========================================================================
// Act 5: The Gossip That Went Wrong (coordination/gossip)
// ===========================================================================

/// Vector clocks track observations and never go backward.
#[test]
fn act5_vector_clock_monotonic() {
    let mut clock = VectorClock::new();
    let agent_a = AgentId("agent-a".into());

    assert_eq!(clock.version_for(&agent_a), 0);
    clock.observe(&agent_a, 3);
    assert_eq!(clock.version_for(&agent_a), 3);

    // Try to go backward -- should have no effect.
    clock.observe(&agent_a, 1);
    assert_eq!(clock.version_for(&agent_a), 3, "Vector clocks never regress!");
}

/// The gossip protocol uses last-writer-wins (higher access_count).
#[test]
fn act5_last_writer_wins() {
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    let old_entry = make_gossip_entry("e1", "agent-a", 1);
    let new_entry = make_gossip_entry("e1", "agent-b", 5);

    engine_a.ingest_local(old_entry);
    engine_b.ingest_local(new_entry);

    let request = engine_a.build_request();
    let response = engine_b.handle_request(&request);
    let updated = engine_a.process_response(response);

    assert_eq!(updated.len(), 1, "One entry should have been updated.");
    let merged = engine_a.get(&EntryId("e1".into())).unwrap();
    assert_eq!(merged.access_count, 5, "Higher access count should win. Got {}", merged.access_count);
}

/// Bidirectional gossip rounds achieve full CRDT convergence.
#[test]
fn act5_bidirectional_convergence() {
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    engine_a.ingest_local(make_gossip_entry("e1", "agent-a", 3));
    engine_b.ingest_local(make_gossip_entry("e2", "agent-b", 7));

    // Round 1: A pulls from B.
    let req_a = engine_a.build_request();
    let resp_b = engine_b.handle_request(&req_a);
    engine_a.process_response(resp_b);

    // Round 2: B pulls from A.
    let req_b = engine_b.build_request();
    let resp_a = engine_a.handle_request(&req_b);
    engine_b.process_response(resp_a);

    // Both should now have both entries.
    assert!(engine_a.get(&EntryId("e1".into())).is_some());
    assert!(engine_a.get(&EntryId("e2".into())).is_some());
    assert!(engine_b.get(&EntryId("e1".into())).is_some());
    assert!(engine_b.get(&EntryId("e2".into())).is_some());
}

/// After gossip, vector clocks reflect what the agent learned from peers.
#[test]
fn act5_clocks_advance_after_gossip() {
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    engine_a.ingest_local(make_gossip_entry("e1", "agent-a", 1));
    engine_b.ingest_local(make_gossip_entry("e2", "agent-b", 1));

    // Before gossip.
    assert_eq!(engine_a.clock().version_for(&agent_a), 1);
    assert_eq!(engine_a.clock().version_for(&agent_b), 0);

    // A gossips with B.
    let req = engine_a.build_request();
    let resp = engine_b.handle_request(&req);
    engine_a.process_response(resp);

    // After gossip: A now knows about B's version.
    assert_eq!(engine_a.clock().version_for(&agent_b), 1, "A's clock should now track B's version");
}

// ===========================================================================
// Epilogue: The Budget That Saved Us All (agent/budget)
// ===========================================================================

/// At full budget: Full mode. At 90% used: EmergencyHalt, blocking both Validate and Implement.
#[test]
fn epilogue_budget_mode_thresholds() {
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget_mode(&budget), BudgetMode::Full);

    let mut budget_90 = TokenBudget::new(32_000);
    budget_90.used = 28_800; // 90% used -> 10% remaining -> EmergencyHalt
    assert_eq!(budget_mode(&budget_90), BudgetMode::EmergencyHalt);
    assert!(!should_proceed(&budget_90, TaskPhase::Validate));
    assert!(!should_proceed(&budget_90, TaskPhase::Implement));
}

/// Catalog and Coordinate always run, even in EmergencyHalt.
#[test]
fn epilogue_catalog_always_proceeds() {
    let mut budget = TokenBudget::new(32_000);
    budget.used = 30_720; // 96% used -> EmergencyHalt
    assert_eq!(budget_mode(&budget), BudgetMode::EmergencyHalt);
    assert!(should_proceed(&budget, TaskPhase::Catalog), "Catalog must always run.");
    assert!(should_proceed(&budget, TaskPhase::Coordinate), "Coordination must always run.");
}

/// Token reserves carve out space for catalog (1500) and coordination (2000).
#[test]
fn epilogue_reserves_calculation() {
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget.catalog_reserve, 1_500);
    assert_eq!(budget.coordination_reserve, 2_000);
    assert_eq!(budget.available_for_work(), 32_000 - 1_500 - 2_000);
}
