# The Case of the Dying Memories

*A WEAVE Protocol Mystery in 5 Acts*

---

It was a cold Tuesday in the memory partition. The kind of day where vector
clocks tick a little slower and even the most diligent gossip engine keeps to
itself. I was halfway through my second cup of compile-time when the call came
in.

"Detective, we've got a situation in `refs/but-ai/memory`. A memory entry about
database connection pooling -- found deceased in the archived partition. Only
three days old."

Three days. Memory entries fitted with a Weibull distribution, k=2.0,
lambda=100, don't just *die* at three days. Not unless someone tampered with the
survival function.

I grabbed my toolkit -- `cargo test` and a fresh test file -- and headed to the
scene.

---

## Your Case File

Create a new test file at `crates/but-ai/tests/tutorial_mystery.rs`. This is
your evidence ledger. Every test you write is a clue. When all 22 tests pass,
the case is solved.

Start with the imports and helper functions:

```rust
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
```

Every detective needs a way to reconstruct the scene. These helpers build memory
entries for your tests:

```rust
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
```

Process the evidence so far:

```sh
cargo test -p but-ai --test tutorial_mystery
```

Nothing to test yet. The real work begins now.

---

## Act 1: The Premature Death

*Scene: The Morgue (`survival/distributions` module)*

The memory lay face-down in the deceased partition, its survival probability
reading 0.03. Classic case of parameter tampering. I pulled up the Weibull
distribution -- shape k=2.0, scale lambda=100 -- and ran the numbers myself.

### Clue 1: Check the Time of Death

A Weibull distribution with k=2.0 and lambda=100 means the *scale* of the
survival curve stretches out to 100 days. At day 3, the memory should be
practically immortal. Let's prove it.

```rust
#[test]
fn act1_weibull_survival_at_day_3() {
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
```

The `SurvivalDistribution` enum lives in `but_ai::types`. The
`survival_probability(t)` method computes S(t) = P(T > t) -- the probability
that the memory survives past time t. For a Weibull distribution:

```
S(t) = exp(-(t/lambda)^k)
S(3) = exp(-(3/100)^2) = exp(-0.0009) ~ 0.9991
```

The victim had an S(3) of 0.9991. It should have been very much alive.

### Clue 2: Map the Full Survival Curve

Let's trace the curve from healthy to terminal:

```rust
#[test]
fn act1_weibull_curve_over_time() {
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
```

Notice the shape with k=2.0: the hazard rate *increases* over time. This is
appropriate for architectural knowledge -- it stays relevant for a long time but
eventually gets superseded. Compare to an exponential distribution (k=1.0) where
the hazard rate is constant -- the memory is equally likely to die at any moment,
like a bug fix that could be invalidated by any change.

### Clue 3: The Smoking Gun

I had my theory. Someone swapped k and lambda. Instead of k=2.0, lambda=100,
the perpetrator used k=100, lambda=2.0. Let's prove it kills instantly:

```rust
#[test]
fn act1_swapped_parameters_produce_wrong_curve() {
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
```

With lambda=2.0 and k=100, S(3) = exp(-(3/2)^100). That exponent is
astronomically large. The memory never stood a chance.

### Clue 4: The Coroner's Report

The lifecycle module translates survival probabilities into three states. This is
how the system decided the memory was "deceased":

```rust
#[test]
fn act1_lifecycle_state_from_survival() {
    assert_eq!(state_for_probability(0.999), MemoryState::Alive);
    assert_eq!(state_for_probability(MORIBUND_THRESHOLD), MemoryState::Alive);
    assert_eq!(state_for_probability(0.20), MemoryState::Moribund);
    assert_eq!(state_for_probability(DECEASED_THRESHOLD), MemoryState::Moribund);
    assert_eq!(state_for_probability(0.05), MemoryState::Deceased);
}
```

The thresholds: `MORIBUND_THRESHOLD` is 0.25, `DECEASED_THRESHOLD` is 0.10.
Between them lies the moribund state -- a grace period where the memory is
under review but not yet expired. This intermediate state prevents premature
deaths. Unless, of course, someone tampers with the parameters.

Process the evidence:

```sh
cargo test -p but-ai --test tutorial_mystery -- act1
```

Four tests pass. The cause of death: parameter tampering. But there's more to
investigate.

---

## Act 2: The Missing Connection

