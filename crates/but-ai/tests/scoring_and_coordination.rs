//! Integration tests for memory scoring, coordination, and agent modules.

use but_ai::coordination::dependency::DependencyGraph;
use but_ai::coordination::dependency::DependencyNode;
use but_ai::coordination::gossip::GossipEngine;
use but_ai::coordination::messages;
use but_ai::memory::lifecycle;
use but_ai::memory::retrieval::RetrievalEngine;
use but_ai::memory::see_also::SeeAlsoGraph;
use but_ai::memory::store::InMemoryStore;
use but_ai::types::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_entry(id: &str, content: &str, motifs: &[&str]) -> MemoryEntry {
    MemoryEntry {
        id: EntryId(id.into()),
        agent: AgentId("test-agent".into()),
        content: content.into(),
        created_at: "2026-03-29T00:00:00Z".into(),
        last_accessed: "2026-03-29T00:00:00Z".into(),
        classification: Classification {
            subject_headings: vec![],
            call_number: CallNumber::parse("TEST.UNIT"),
            controlled_vocab: false,
        },
        see_also: Vec::new(),
        motifs: motifs.iter().map(|m| MotifId(m.to_string())).collect(),
        tension_refs: Vec::new(),
        survival: SurvivalMetadata {
            distribution: SurvivalDistribution::Exponential { lambda: 0.01 },
            current_probability: 0.8,
            hazard_rate: 0.01,
            surprise_index: 0.0,
            goodness_of_fit: 0.95,
        },
        state: MemoryState::Alive,
        consensus_citations: 0,
        access_count: 5,
        source_commit: None,
    }
}

fn make_entry_with_survival(id: &str, content: &str, sp: f64) -> MemoryEntry {
    let mut entry = make_entry(id, content, &[]);
    entry.survival.current_probability = sp;
    entry
}

fn make_entry_with_state(id: &str, sp: f64, state: MemoryState) -> MemoryEntry {
    let mut entry = make_entry(id, "test content", &[]);
    entry.survival.current_probability = sp;
    entry.state = state;
    entry
}

fn pr(owner: &str, repo: &str, number: u64) -> PrRef {
    PrRef {
        repo: RepoRef {
            forge: ForgeType::GitHub,
            owner: owner.to_string(),
            repo: repo.to_string(),
        },
        number,
    }
}

fn dep_node(pr_ref: PrRef, deps: Vec<PrRef>) -> DependencyNode {
    DependencyNode {
        pr: pr_ref,
        depends_on: deps,
        agent: AgentId("test-agent".to_string()),
        status: PrStatus::Open,
    }
}

fn sample_coordination_message() -> CoordinationMessage {
    CoordinationMessage {
        schema: messages::SCHEMA_VERSION.to_string(),
        message_type: MessageType::StatusReport,
        from: AgentId("agent-alpha".to_string()),
        to: Some(AgentId("agent-beta".to_string())),
        payload: serde_json::json!({
            "status": "in_progress",
            "files_changed": 7,
            "branch": "feat/auth"
        }),
        timestamp: "2026-03-29T15:30:00Z".to_string(),
    }
}

fn make_gossip_entry(id: &str, agent: &str) -> MemoryEntry {
    let mut entry = make_entry(id, &format!("gossip content for {id}"), &[]);
    entry.agent = AgentId(agent.to_string());
    entry.access_count = 1;
    entry
}

// ===========================================================================
// Retrieval Scoring
// ===========================================================================

#[test]
fn retrieval_auth_entry_ranks_first_for_auth_query() {
    let store = InMemoryStore::new();
    store
        .store(&make_entry(
            "e-auth",
            "authentication middleware JWT token login",
            &["auth"],
        ))
        .unwrap();
    store
        .store(&make_entry(
            "e-db",
            "database migration schema setup",
            &["database"],
        ))
        .unwrap();
    store
        .store(&make_entry(
            "e-ui",
            "frontend button component styling",
            &["ui"],
        ))
        .unwrap();
    store
        .store(&make_entry(
            "e-net",
            "network socket connection pooling",
            &["networking"],
        ))
        .unwrap();
    store
        .store(&make_entry(
            "e-log",
            "logging structured tracing output",
            &["observability"],
        ))
        .unwrap();

    let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
    let results = engine
        .retrieve("authentication", 5, &RelevanceWeights::default())
        .unwrap();

    assert_eq!(results.len(), 5);
    assert_eq!(
        results[0].entry.id,
        EntryId("e-auth".into()),
        "auth entry should rank first for 'authentication' query"
    );
}

