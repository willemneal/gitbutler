//! The Case of the Dying Memories -- a WEAVE Protocol mystery in 5 acts.
//!
//! Each test function is a piece of evidence in the investigation.
//! Run with: `cargo test -p but-ai --test tutorial_mystery`

use but_ai::agent::{budget_mode, BudgetMode};
use but_ai::agent::budget::should_proceed;
use but_ai::coordination::gossip::{GossipEngine, VectorClock};
use but_ai::memory::lifecycle::{state_for_probability, DECEASED_THRESHOLD, MORIBUND_THRESHOLD};
use but_ai::memory::retrieval::RetrievalEngine;
use but_ai::memory::see_also::SeeAlsoGraph;
use but_ai::memory::store::InMemoryStore;
use but_ai::narrative::motif::MotifIndex;
use but_ai::narrative::tension::TensionRegistry;
use but_ai::types::*;

// ---------------------------------------------------------------------------
// Helpers -- the detective's toolkit
// ---------------------------------------------------------------------------

/// Build a memory entry. Every detective needs a way to reconstruct the scene.
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

/// Build an entry with specific subject headings (for retrieval tests).
fn make_classified_entry(id: &str, content: &str, headings: &[&str]) -> MemoryEntry {
    let mut entry = make_entry(id, content);
    entry.classification.subject_headings =
        headings.iter().map(|h| h.to_string()).collect();
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
// ACT 1: THE PREMATURE DEATH (survival/distributions)
// ===========================================================================

#[test]
fn act1_weibull_survival_at_day_3() {
    // The victim: a memory about "database connection pooling".
    // Fitted with Weibull(k=2.0, lambda=100).
    // Found deceased after only 3 days. Was this death natural?
    let dist = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };

    let s_at_3 = dist.survival_probability(3.0);

    // S(3) should be very close to 1.0 -- this memory should be ALIVE.
    assert!(
        s_at_3 > 0.99,
        "S(3) = {s_at_3:.6}. The memory was murdered -- it should be alive at day 3!"
    );
}

#[test]
fn act1_weibull_curve_over_time() {
    // Reconstruct the correct survival curve for Weibull(k=2.0, lambda=100).
    let dist = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };

    let checkpoints = [
        (3.0, 0.999),  // Day 3: practically immortal
        (30.0, 0.91),  // Day 30: still very healthy
        (100.0, 0.36), // Day 100: past the median, declining
        (200.0, 0.01), // Day 200: almost certainly deceased
    ];

    for (day, expected_min) in checkpoints {
        let s = dist.survival_probability(day);
        assert!(
            s > expected_min,
            "Day {day}: S(t) = {s:.4}, expected > {expected_min}"
        );
    }

    // The curve should be monotonically decreasing.
    let mut prev = 1.0;
    for day in 1..=300 {
        let s = dist.survival_probability(day as f64);
        assert!(s <= prev + 1e-10, "S(t) increased at day {day}!");
        prev = s;
    }
}

#[test]
fn act1_swapped_parameters_produce_wrong_curve() {
    // THE REVEAL: someone swapped k and lambda!
    let correct = SurvivalDistribution::Weibull {
        k: 2.0,
        lambda: 100.0,
    };
    let swapped = SurvivalDistribution::Weibull {
        k: 100.0,
        lambda: 2.0,
    };

    // With correct parameters, the memory lives well past day 3.
    assert!(correct.survival_probability(3.0) > 0.99);

    // With swapped parameters, it dies almost instantly.
    assert!(
        swapped.survival_probability(3.0) < 0.001,
        "Swapped parameters kill the memory by day 3 -- case closed!"
    );
}

#[test]
fn act1_lifecycle_state_from_survival() {
    // The coroner's report: how S(t) maps to lifecycle state.
    assert_eq!(state_for_probability(0.999), MemoryState::Alive);
    assert_eq!(state_for_probability(MORIBUND_THRESHOLD), MemoryState::Alive);
    assert_eq!(state_for_probability(0.20), MemoryState::Moribund);
    assert_eq!(state_for_probability(DECEASED_THRESHOLD), MemoryState::Moribund);
    assert_eq!(state_for_probability(0.05), MemoryState::Deceased);
}

// ===========================================================================
// ACT 2: THE MISSING CONNECTION (memory/see_also + retrieval)
// ===========================================================================