*Scene: The Library (`memory/` module)*

Two memories sat in adjacent shelves, one about JWT authentication, the other
about session management. They had never been introduced. When someone searched
for "authentication," the session management memory didn't even show up. In a
properly run library, a "see also" card would connect them. But the cards were
never filed.

### Clue 5: The Isolation Test

First, prove the problem exists. Without see-also links, graph distance is zero:

```rust
#[test]
fn act2_isolated_memories_score_zero_see_also() {
    let store = InMemoryStore::new();
    let jwt = make_classified_entry(
        "jwt", "JWT authentication token validation", &["authentication"]
    );
    let session = make_classified_entry(
        "session", "session management cookie handling", &["session"]
    );
    store.store(&jwt).unwrap();
    store.store(&session).unwrap();

    let graph = SeeAlsoGraph::new(5);
    assert_eq!(
        graph.distance_score(&EntryId("jwt".into()), &EntryId("session".into())),
        0.0,
        "No path exists -- they're complete strangers."
    );
}
```

The `SeeAlsoGraph` is a bidirectional cross-reference graph. Without explicit
links, `distance_score` returns 0.0 -- there is no path between the entries.

### Clue 6: Build the Bridge

Add a link, and the score jumps to life:

```rust
#[test]
fn act2_linked_memories_boost_retrieval() {
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
```

The scoring formula: `1.0 / (hops + 1.0)`. Direct links score 0.5, two-hop
connections score 0.33, three hops score 0.25. No path? Zero. Simple,
predictable, and powerful.

### Clue 7: The Full Retrieval

Now the satisfying part -- watch the retrieval engine use the graph to boost
session management when searching for "authentication":

```rust
#[test]
fn act2_retrieval_with_see_also_graph() {
    let store = InMemoryStore::new();
    let mut jwt = make_classified_entry(
        "jwt", "JWT authentication token validation", &["authentication"]
    );
    jwt.classification.subject_headings.push("jwt".into());
    let session = make_classified_entry(
        "session", "session management cookie handling", &["session"]
    );
    let unrelated = make_classified_entry(
        "unrelated", "database migration scripts", &["database"]
    );

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
```

The `RetrievalEngine` uses six scoring components. The `see_also_distance`
component is one of them, weighted at 0.20 by default. It transforms the
library's dusty card catalog into a knowledge graph.

```sh
cargo test -p but-ai --test tutorial_mystery -- act2
```

Three more tests pass. The memories are connected. But there are deeper patterns
hiding in the evidence room.

---

## Act 3: The Pattern No One Noticed

*Scene: The Evidence Room (`narrative/motif` module)*

I'd been reviewing ten tasks' worth of memory entries when I noticed it: "error
handling" appeared in every single one. Different contexts, different files, but
the same theme. And yet, no motif had been recorded. The pattern was there, plain
as day, but nobody was counting.

### Clue 8: One Sighting Is Not Enough

A motif needs to prove itself. One appearance is just a coincidence:

```rust
#[test]
fn act3_single_appearance_stays_proto() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

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
```

After one appearance, `record_appearance` returns `false` -- no promotion
occurred. The theme exists as a *proto-motif*: tracked, but not yet confirmed.

### Clue 9: Two Sightings -- Still Circumstantial

```rust
#[test]
fn act3_two_appearances_still_proto() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-1".into())
    );
    let promoted = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("task-2".into()),
    );

    assert!(!promoted, "Two sightings -- tantalizingly close, but not enough.");
    assert_eq!(index.motif_count(), 0);
    assert_eq!(index.proto_motif_count(), 1);
}
```

Two appearances. The pattern is real. But the system demands three.

### Clue 10: The Third Witness

```rust
#[test]
fn act3_three_appearances_motif_emerges() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-1".into())
    );
    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-2".into())
    );
    let promoted = index.record_appearance(
        &motif_id,
        "error handling patterns",
        &EntryId("task-3".into()),
    );

    assert!(promoted, "Three sightings -- the motif EMERGES!");
    assert_eq!(index.motif_count(), 1);
    assert_eq!(index.proto_motif_count(), 0);
    assert!(index.is_emerged(&motif_id));

    let motif = index.get(&motif_id).unwrap();
    assert_eq!(motif.appearances.len(), 3);
}
```