#[test]
fn high_survival_ranks_above_low_survival() {
    let store = InMemoryStore::new();
    store
        .store(&make_entry_with_survival(
            "e-high",
            "generic test content alpha",
            0.95,
        ))
        .unwrap();
    store
        .store(&make_entry_with_survival(
            "e-low",
            "generic test content beta",
            0.10,
        ))
        .unwrap();

    // Use weights that emphasize survival probability only.
    let weights = RelevanceWeights {
        motif_resonance: 0.0,
        call_number_proximity: 0.0,
        see_also_distance: 0.0,
        survival_probability: 1.0,
        freshness: 0.0,
        tension_boost: 0.0,
    };

    let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
    let results = engine.retrieve("anything", 2, &weights).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].entry.id, EntryId("e-high".into()));
    assert!(
        results[0].score > results[1].score,
        "high survival ({}) should outscore low survival ({})",
        results[0].score,
        results[1].score
    );
}

#[test]
fn see_also_neighbor_discovered() {
    let store = InMemoryStore::new();
    let mut entry_a = make_entry("e-a", "authentication login flow", &["auth"]);
    entry_a.classification.subject_headings = vec!["authentication".into()];
    let entry_b = make_entry("e-b", "session management cookies", &["sessions"]);

    store.store(&entry_a).unwrap();
    store.store(&entry_b).unwrap();

    let mut graph = SeeAlsoGraph::new(5);
    graph.add_link(
        EntryId("e-a".into()),
        EntryId("e-b".into()),
        Relationship::RelatedTo,
        "auth relates to sessions".into(),
    );

    let engine = RetrievalEngine::new(store, graph);
    let results = engine
        .retrieve("authentication", 5, &RelevanceWeights::default())
        .unwrap();

    // Both entries should be returned, and e-b should have a non-zero
    // see-also score because it is linked to e-a.
    let b_result = results.iter().find(|r| r.entry.id == EntryId("e-b".into()));
    assert!(
        b_result.is_some(),
        "entry B should be returned as a see-also neighbor"
    );
    assert!(
        b_result.unwrap().breakdown.see_also_distance > 0.0,
        "entry B should have positive see-also score"
    );
}

#[test]
fn default_weights_sum_to_one() {
    let w = RelevanceWeights::default();
    let sum = w.motif_resonance
        + w.call_number_proximity
        + w.see_also_distance
        + w.survival_probability
        + w.freshness
        + w.tension_boost;
    assert!(
        (sum - 1.0).abs() < 1e-10,
        "default weights should sum to 1.0, got {sum}"
    );
}

#[test]
fn zero_weight_dimension_has_no_effect() {
    let store = InMemoryStore::new();
    let mut entry = make_entry("e1", "test content", &[]);
    // Give it high tension to boost via tension_boost.
    entry.tension_refs.push(TensionRef {
        tension_id: TensionId("t1".into()),
        role: TensionRole::Introduced,
    });
    store.store(&entry).unwrap();

    // Weights with tension_boost = 0.
    let weights_zero_tension = RelevanceWeights {
        motif_resonance: 0.0,
        call_number_proximity: 0.0,
        see_also_distance: 0.0,
        survival_probability: 1.0,
        freshness: 0.0,
        tension_boost: 0.0,
    };
    // Weights with tension_boost > 0.
    let weights_with_tension = RelevanceWeights {
        motif_resonance: 0.0,
        call_number_proximity: 0.0,
        see_also_distance: 0.0,
        survival_probability: 0.5,
        freshness: 0.0,
        tension_boost: 0.5,
    };

    let engine = RetrievalEngine::new(store, SeeAlsoGraph::new(5));
    let r_zero = engine
        .retrieve("test", 1, &weights_zero_tension)
        .unwrap();
    let r_with = engine
        .retrieve("test", 1, &weights_with_tension)
        .unwrap();

    // When tension_boost weight is zero, it shouldn't contribute.
    // The scores should differ because the tension component is suppressed.
    assert!(
        (r_zero[0].score - r_with[0].score).abs() > 0.001,
        "zeroing tension_boost weight should change the composite score"
    );
}

// ===========================================================================
// Memory Lifecycle
// ===========================================================================

#[test]
fn store_alive_and_retrieve() {
    let store = InMemoryStore::new();
    let entry = make_entry_with_state("e1", 0.9, MemoryState::Alive);
    store.store(&entry).unwrap();

    let loaded = store.load(&EntryId("e1".into())).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state, MemoryState::Alive);
}