#[test]
fn act2_isolated_memories_score_zero_see_also() {
    // Two memories that SHOULD know each other: JWT auth and session management.
    let store = InMemoryStore::new();
    let jwt = make_classified_entry("jwt", "JWT authentication token validation", &["authentication"]);
    let session = make_classified_entry("session", "session management cookie handling", &["session"]);
    store.store(&jwt).unwrap();
    store.store(&session).unwrap();

    // Without any see-also links, they're strangers.
    let graph = SeeAlsoGraph::new(5);
    assert_eq!(
        graph.distance_score(&EntryId("jwt".into()), &EntryId("session".into())),
        0.0,
        "No path exists -- they're complete strangers."
    );
}

#[test]
fn act2_linked_memories_boost_retrieval() {
    // Add a see-also link and watch the score change.
    let mut graph = SeeAlsoGraph::new(5);
    let jwt_id = EntryId("jwt".into());
    let session_id = EntryId("session".into());

    graph.add_link(
        jwt_id.clone(),
        session_id.clone(),
        Relationship::RelatedTo,
        "Both handle user identity".into(),
    );

    // Direct link = 1 hop -> score = 1/(1+1) = 0.5
    let score = graph.distance_score(&jwt_id, &session_id);
    assert!(
        (score - 0.5).abs() < 0.01,
        "Direct link should give 0.5, got {score}"
    );
}

#[test]
fn act2_retrieval_with_see_also_graph() {
    // Full retrieval test: linked entries score higher than unlinked ones.
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

    // JWT should be first (direct keyword match).
    assert_eq!(results[0].entry.id.0, "jwt");

    // Session should score higher than unrelated thanks to see-also link.
    let session_result = results.iter().find(|r| r.entry.id.0 == "session");
    let unrelated_result = results.iter().find(|r| r.entry.id.0 == "unrelated");
    assert!(
        session_result.unwrap().breakdown.see_also_distance
            > unrelated_result.unwrap().breakdown.see_also_distance,
        "Session should get a see-also boost from its link to JWT."
    );
}

// ===========================================================================
// ACT 3: THE PATTERN NO ONE NOTICED (narrative/motif)
// ===========================================================================

#[test]
fn act3_single_appearance_stays_proto() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    // First sighting: a proto-motif is born, but nobody notices.
    let promoted = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("task-1".into()),
    );

    assert!(!promoted, "One sighting does not make a motif.");
    assert_eq!(index.motif_count(), 0);
    assert_eq!(index.proto_motif_count(), 1);
    assert!(!index.is_emerged(&motif_id));
}

#[test]
fn act3_two_appearances_still_proto() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-1".into()));
    let promoted = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("task-2".into()),
    );

    assert!(!promoted, "Two sightings -- tantalizingly close, but not enough.");
    assert_eq!(index.motif_count(), 0);
    assert_eq!(index.proto_motif_count(), 1);
}

#[test]
fn act3_three_appearances_motif_emerges() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-1".into()));
    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-2".into()));
    let promoted = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("task-3".into()),
    );

    assert!(promoted, "Three sightings -- the motif EMERGES!");
    assert_eq!(index.motif_count(), 1);
    assert_eq!(index.proto_motif_count(), 0);
    assert!(index.is_emerged(&motif_id));

    // Verify the motif has all three appearances.
    let motif = index.get(&motif_id).unwrap();
    assert_eq!(motif.appearances.len(), 3);
}

#[test]
fn act3_emerged_motif_has_resonance() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    // Build the motif to emergence.
    for i in 1..=3 {
        index.record_appearance(
            &motif_id,
            "error handling patterns",
            &EntryId(format!("task-{i}")),
        );
    }

    // Create a memory entry that carries this motif.
    let mut entry = make_entry("evidence", "robust error handling in auth module");
    entry.motifs.push(motif_id.clone());

    // Query for "error handling" -- the emerged motif resonates.
    let resonance = index.resonance("error handling", &entry);
    assert!(
        resonance > 0.0,
        "Emerged motif should resonate with matching query. Got {resonance}"
    );
}