The emergence threshold is 3. When `record_appearance` returns `true`, the
proto-motif has been promoted to a full motif. It moves from the
`proto_motifs` map to the `motifs` map, and `is_emerged` starts returning true.

### Clue 11: Resonance Changes Everything

An emerged motif becomes a retrieval anchor. When a query matches a motif's
description, entries carrying that motif get a resonance boost:

```rust
#[test]
fn act3_emerged_motif_has_resonance() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    for i in 1..=3 {
        index.record_appearance(
            &motif_id,
            "error handling patterns",
            &EntryId(format!("task-{i}")),
        );
    }

    let mut entry = make_entry("evidence", "robust error handling in auth module");
    entry.motifs.push(motif_id.clone());

    let resonance = index.resonance("error handling", &entry);
    assert!(
        resonance > 0.0,
        "Emerged motif should resonate with matching query. Got {resonance}"
    );
}
```

The `resonance` method computes word overlap between the query and the motif's
description. For emerged motifs, this is a full-weight contribution. For
proto-motifs? Read on.

### Clue 12: Proto-Motifs Whisper

Proto-motifs still contribute to resonance, but at 30% weight:

```rust
#[test]
fn act3_proto_motif_resonates_at_reduced_weight() {
    let mut index = MotifIndex::new();
    let motif_id = MotifId("error-handling".into());

    // Only 1 appearance -- still a proto-motif.
    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-1".into())
    );

    let mut entry = make_entry("evidence", "error handling");
    entry.motifs.push(motif_id.clone());

    let resonance = index.resonance("error handling", &entry);
    assert!(
        resonance > 0.0,
        "Proto-motifs resonate too, just at reduced weight. Got {resonance}"
    );

    // Now promote it and compare.
    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-2".into())
    );
    index.record_appearance(
        &motif_id, "error handling patterns", &EntryId("task-3".into())
    );

    let full_resonance = index.resonance("error handling", &entry);
    assert!(
        full_resonance >= resonance,
        "Emerged motif resonance ({full_resonance}) should be >= proto ({resonance})"
    );
}
```

The constant `PROTO_MOTIF_WEIGHT` is 0.3. Proto-motifs are like rumors in a
detective novel -- they carry some weight, but you don't bet the case on them.
Once confirmed (3+ appearances), they become the full thing.

```sh
cargo test -p but-ai --test tutorial_mystery -- act3
```

Five tests pass. Motifs explained. But I had a nagging feeling about something
else buried in the evidence -- a contradiction nobody wanted to talk about.

---

## Act 4: The Contradiction Cover-Up

*Scene: The Interrogation Room (`narrative/tension` module)*

Two memories. Direct contradictions. One said: "Use synchronous database calls
for simplicity." The other: "Use async database calls for performance." Both
alive. Both cited. And not a single tension logged between them. The
contradiction had been silently buried.

### Clue 13: Urgency Grows With Time

A tension is a tracked contradiction. Once introduced, its urgency follows a
Weibull CDF with k=2.0 and lambda=14 days:

```rust
#[test]
fn act4_tension_urgency_grows_over_time() {
    let mut registry = TensionRegistry::new();
    let tension_id = TensionId("sync-vs-async".into());

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
    assert!(u7 > u0, "Urgency should grow over time. Day 0: {u0}, Day 7: {u7}");

    // After 14 days: urgency significant (Weibull lambda=14 days).
    let fourteen_days = 14 * 24 * 3600;
    let u14 = registry.urgency_score(&tension_id, t0 + fourteen_days);
    assert!(u14 > u7, "Day 14 urgency ({u14}) should exceed day 7 ({u7})");
    assert!(u14 > 0.3, "By day 14, urgency should be substantial. Got {u14}");
}
```

The urgency formula: `urgency(t) = 1 - exp(-(t/lambda)^k)`. It is a Weibull
CDF -- the same family of distributions used for memory survival, but now
applied to the *pain* of leaving a contradiction unresolved. At t=0, urgency is
zero. At t=lambda (14 days), it is `1 - exp(-1) ~ 0.632` before the severity
multiplier. The longer you ignore it, the louder it screams.

### Clue 14: Resolution Silences the Alarm

```rust
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

    let resolved = registry.resolve(&tension_id, &EntryId("memory-resolution".into()));
    assert!(resolved, "Resolution should succeed.");

    let thirty_days = 30 * 24 * 3600;
    let u = registry.urgency_score(&tension_id, t0 + thirty_days);
    assert!(
        u < f64::EPSILON,
        "Resolved tensions have zero urgency. Got {u}"
    );
}
```