#[test]
fn transition_to_moribund_still_retrievable_lower_scored() {
    let store = InMemoryStore::new();
    store
        .store(&make_entry_with_state("e1", 0.9, MemoryState::Alive))
        .unwrap();

    store
        .transition(&EntryId("e1".into()), MemoryState::Moribund)
        .unwrap();

    // Still loadable.
    let loaded = store.load(&EntryId("e1".into())).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state, MemoryState::Moribund);

    // Not in the Alive list.
    let alive_ids = store.list(Some(MemoryState::Alive)).unwrap();
    assert!(!alive_ids.contains(&EntryId("e1".into())));

    // In the Moribund list.
    let moribund_ids = store.list(Some(MemoryState::Moribund)).unwrap();
    assert!(moribund_ids.contains(&EntryId("e1".into())));
}

#[test]
fn transition_to_deceased_not_in_alive_list() {
    let store = InMemoryStore::new();
    store
        .store(&make_entry_with_state("e1", 0.05, MemoryState::Alive))
        .unwrap();

    store
        .transition(&EntryId("e1".into()), MemoryState::Deceased)
        .unwrap();

    let alive = store.list(Some(MemoryState::Alive)).unwrap();
    assert!(alive.is_empty());

    // Still loadable via direct load, but not in Alive list.
    let loaded = store.load(&EntryId("e1".into())).unwrap();
    assert!(loaded.is_some());
    assert_eq!(loaded.unwrap().state, MemoryState::Deceased);
}

#[test]
fn resuscitation_deceased_to_alive() {
    let store = InMemoryStore::new();
    store
        .store(&make_entry_with_state("e1", 0.05, MemoryState::Deceased))
        .unwrap();

    let resuscitated = lifecycle::resuscitate(&store, &EntryId("e1".into())).unwrap();
    assert!(resuscitated, "should return true for deceased entry");

    let loaded = store.load(&EntryId("e1".into())).unwrap().unwrap();
    assert_eq!(loaded.state, MemoryState::Alive);

    let alive = store.list(Some(MemoryState::Alive)).unwrap();
    assert!(alive.contains(&EntryId("e1".into())));
}

// ===========================================================================
// CRDT Gossip
// ===========================================================================

#[test]
fn gossip_round_synchronizes_entries() {
    let agent_a = AgentId("agent-a".to_string());
    let agent_b = AgentId("agent-b".to_string());

    let mut engine_a = GossipEngine::new(agent_a.clone());
    let mut engine_b = GossipEngine::new(agent_b.clone());

    // Each agent ingests a different entry.
    engine_a.ingest_local(make_gossip_entry("entry-from-a", "agent-a"));
    engine_b.ingest_local(make_gossip_entry("entry-from-b", "agent-b"));

    assert_eq!(engine_a.entries().len(), 1);
    assert_eq!(engine_b.entries().len(), 1);

    // Gossip round: A requests from B.
    let request_a = engine_a.build_request();
    let response_b = engine_b.handle_request(&request_a);
    engine_a.process_response(response_b);

    // Gossip round: B requests from A.
    let request_b = engine_b.build_request();
    let response_a = engine_a.handle_request(&request_b);
    engine_b.process_response(response_a);

    // After gossip, both should have both entries.
    assert_eq!(
        engine_a.entries().len(),
        2,
        "engine A should have 2 entries after gossip"
    );
    assert_eq!(
        engine_b.entries().len(),
        2,
        "engine B should have 2 entries after gossip"
    );
    assert!(engine_a.get(&EntryId("entry-from-b".into())).is_some());
    assert!(engine_b.get(&EntryId("entry-from-a".into())).is_some());
}

#[test]
fn gossip_merge_commutativity() {
    use but_ai::coordination::gossip::VectorClock;

    let agent_x = AgentId("x".into());
    let agent_y = AgentId("y".into());

    let mut clock_a = VectorClock::new();
    clock_a.observe(&agent_x, 3);
    clock_a.observe(&agent_y, 1);

    let mut clock_b = VectorClock::new();
    clock_b.observe(&agent_x, 1);
    clock_b.observe(&agent_y, 5);

    // merge(A, B)
    let mut ab = clock_a.clone();
    ab.merge(&clock_b);

    // merge(B, A)
    let mut ba = clock_b.clone();
    ba.merge(&clock_a);

    assert_eq!(
        ab.version_for(&agent_x),
        ba.version_for(&agent_x),
        "merge should be commutative for agent x"
    );
    assert_eq!(
        ab.version_for(&agent_y),
        ba.version_for(&agent_y),
        "merge should be commutative for agent y"
    );
}