#[test]
fn act3_proto_motif_resonates_at_reduced_weight() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    // Only 1 appearance -- still a proto-motif.
    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-1".into()));

    let mut entry = make_entry("evidence", "error handling");
    entry.motifs.push(motif_id.clone());

    // Proto-motifs still resonate, but at 0.3x weight.
    let resonance = index.resonance("error handling", &entry);
    assert!(
        resonance > 0.0,
        "Proto-motifs resonate too, just at reduced weight. Got {resonance}"
    );

    // Now promote it and compare.
    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-2".into()));
    index.record_appearance(&motif_id, "error handling patterns", &EntryId("task-3".into()));

    let full_resonance = index.resonance("error handling", &entry);
    assert!(
        full_resonance >= resonance,
        "Emerged motif resonance ({full_resonance}) should be >= proto ({resonance})"
    );
}

// ===========================================================================
// ACT 4: THE CONTRADICTION COVER-UP (narrative/tension)
// ===========================================================================

#[test]
fn act4_tension_urgency_grows_over_time() {
    let mut registry = TensionRegistry::new();
    let tension_id = TensionId("sync-vs-async".into());

    // The contradiction: sync vs async database calls.
    let tension = Tension {
        id: tension_id.clone(),
        description: "Synchronous vs asynchronous database calls".into(),
        severity: TensionSeverity::Moderate,
        introduced_in: EntryId("memory-a".into()),
        resolved_in: None,
    };

    let t0 = 1_000_000u64; // some baseline timestamp
    registry.introduce(tension, t0);

    // At t=0 (introduction): urgency should be very low.
    let u0 = registry.urgency_score(&tension_id, t0);
    assert!(u0 < 0.01, "Urgency at introduction should be near zero, got {u0}");

    // After 7 days: urgency growing.
    let seven_days = 7 * 24 * 3600;
    let u7 = registry.urgency_score(&tension_id, t0 + seven_days);
    assert!(
        u7 > u0,
        "Urgency should grow over time. Day 0: {u0}, Day 7: {u7}"
    );

    // After 14 days: urgency significant (Weibull lambda=14 days).
    let fourteen_days = 14 * 24 * 3600;
    let u14 = registry.urgency_score(&tension_id, t0 + fourteen_days);
    assert!(
        u14 > u7,
        "Day 14 urgency ({u14}) should exceed day 7 ({u7})"
    );
    assert!(
        u14 > 0.3,
        "By day 14, urgency should be substantial. Got {u14}"
    );
}

#[test]
fn act4_resolved_tension_has_zero_urgency() {
    let mut registry = TensionRegistry::new();
    let tension_id = TensionId("sync-vs-async".into());

    let tension = Tension {
        id: tension_id.clone(),
        description: "Synchronous vs asynchronous database calls".into(),
        severity: TensionSeverity::High,
        introduced_in: EntryId("memory-a".into()),
        resolved_in: None,
    };

    let t0 = 1_000_000u64;
    registry.introduce(tension, t0);

    // Resolve the tension.
    let resolved = registry.resolve(&tension_id, &EntryId("memory-resolution".into()));
    assert!(resolved, "Resolution should succeed.");

    // Urgency drops to zero after resolution, no matter how old.
    let thirty_days = 30 * 24 * 3600;
    let u = registry.urgency_score(&tension_id, t0 + thirty_days);
    assert!(
        u < f64::EPSILON,
        "Resolved tensions have zero urgency. Got {u}"
    );
}