Once `resolve` is called, `urgency_score` returns 0.0 regardless of age. The
contradiction has been addressed. Case closed -- on that particular tension,
anyway.

### Clue 15: The 14-Day Escalation

If nobody resolves a tension within 14 days, the system escalates it to
`Critical` severity:

```rust
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
```

The escalation threshold is 14 days (matching the Weibull lambda). After
escalation, the severity is set to `Critical` and an escalation bonus of 0.15
is added to the urgency score. The system does not let contradictions hide
forever.

```sh
cargo test -p but-ai --test tutorial_mystery -- act4
```

Three tests. Contradictions exposed. But the investigation was not over. I had
heard rumors -- gossip, you might say -- about agents that were not keeping their
stories straight.

---

## Act 5: The Gossip That Went Wrong

*Scene: The Wiretap Room (`coordination/gossip` module)*

Two agents. Two different versions of the same memory. Agent A had an old copy;
Agent B had an updated one. They were supposed to synchronize via the gossip
protocol. But who would win? And would they actually converge?

### Clue 16: Clocks Don't Go Backward

First, the foundation: vector clocks track what each agent has seen.

```rust
#[test]
fn act5_vector_clock_tracks_observations() {
    let mut clock = VectorClock::new();
    let agent_a = AgentId("agent-a".into());

    assert_eq!(clock.version_for(&agent_a), 0);

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
```

`VectorClock` maps agent IDs to version numbers. `observe` only advances -- it
never decreases. This monotonicity is the foundation of CRDT convergence: you
can always tell which state is "newer."

### Clue 17: Last Writer Wins

When two agents have different versions of the same entry, the one with the
higher `access_count` wins:

```rust
#[test]
fn act5_gossip_last_writer_wins() {
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
    assert_eq!(
        merged.access_count, 5,
        "Higher access count wins. Got {}",
        merged.access_count
    );
}
```

The gossip protocol is pull-based:

1. Agent A sends a `GossipRequest` containing its `VectorClock`.
2. Agent B compares clocks and returns entries A is missing.
3. Agent A merges the entries, using last-writer-wins (access count, then
   timestamp) to resolve conflicts.

### Clue 18: Full Convergence

The real test: after two gossip rounds (A pulls from B, then B pulls from A),
both agents should have identical state:

```rust
#[test]
fn act5_bidirectional_gossip_converges() {
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

    // Both agents now have both entries.
    assert!(engine_a.get(&EntryId("e1".into())).is_some());
    assert!(engine_a.get(&EntryId("e2".into())).is_some());
    assert!(engine_b.get(&EntryId("e1".into())).is_some());
    assert!(engine_b.get(&EntryId("e2".into())).is_some());
}
```

This is the core promise of CRDTs: given enough gossip rounds, all replicas
converge to the same state. No central coordinator needed. No locks. No
elections. Just vector clocks and merge functions.

### Clue 19: Clock Advancement

After gossip, the receiving agent's clock should reflect what it learned:

```rust
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
```

The `process_response` method does two things: merge entries (last-writer-wins)
and merge clocks (element-wise maximum). After the round, A knows B is at
version 1. The next gossip round will be more efficient -- A won't ask for
entries it already has.

```sh
cargo test -p but-ai --test tutorial_mystery -- act5
```

Four tests. The wiretap room is clean. Gossip works. Agents converge.

---

## Epilogue: The Budget That Saved Us All

*Scene: The Chief's Office (`agent/budget` module)*

"Look," the chief said, leaning back in her chair, "I don't care how many motifs
emerge or how many contradictions you resolve. If the agent runs out of tokens
before it catalogs its work, all that knowledge is *gone*. Lost to the void.
That's why we have reserves."

### Clue 20: Emergency Mode Blocks Implementation

```rust
#[test]
fn epilogue_budget_modes_at_thresholds() {
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget_mode(&budget), BudgetMode::Full);

    let mut budget_90 = TokenBudget::new(32_000);
    budget_90.used = 28_800; // 90%
    assert_eq!(budget_mode(&budget_90), BudgetMode::EmergencyHalt);
    assert!(!should_proceed(&budget_90, TaskPhase::Validate));
    assert!(!should_proceed(&budget_90, TaskPhase::Implement));
}
```