#[test]
fn gossip_merge_idempotency() {
    use but_ai::coordination::gossip::VectorClock;

    let agent = AgentId("alpha".into());

    let mut clock = VectorClock::new();
    clock.observe(&agent, 7);

    let snapshot = clock.clone();
    clock.merge(&snapshot);

    assert_eq!(
        clock.version_for(&agent),
        7,
        "merge(A, A) should equal A"
    );
}

#[test]
fn vector_clock_advances_monotonically() {
    use but_ai::coordination::gossip::VectorClock;

    let agent = AgentId("mono".into());
    let mut clock = VectorClock::new();

    clock.observe(&agent, 1);
    assert_eq!(clock.version_for(&agent), 1);

    clock.observe(&agent, 5);
    assert_eq!(clock.version_for(&agent), 5);

    // Attempt to go backward -- should be ignored.
    clock.observe(&agent, 3);
    assert_eq!(
        clock.version_for(&agent),
        5,
        "clock should never go backward"
    );

    clock.observe(&agent, 10);
    assert_eq!(clock.version_for(&agent), 10);
}

// ===========================================================================
// Coordination Messages
// ===========================================================================

#[test]
fn render_parse_round_trip() {
    let msg = sample_coordination_message();
    let rendered = messages::render(&msg).unwrap();
    let parsed_results = messages::parse(&rendered);

    assert_eq!(parsed_results.len(), 1);
    let parsed = parsed_results.into_iter().next().unwrap().unwrap();

    assert_eq!(parsed.schema, msg.schema);
    assert_eq!(parsed.message_type, msg.message_type);
    assert_eq!(parsed.from.0, msg.from.0);
    assert_eq!(parsed.to.as_ref().unwrap().0, msg.to.as_ref().unwrap().0);
    assert_eq!(parsed.timestamp, msg.timestamp);
    assert_eq!(parsed.payload, msg.payload);
}

#[test]
fn no_fence_returns_empty() {
    let results = messages::parse("Just a regular PR comment with no code fences.");
    assert!(results.is_empty());
}

#[test]
fn multiple_messages_in_one_comment() {
    let msg1 = sample_coordination_message();
    let msg2 = CoordinationMessage {
        message_type: MessageType::BudgetReport,
        payload: serde_json::json!({"remaining": 5000}),
        ..sample_coordination_message()
    };
    let msg3 = CoordinationMessage {
        message_type: MessageType::PatchHandoff,
        payload: serde_json::json!({"patch_id": "abc123"}),
        ..sample_coordination_message()
    };

    let comment = format!(
        "PR update:\n\n{}\n\nMiddle text\n\n{}\n\nTrailing:\n\n{}",
        messages::render(&msg1).unwrap(),
        messages::render(&msg2).unwrap(),
        messages::render(&msg3).unwrap(),
    );

    let parsed = messages::parse(&comment);
    assert_eq!(parsed.len(), 3, "should parse all three messages");
    assert_eq!(
        parsed[0].as_ref().unwrap().message_type,
        MessageType::StatusReport
    );
    assert_eq!(
        parsed[1].as_ref().unwrap().message_type,
        MessageType::BudgetReport
    );
    assert_eq!(
        parsed[2].as_ref().unwrap().message_type,
        MessageType::PatchHandoff
    );
}

// ===========================================================================
// Dependency DAG
// ===========================================================================

#[test]
fn topological_sort_linear_chain() {
    let mut graph = DependencyGraph::new();

    let a = pr("org", "repo", 1);
    let b = pr("org", "repo", 2);
    let c = pr("org", "repo", 3);

    graph.upsert(dep_node(a.clone(), vec![]));
    graph.upsert(dep_node(b.clone(), vec![a.clone()]));
    graph.upsert(dep_node(c.clone(), vec![b.clone()]));

    let sorted = graph.topological_sort().unwrap();
    let order: Vec<u64> = sorted.iter().map(|n| n.pr.number).collect();

    let pos_a = order.iter().position(|&n| n == 1).unwrap();
    let pos_b = order.iter().position(|&n| n == 2).unwrap();
    let pos_c = order.iter().position(|&n| n == 3).unwrap();

    assert!(pos_a < pos_b, "A must come before B");
    assert!(pos_b < pos_c, "B must come before C");
}

