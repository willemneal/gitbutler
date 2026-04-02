# Tutorial Review: "The Case of the Dying Memories"

## First Impressions

The detective noir framing is genuinely enjoyable. It threads a cohesive narrative across
what would otherwise be a dry API walkthrough: a dead memory, a missing connection, a
hidden pattern, a buried contradiction, and rogue gossip. The voice is consistent --
hardboiled-but-technical -- and it never tips into parody. I wanted to keep reading, which
is the hardest thing to get right in a tutorial.

The opening paragraph is strong. "I was halfway through my second cup of compile-time
when the call came in" lands well; it signals that this tutorial has personality without
being cute about it. The premise (a 3-day-old memory found dead that shouldn't have died)
is a clean hook because the reader immediately wonders: "wait, why *would* it die at day 3?"

The pacing is good. Each act is self-contained, introduces one module, and ends with a
"run the tests" checkpoint. The whole piece can be completed in about 30 minutes.

## Act-by-Act Review

### Act 1: The Premature Death (survival/distributions)

**Clarity:** Excellent. The Weibull formula is shown in concrete terms (`S(3) = exp(-(3/100)^2) = exp(-0.0009) ~ 0.9991`), which makes the math approachable. The "swapped k and lambda" reveal is satisfying -- it explains *why* the victim died and makes the reader feel like they solved something.

**Difficulty:** Appropriately easy for a first act. The reader types code, sees it pass, and gains confidence. No conceptual hurdles.

**What worked:** The four-test structure (check S(3), trace the curve, swap parameters, map to lifecycle states) builds naturally. Clue 4 (the lifecycle state mapping) is a clean bridge to the broader system.

**What didn't:** The explanation of the monotonic decrease check in Clue 2 is implicit -- the test asserts it but the prose doesn't explain *why* this matters. A sentence like "a survival curve that goes up would be nonsensical -- memories don't spontaneously become more relevant without being accessed" would help.

### Act 2: The Missing Connection (memory/see_also, memory/retrieval)

**Clarity:** Very clear. The scoring formula `1.0 / (hops + 1.0)` is simple enough to verify mentally. The progression from "prove isolation" to "add a link" to "full retrieval" is logical.

**Difficulty:** Slightly harder than Act 1 because it introduces two modules (SeeAlsoGraph and RetrievalEngine) simultaneously. This is fine -- the graph is a dependency of the engine, so they naturally fit together.

**What worked:** Clue 7 is the payoff for the act, and it delivers. The reader sees the full 6-component scoring in action and can inspect `breakdown.see_also_distance` to verify the mechanism. The assertion compares session vs. unrelated, which is concrete and persuasive.

**What didn't:** The tutorial says the retrieval engine has "six scoring components" and weights see-also at 0.20, but doesn't list the other five with their weights. The reader has to look at `RelevanceWeights::default()` to discover them. A brief table here would be helpful, especially since Act 3 introduces `motif_resonance` (another component).

Also: the tutorial text says "Direct links score 0.5, two-hop connections score 0.33, three hops score 0.25" -- but the actual formula `1.0/(hops+1.0)` gives `1.0/(2+1)=0.333` for 2 hops and `1.0/(3+1)=0.25` for 3 hops. This is correct but could be clearer that the denominator is `hops+1`, not `hops`.

### Act 3: The Pattern No One Noticed (narrative/motif)

**Clarity:** Crystal clear. The 1-2-3 progression is the strongest pedagogical sequence in the tutorial. Each test adds exactly one observation, and the reader can predict the outcome before seeing it confirmed.

**Difficulty:** Easy, but the resonance scoring in Clues 11-12 adds depth. The 0.3x proto-motif weight is a nice detail that rewards the reader for paying attention.

**What worked:** The assert messages are particularly good here. "One sighting does not make a motif" and "tantalizingly close, but not enough" maintain the voice while being informative test failures.

**What didn't:** Clue 12 asks the reader to verify that `full_resonance >= proto_resonance`, but the underlying math (word-overlap divided by word-count, weighted by 0.3 for proto) isn't explained. The reader learns *that* proto-motifs are weaker, but not *how* the resonance function actually works. For a tutorial, this may be intentional -- but it means the reader can't predict exact numeric values, only ordinal relationships.

### Act 4: The Contradiction Cover-Up (narrative/tension)

**Clarity:** Good. The Weibull CDF urgency formula is well-explained, and the parallel to the survival distribution from Act 1 is a nice callback. The severity multiplier and escalation bonus are mentioned in prose but not tested explicitly in separate tests, which is fine for this level of depth.

**Difficulty:** Appropriate. The timestamp arithmetic (seconds) is slightly tricky -- `14 * 24 * 3600` -- but the tutorial spells it out clearly.

**What worked:** The escalation test (Clue 15) is the act's payoff. Seeing severity jump from Moderate to Critical after 14 days is concrete and dramatic. The 13-day "nothing happens" / 14-day "escalation triggers" boundary test is satisfying.

**What didn't:** The prose says "an escalation bonus of 0.15 is added to the urgency score" but this is never tested directly. The reader takes it on faith. A small additional assertion showing the urgency jump after escalation would reinforce this.

### Act 5: The Gossip That Went Wrong (coordination/gossip)

**Clarity:** Very clear. The pull-based gossip protocol is explained in three bullet points (send clock, compare, merge), and the tests follow this flow exactly.

**Difficulty:** This is the hardest act conceptually (CRDTs, vector clocks, convergence), but the tests keep it concrete. The reader doesn't need to understand CRDTs in the abstract -- they see two engines exchanging data and converging.

**What worked:** The bidirectional gossip test (Clue 18) is the best test in the entire tutorial. It demonstrates convergence in four assertions, which is both the theoretical promise of CRDTs and a practical proof that the implementation works. The vector clock advancement test (Clue 19) is a nice detail that shows the reader how gossip efficiency improves over time.

**What didn't:** The last-writer-wins test (Clue 17) could mention that `access_count` is a somewhat unusual LWW criterion. Most CRDT literature uses timestamps or logical clocks. A brief note ("we use access_count because...") would prevent confusion for readers familiar with CRDTs.

### Epilogue: The Budget That Saved Us All

**Clarity:** Excellent. The table of budget modes is the clearest piece of technical writing in the tutorial. The motto ("An unclassified result is a lost result") is memorable.

**Difficulty:** Easy. A gentle wind-down after Act 5.

**What worked:** The reserves test (Clue 22) gives the reader a concrete formula to verify: `32000 - 1500 - 2000 = 28500`. Simple arithmetic is satisfying to confirm.

**What didn't:** Nothing significant. The comment in the official test says "90% consumed: MinimumOutput" but then asserts `EmergencyHalt`, which is slightly misleading. The comment should say EmergencyHalt, since 90% consumed = 10% remaining, which is below the 20% MinimumOutput threshold.

## My Test Attempt

I was able to write all 22 tests and have them compile and pass on the first try without looking at the official file. This is the strongest possible endorsement of the tutorial's clarity.

**Where I could have gotten stuck (but didn't):**

1. **Import paths.** The tutorial provides the exact import block upfront, which eliminates guesswork. Without this, I would have struggled with `but_ai::agent::budget_mode` vs `but_ai::agent::budget::budget_mode` (both exist -- the former re-exports the latter). This was the single most important thing the tutorial got right.

2. **The `make_entry` helper.** The `MemoryEntry` struct has 14 fields. Without the provided helper, a first-time user would spend 10 minutes figuring out sensible defaults. The tutorial handles this perfectly.

3. **The `store.store(&jwt)` call taking `&self` despite mutating.** The `InMemoryStore` uses unsafe interior mutability, which is surprising. The tutorial doesn't draw attention to this, and it doesn't need to -- but if I had been reading the source code without the tutorial, this would have confused me.

4. **The `SeeAlsoGraph::distance_score` formula.** The tutorial says "Direct link = 1 hop -> score = 1/(1+1) = 0.5", which is subtly different from what I expected. Looking at the source, `shortest_path` returns `Some(1)` for a direct link, and the formula is `1.0 / (hops + 1.0)`. Without the tutorial explicitly stating the formula, I might have expected `1.0 / hops` instead.

## Comparison with Official Tests

My attempt and the official file are nearly identical in structure and content. The differences are cosmetic:

1. **Test names.** I used slightly different names (e.g., `act1_weibull_curve_checkpoints` vs `act1_weibull_curve_over_time`, `act5_vector_clock_monotonic` vs `act5_vector_clock_tracks_observations`). The tutorial prescribes exact test names, so the official names are canonical.

2. **Comments.** The official file has more inline comments maintaining the detective voice ("THE REVEAL: someone swapped k and lambda!"). My version is more terse. The official version is better for a tutorial context.

3. **Assert messages.** Nearly identical. I matched the tutorial's messages closely because they were provided in the code blocks.

4. **Structure.** Both files are organized identically: helpers, then acts 1-5, then epilogue. The tutorial's structure is strong enough to produce this convergence.

The fact that a first-time reader can independently produce a near-identical test file is evidence that the tutorial succeeds at its primary goal: teaching the API through guided practice.

## Suggestions for Improvement

### High-priority

1. **Fix the misleading comment in `epilogue_budget_modes_at_thresholds`.** The official test has a comment saying "90% consumed: MinimumOutput (validation skipped!)" but asserts `BudgetMode::EmergencyHalt`. The comment should say "90% consumed: EmergencyHalt". This is a direct contradiction in the test file.

2. **Add the 6-component scoring table to Act 2.** The tutorial mentions "six scoring components" but only names two (see-also distance and motif resonance) by Act 2. Since Acts 2-4 each introduce different components, a brief table early on showing all six with their default weights would give the reader a map of where the tutorial is headed.

3. **Explain the monotonic decrease assertion in Act 1, Clue 2.** Add one sentence explaining *why* a non-decreasing survival curve would be an error.

### Medium-priority

4. **Show the numeric resonance values in Act 3.** The tutorial tests `resonance > 0.0` and `full_resonance >= proto_resonance`, but never shows expected numeric values. Adding a comment like "proto resonance ~ 0.30 * overlap, emerged resonance ~ 1.0 * overlap" would make the mechanism more concrete.

5. **Note the LWW criterion choice in Act 5.** A brief aside ("We use access_count as our LWW tiebreaker because it reflects how frequently a memory is accessed, making it a natural proxy for freshness") would preempt questions from readers familiar with CRDTs.

6. **Add a "what's next?" section at the end.** After "Case Closed", point the reader to the next tutorial or to modules not covered (identity, validation, compaction, fitting).

### Low-priority

7. **Consider adding a Clue 0** that demonstrates creating, storing, and loading a basic MemoryEntry. This would warm up readers who are completely new to Rust trait objects and the `MemoryStore` interface.

8. **The prose mentions `PROTO_MOTIF_WEIGHT` as a constant** but doesn't tell the reader where to find it. Adding `(defined in `narrative/motif.rs`)` would help.

9. **Consider adding a "challenge" test** at the end of each act for readers who want to go deeper. Example for Act 1: "Write a test that compares Exponential, Weibull, and LogNormal survival curves at t=50."

## Rating

**8.5 / 10**

This is a well-crafted tutorial that achieves its primary goals: it teaches the but-ai API, it's fun to read, and it produces working test code. The detective metaphor is more than decoration -- it provides a narrative structure that makes the material memorable.

The half-point deductions:
- The misleading comment in the epilogue budget test (-0.25): a factual error in a tutorial undermines trust.
- The missing 6-component scoring overview (-0.25): the reader is told about a 6-part formula but only discovers the parts piecemeal across acts.
- Resonance scoring mechanics are opaque (-0.25): the reader tests ordinal relationships but can't predict numeric values.
- No forward pointers or "what's next" section (-0.25): the tutorial ends abruptly after "Case Closed."
- The other half-point is for the consistently excellent assert messages and the overall quality that earns it: (+0.5).

The tutorial is ready for use. The suggested improvements are polish, not structural changes.

## Suggested Second Tutorial: "The Case of the Forged Credentials"

A natural follow-up that builds on what "Dying Memories" teaches. The premise: an agent
submits a patch claiming to be from an authorized architect, but the signing key doesn't
match. The investigation reveals identity forgery, unauthorized branch access, and a
compaction audit that nearly destroyed the evidence.

### What it would cover

**Act 1: The Impersonation** (`identity/signing`, `identity/authorization`)
- Verify a commit signature against an agent's registered key
- Check whether an agent is authorized for a given branch pattern
- Discover the forgery: the signing key fingerprint doesn't match

**Act 2: The Overcrowded Shelf** (`memory/compaction`)
- When the memory store exceeds its capacity, compaction runs
- Compaction uses survival probabilities to decide what to evict
- A test showing that high-survival entries survive compaction while low-survival entries are evicted
- The complication: the evidence (a recently-stored memory about the forged patch) was nearly compacted because its survival distribution was tampered with (callback to Tutorial 1's parameter-swap attack)

**Act 3: The Validation Gauntlet** (`validation/integrity`, `validation/contradiction`)
- Run an integrity check that catches inconsistencies between an entry's claimed call number and its actual content
- Run a contradiction check that identifies when two alive memories assert opposite things
- Show how these checks would have caught the forged entry if they had been run before ingestion

**Act 4: The Suspicious Access Pattern** (`survival/fitting`, `survival/surprise`)
- Fit a survival distribution to observed access data
- Compute the surprise index (KL divergence) when actual access patterns diverge from predicted ones
- The forged entry has an abnormally high surprise index because its access pattern doesn't match any known distribution

**Act 5: The Cross-Repo Chase** (`coordination/dependency`, `coordination/forge`)
- Track a dependency chain across repositories
- Use the forge adapter to query PR status
- Discover that the forged agent also submitted suspicious patches to a downstream repo

### Why this sequence works

It builds directly on Tutorial 1's foundations:
- Act 1 uses the `AgentIdentity` and `AuthorizationScope` types introduced but not tested in Tutorial 1
- Act 2 requires understanding survival probabilities (Tutorial 1, Act 1) to predict compaction behavior
- Act 3 introduces validation, which is the quality gate the detective was missing
- Act 4 extends the survival module into fitting and surprise, deepening the statistical literacy from Tutorial 1
- Act 5 exercises the coordination module beyond gossip, covering the forge adapter and dependency tracking

The detective metaphor extends naturally: the first case was a murder mystery (who killed the memory?), the second is an identity theft case (who forged the credentials?). Together they cover roughly 80% of the but-ai API surface.