#[test]
fn act4_escalation_at_14_days() {
    let mut registry = TensionRegistry::new();
    let tension_id = TensionId("sync-vs-async".into());

    let tension = Tension {
        id: tension_id.clone(),
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
    assert_eq!(escalated[0], tension_id);

    // Verify severity was upgraded to Critical.
    let t = registry.get(&tension_id).unwrap();
    assert_eq!(t.severity, TensionSeverity::Critical);
}

// ===========================================================================
// ACT 5: THE GOSSIP THAT WENT WRONG (coordination/gossip)
// ===========================================================================

#[test]
fn act5_vector_clock_tracks_observations() {
    let mut clock = VectorClock::new();
    let agent_a = AgentId("agent-a".into());

    // No observations yet.
    assert_eq!(clock.version_for(&agent_a), 0);

    // Observe version 3.
    clock.observe(&agent_a, 3);
    assert_eq!(clock.version_for(&agent_a), 3);

    // Clocks never go backward.
    clock.observe(&agent_a, 1);
    assert_eq!(
        clock.version_for(&agent_a),
        3,
        "Vector clocks never regress!"
    );
}

#[test]
fn act5_gossip_last_writer_wins() {
    // Agent A has an old version of entry "e1" (access_count=1).
    // Agent B has a new version of entry "e1" (access_count=5).
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    // Both agents have entry "e1", but B's version is more accessed.
    let old_entry = make_gossip_entry("e1", "agent-a", 1);
    let new_entry = make_gossip_entry("e1", "agent-b", 5);

    engine_a.ingest_local(old_entry);
    engine_b.ingest_local(new_entry);

    // A requests gossip from B.
    let request = engine_a.build_request();
    let response = engine_b.handle_request(&request);

    // A processes B's response.
    let updated = engine_a.process_response(response);

    // Last-writer-wins: B's version (access_count=5) overwrites A's (access_count=1).
    assert_eq!(updated.len(), 1, "One entry should have been updated.");
    let merged = engine_a.get(&EntryId("e1".into())).unwrap();
    assert_eq!(
        merged.access_count, 5,
        "Higher access count wins. Got {}",
        merged.access_count
    );
}

#[test]
fn act5_bidirectional_gossip_converges() {
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    // A has entry "e1", B has entry "e2".
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

    // CONVERGENCE: both agents now have both entries.
    assert!(
        engine_a.get(&EntryId("e1".into())).is_some(),
        "A should still have e1"
    );
    assert!(
        engine_a.get(&EntryId("e2".into())).is_some(),
        "A should have acquired e2"
    );
    assert!(
        engine_b.get(&EntryId("e1".into())).is_some(),
        "B should have acquired e1"
    );
    assert!(
        engine_b.get(&EntryId("e2".into())).is_some(),
        "B should still have e2"
    );
}

#[test]
fn act5_vector_clocks_advance_after_gossip() {
    let agent_a = AgentId("agent-a".into());
    let agent_b = AgentId("agent-b".into());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    engine_a.ingest_local(make_gossip_entry("e1", "agent-a", 1));
    engine_b.ingest_local(make_gossip_entry("e2", "agent-b", 1));

    // Before gossip: each only knows about itself.
    assert_eq!(engine_a.clock().version_for(&agent_a), 1);
    assert_eq!(engine_a.clock().version_for(&agent_b), 0);

    // A gossips with B.
    let req = engine_a.build_request();
    let resp = engine_b.handle_request(&req);
    engine_a.process_response(resp);

    // After gossip: A's clock now knows about B.
    assert_eq!(
        engine_a.clock().version_for(&agent_b),
        1,
        "A's clock should now track B's version"
    );
}

// ===========================================================================
// EPILOGUE: THE BUDGET THAT SAVED US ALL (agent/budget)
// ===========================================================================

#[test]
fn epilogue_budget_modes_at_thresholds() {
    // Fresh budget: full protocol.
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget_mode(&budget), BudgetMode::Full);

    // 90% consumed (10% remaining < 20%): EmergencyHalt.
    let mut budget_90 = TokenBudget::new(32_000);
    budget_90.used = 28_800; // 90%
    assert_eq!(budget_mode(&budget_90), BudgetMode::EmergencyHalt);
    // Validation is skipped in emergency.
    assert!(!should_proceed(&budget_90, TaskPhase::Validate));
    // But implementation is also blocked.
    assert!(!should_proceed(&budget_90, TaskPhase::Implement));
}

#[test]
fn epilogue_catalog_always_runs() {
    // Even at 96% usage, cataloging and coordination still run.
    let mut budget = TokenBudget::new(32_000);
    budget.used = 30_720; // 96%

    assert_eq!(budget_mode(&budget), BudgetMode::EmergencyHalt);
    assert!(
        should_proceed(&budget, TaskPhase::Catalog),
        "An unclassified result is a lost result."
    );
    assert!(
        should_proceed(&budget, TaskPhase::Coordinate),
        "Coordination is mandatory -- the team must know."
    );
}

#[test]
fn epilogue_reserves_protect_essential_phases() {
    // The reserves: 1,500 for catalog, 2,000 for coordination = 3,500 total.
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget.catalog_reserve, 1_500);
    assert_eq!(budget.coordination_reserve, 2_000);

    // Available-for-work excludes reserves.
    let available = budget.available_for_work();
    assert_eq!(
        available,
        32_000 - 1_500 - 2_000,
        "Work budget = total - reserves"
    );
}