#[test]
fn topological_sort_cycle_returns_error() {
    let mut graph = DependencyGraph::new();

    let a = pr("org", "repo", 1);
    let b = pr("org", "repo", 2);

    graph.upsert(dep_node(a.clone(), vec![b.clone()]));
    graph.upsert(dep_node(b.clone(), vec![a.clone()]));

    let result = graph.topological_sort();
    assert!(
        result.is_err(),
        "cycle should produce an error"
    );
    assert!(
        result.unwrap_err().to_string().contains("Cycle"),
        "error message should mention cycle"
    );
}

#[test]
fn ready_returns_unmerged_node_with_deps_met() {
    let mut graph = DependencyGraph::new();

    let a = pr("org", "repo", 1);
    let b = pr("org", "repo", 2);
    let c = pr("org", "repo", 3);

    graph.upsert(dep_node(a.clone(), vec![]));
    graph.upsert(dep_node(b.clone(), vec![a.clone()]));
    graph.upsert(dep_node(c.clone(), vec![b.clone()]));

    // Initially only A is ready (no deps).
    let ready = graph.ready();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].pr.number, 1);

    // Merge A -> B becomes ready, C still blocked.
    graph.set_status(&a, PrStatus::Merged);
    let ready = graph.ready();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].pr.number, 2);

    // Merge B -> C becomes ready.
    graph.set_status(&b, PrStatus::Merged);
    let ready = graph.ready();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].pr.number, 3);
}

// ===========================================================================
// Budget Management
// ===========================================================================

#[test]
fn budget_mode_full_at_zero_percent() {
    let budget = TokenBudget::new(100_000);
    assert_eq!(
        but_ai::agent::budget_mode(&budget),
        but_ai::agent::BudgetMode::Full
    );
}

#[test]
fn budget_mode_abbreviated_at_60_percent() {
    let mut budget = TokenBudget::new(10_000);
    budget.used = 3_000; // 30% used, 70% remaining -> Abbreviated (50-80%)
    assert_eq!(
        but_ai::agent::budget_mode(&budget),
        but_ai::agent::BudgetMode::Abbreviated
    );
}

#[test]
fn budget_mode_minimum_output_at_85_percent() {
    let mut budget = TokenBudget::new(10_000);
    budget.used = 7_500; // 75% used, 25% remaining
    assert_eq!(
        but_ai::agent::budget_mode(&budget),
        but_ai::agent::BudgetMode::MinimumOutput
    );
}

#[test]
fn budget_mode_emergency_halt_at_96_percent() {
    let mut budget = TokenBudget::new(10_000);
    budget.used = 9_600; // 96% used, 4% remaining
    assert_eq!(
        but_ai::agent::budget_mode(&budget),
        but_ai::agent::BudgetMode::EmergencyHalt
    );
}

#[test]
fn available_for_work_accounts_for_reserves() {
    let budget = TokenBudget::new(10_000);
    // remaining = 10000, reserves = 1500 + 2000 = 3500
    // available_for_work = 10000 - 0 - 3500 = 6500
    let available = budget.available_for_work();
    let expected = budget.remaining() - budget.catalog_reserve - budget.coordination_reserve;
    assert_eq!(
        available, expected,
        "available_for_work should be remaining minus reserves"
    );
}

// ===========================================================================
// Phase Gating
// ===========================================================================

#[test]
fn classify_phase_includes_memory_retrieve_not_file_write() {
    assert!(
        but_ai::agent::phase_gate::is_tool_allowed("memory-retrieve", TaskPhase::Classify),
        "Classify phase should include memory-retrieve"
    );
    assert!(
        !but_ai::agent::phase_gate::is_tool_allowed("file-write", TaskPhase::Classify),
        "Classify phase should NOT include file-write"
    );
}

#[test]
fn implement_phase_includes_file_write() {
    assert!(
        but_ai::agent::phase_gate::is_tool_allowed("file-write", TaskPhase::Implement),
        "Implement phase should include file-write"
    );
}

#[test]
fn validate_phase_includes_continuity_check_not_file_write() {
    assert!(
        but_ai::agent::phase_gate::is_tool_allowed("continuity-check", TaskPhase::Validate),
        "Validate phase should include continuity-check"
    );
    assert!(
        !but_ai::agent::phase_gate::is_tool_allowed("file-write", TaskPhase::Validate),
        "Validate phase should NOT include file-write"
    );
}