The four modes, based on *remaining* budget:

| Remaining | Mode            | What happens                              |
|-----------|-----------------|-------------------------------------------|
| >= 80%    | Full            | All passes, full validation, coordination |
| 50-80%    | Abbreviated     | Skip polish pass, reduced validation      |
| 20-50%    | MinimumOutput   | Rough pass only, skip validation          |
| < 20%     | EmergencyHalt   | Stop work, catalog + coordinate only      |

### Clue 21: Catalog Always Runs

Even at 96% usage, two phases always proceed -- because their tokens are
*reserved*:

```rust
#[test]
fn epilogue_catalog_always_runs() {
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
```

The motto of the but-ai budget system: *"An unclassified result is a lost
result."* No matter how badly the budget is blown, the agent will always catalog
its work (1,500 reserved tokens) and coordinate with the team (2,000 reserved
tokens).

### Clue 22: The Reserves

```rust
#[test]
fn epilogue_reserves_protect_essential_phases() {
    let budget = TokenBudget::new(32_000);
    assert_eq!(budget.catalog_reserve, 1_500);
    assert_eq!(budget.coordination_reserve, 2_000);

    let available = budget.available_for_work();
    assert_eq!(
        available,
        32_000 - 1_500 - 2_000,
        "Work budget = total - reserves"
    );
}
```

`available_for_work()` returns `total - used - reserves`. The reserves are
carved out before any work begins. Implementation, planning, validation -- they
all draw from the work budget. But catalog and coordinate have their own
protected allocation.

```sh
cargo test -p but-ai --test tutorial_mystery -- epilogue
```

Three tests. The budget holds. The system is safe.

---

## Case Closed

Run the full evidence file:

```sh
cargo test -p but-ai --test tutorial_mystery
```

```
running 22 tests
test act1_lifecycle_state_from_survival ... ok
test act1_swapped_parameters_produce_wrong_curve ... ok
test act1_weibull_curve_over_time ... ok
test act1_weibull_survival_at_day_3 ... ok
test act2_isolated_memories_score_zero_see_also ... ok
test act2_linked_memories_boost_retrieval ... ok
test act2_retrieval_with_see_also_graph ... ok
test act3_emerged_motif_has_resonance ... ok
test act3_proto_motif_resonates_at_reduced_weight ... ok
test act3_single_appearance_stays_proto ... ok
test act3_three_appearances_motif_emerges ... ok
test act3_two_appearances_still_proto ... ok
test act4_escalation_at_14_days ... ok
test act4_resolved_tension_has_zero_urgency ... ok
test act4_tension_urgency_grows_over_time ... ok
test act5_bidirectional_gossip_converges ... ok
test act5_gossip_last_writer_wins ... ok
test act5_vector_clock_tracks_observations ... ok
test act5_vector_clocks_advance_after_gossip ... ok
test epilogue_budget_modes_at_thresholds ... ok
test epilogue_catalog_always_runs ... ok
test epilogue_reserves_protect_essential_phases ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Twenty-two tests. Five acts. One solved case.

---

## What You Learned

| Act | Module | Concepts |
|-----|--------|----------|
| 1 | `survival/distributions` | `SurvivalDistribution::Weibull`, S(t) computation, lifecycle state mapping |
| 2 | `memory/see_also`, `memory/retrieval` | `SeeAlsoGraph`, `RetrievalEngine`, 6-component scoring, graph-boosted retrieval |
| 3 | `narrative/motif` | `MotifIndex`, proto-motifs, emergence threshold (3), resonance scoring, 0.3x proto weight |
| 4 | `narrative/tension` | `TensionRegistry`, Weibull urgency CDF, escalation at 14 days, resolution |
| 5 | `coordination/gossip` | `VectorClock`, `GossipEngine`, last-writer-wins merge, CRDT convergence |
| Epilogue | `agent/budget` | `TokenBudget`, `BudgetMode`, mandatory reserves, phase gating |

The WEAVE protocol's but-ai crate is built on a simple premise: agent memory
should be treated with the same rigor as actuarial science. Memories have
lifespans (survival distributions), connections (see-also graphs), recurring
themes (motifs), contradictions (tensions), and synchronization guarantees
(CRDT gossip). The budget system ensures none of this machinery runs on fumes.

The case is closed. But the memories live on.
