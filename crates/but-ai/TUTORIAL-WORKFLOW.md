# A Week with WEAVE

*How your AI assistant gets smarter without you noticing*

---

You just installed GitButler. You have an AI coding assistant -- maybe Claude,
maybe something else. You never heard of WEAVE, never configured any "memory
engine," never read a single line of `but-ai` source code.

And you never will. That is the whole point.

This is the story of your first week. Behind the scenes, `but-ai` is learning
your project, connecting the dots, noticing contradictions, and quietly aging
out stale knowledge. You will not see any of it happen. Your AI assistant will
just... get better.

---

## Prologue: Monday Morning

You open a fresh repository. Your AI assistant knows nothing about your
codebase. You type:

> "Set up the database layer with PostgreSQL."

The AI does what it always does: reads your project structure, writes some
code, opens a PR. Nothing unusual.

What you do not see:

1. **Classify.** `but-ai` looks at the task and assigns a classification --
   subject headings like `["database", "postgresql", "setup"]` and a call
   number like `INFRA.DB.SETUP`. This is like a library cataloging a new book.

2. **Plan and implement.** The agent orchestrator runs through its six phases
   (Classify, Plan, Implement, Validate, Catalog, Coordinate). This is the
   standard lifecycle for every task.

3. **Catalog.** After the work is done, `but-ai` creates a **memory entry** --
   a small record of what was done, where, and why. The entry gets a survival
   distribution (how long this knowledge is likely to stay relevant) and is
   stored as `Alive`.

4. **The memory is stored.** Not in a database. Not in a config file. In git
   refs, under `refs/but-ai/memory/`. Versioned, portable, synced with push
   and pull.

The AI assistant does not know this happened. Neither do you. Your project now
has one memory.

---

## Day 1: Your First Memory Link

You come back and type:

> "Add connection pooling to the database layer."

The AI reads your code, adds a connection pool, and opens a PR. Same as
yesterday -- or so it seems.

What you do not see:

**Retrieval.** Before the AI started working, `but-ai` searched its memory
store for anything related to "database" and "connection pooling." It found
yesterday's entry about database setup. The retrieval engine scored it using
six dimensions:

- **Call number proximity** -- `INFRA.DB.SETUP` is close to `INFRA.DB.POOL`.
- **Keyword overlap** -- "database" appears in both.
- **Survival probability** -- the entry is fresh, S(t) is high.
- **Freshness** -- it was just accessed.
- **Motif resonance** -- no motifs yet (only one entry).
- **Tension boost** -- no tensions yet.

The AI got the relevant context automatically. It knew about your schema, your
PostgreSQL choice, your migration strategy -- without asking you to repeat any
of it.

**After the task:** A second memory entry is created. And here is the quiet
magic: `but-ai` creates a **see-also link** between the two entries. The
database setup entry now points to the connection pooling entry (relationship:
`RelatedTo`), and vice versa. The link is bidirectional.

Two memories exist. They are connected. Neither you nor the AI created that
connection explicitly. It emerged from the work itself.

---

## Day 2: The Pattern Forms

You type:

> "Add query logging to the database layer."

Another task, another PR. By now you are in a rhythm. The AI feels like it
"gets" your project -- it references the connection pool config when setting
up the logger, uses your naming conventions, puts files in the right places.

What you do not see:

**Motif emergence.** This is the third task touching "database infrastructure."
`but-ai` has been tracking a proto-motif -- a recurring theme that has not yet
reached the threshold. Proto-motifs are tracked at reduced weight (0.3x) until
they appear in three or more entries.

With this third appearance, the proto-motif **promotes to a full motif**:
`"database-infrastructure"`. This is not a tag you applied. It is a pattern
that `but-ai` detected on its own.

From now on, any query mentioning "database" will trigger **motif resonance**
-- a retrieval signal that goes beyond keyword matching. The motif connects
all three entries thematically, even if they use different words. A query about
"SQL performance" would find the logging entry through motif resonance, even
though "SQL performance" never appears in that entry's text.

Three entries. One motif. Zero configuration.

---

## Day 3: The Contradiction

You type:

> "Switch to SQLite for the test environment."

Perfectly reasonable. You want fast, in-memory tests without spinning up
PostgreSQL.

What you do not see:

**Tension detection.** `but-ai` notices a contradiction. Day 1 established
PostgreSQL as the database. Now you are introducing SQLite. These are not the
same thing. Connection pooling behaves differently. Query logging syntax might
differ. Schema migrations might not be portable.

A **tension** is created:

- Description: "Production uses PostgreSQL but test environment uses SQLite --
  connection pooling and query syntax may diverge."
- Severity: `Low` (informational, not blocking).
- Introduced in: today's memory entry.
- Resolved in: not yet.

The tension is not an error. It is not a warning popup. It is a quiet note
that `but-ai` will surface when relevant. The next time someone asks about
the database setup, the AI will mention: "Note: there is a tension between
the production database (PostgreSQL) and the test database (SQLite)."

Nobody told the AI to track this. It noticed.

**Urgency grows over time.** The tension has a Weibull-shaped urgency curve.
Right now, urgency is low (it was just created). Over the next two weeks, if
nobody resolves it, urgency will climb. After 14 days, the tension escalates
to `Critical` severity. This is not an arbitrary countdown -- the urgency
function uses the same survival mathematics as memory aging. Contradictions
that persist are contradictions that matter.

---

## Day 4: The AI Remembers (For Someone Else)

A new teammate joins the project. They have never seen the codebase. Their AI
assistant has zero context. They type:

> "How is the database set up in this project?"

And their AI gives a comprehensive answer. It mentions PostgreSQL, connection
pooling, query logging, and -- crucially -- the PostgreSQL vs SQLite tension.
It sounds like the AI has been on the project for weeks.

What you do not see:

**Gossip synchronization.** When your teammate's `but-ai` instance started,
it did not have any memories. But your memories are stored in git refs, and
git refs are synced with push and pull. Your teammate's instance performed a
**gossip round**: it sent its (empty) vector clock to the shared store, and
received all the memories it was missing.

The gossip protocol uses a CRDT-based merge strategy. Each memory has an
access count and a timestamp. If two agents have different versions of the
same memory, the one with the higher access count wins. No central server.
No coordination service. Just git.

Your teammate's AI now has the same context as yours. The motif
"database-infrastructure" pulls all four entries together. The tension about
PostgreSQL vs SQLite is surfaced. The see-also links connect everything into
a navigable graph.

Knowledge transfer happened automatically. No onboarding document. No
"let me walk you through the codebase" call.

---

## Day 5: Things Get Old

It has been a while since anyone touched the database code. The team moved
on to building the frontend. The early database memories are aging.

What you do not see:

**Lifecycle transitions.** Every memory entry has a survival probability,
S(t), that decreases over time. The survival distribution depends on the
type of knowledge:

- Architectural decisions decay slowly (Weibull, long tail).
- Bug fixes expire faster (Exponential, memoryless).
- Conventions follow a bathtub curve (high early risk, stable middle, late
  wearout).

`but-ai` periodically audits its memory store. It checks each entry's S(t)
against two thresholds:

| State    | Condition            | Meaning                     |
|----------|---------------------|-----------------------------|
| Alive    | S(t) >= 0.25        | Actively relevant           |
| Moribund | 0.10 <= S(t) < 0.25 | Under review, may recover   |
| Deceased | S(t) < 0.10         | Expired, archived           |

The "query logging" memory -- which nobody has accessed in weeks -- transitions
to `Moribund`. The "connection pooling" memory -- which gets referenced every
time the API is modified -- stays `Alive`. Its access count is higher, so its
freshness score is higher, so retrieval ranks it above the moribund entry.

If you come back next month and ask about query logging, `but-ai` will still
find it (Moribund entries are not deleted). But it will rank it lower than the
entries you are actively using. Stale details fade. Key decisions persist.

There is a safety net: if a Moribund entry is accessed again and its survival
probability recovers above 0.25, it is **resuscitated** back to `Alive`. Dead
knowledge can come back to life if it turns out to be relevant again.

---

## Day 6: The Budget Crunch

You are deep into a massive refactor. The AI is working on a complex task --
restructuring the entire API layer. It has been running for a while, consuming
tokens.

What you do not see:

**Budget modes.** `but-ai` manages a token budget for each task. The budget
has four modes based on how much is remaining:

| Mode            | Remaining | Behavior                           |
|-----------------|----------|------------------------------------|
| Full            | >80%     | All passes, full validation        |
| Abbreviated     | 50-80%   | Skip polish, reduced validation    |
| MinimumOutput   | 20-50%   | Rough pass only, skip validation   |
| EmergencyHalt   | <20%     | Stop work, catalog + coordinate    |

As the refactor progresses, the budget drops. At 30% remaining, the mode
shifts to `MinimumOutput`. Validation is skipped (no time for double-checking).
The implementation pass is rougher.

But two things **always happen**, no matter how tight the budget:

1. **Cataloging.** The work is stored as a memory entry. There is a mandatory
   reserve of 1,500 tokens set aside for this, and it is never touched by the
   implementation phase. An unclassified result is a lost result.

2. **Coordination.** The PR is created and coordination messages are sent.
   There is a mandatory reserve of 2,000 tokens for this, also untouchable.

The AI does less, but never loses memory. Even if the budget runs out
mid-task, the partial work is cataloged and the team is notified. The next
agent to pick up the work will find the partial memory and continue from where
things left off.

---

## Day 7: Looking Back

Friday afternoon. You type:

> "What have we done this week?"

And the AI gives you a structured summary:

- **database-infrastructure** (motif, 4 entries): Setup, connection pooling,
  query logging, SQLite testing.
- **Tension (active):** PostgreSQL vs SQLite for testing -- severity Low,
  urgency rising.
- **Memory health:** 3 Alive, 1 Moribund, 0 Deceased.

It reads like a project status report. But nobody wrote it. It assembled
itself from the memories, motifs, and tensions that accumulated over the week.

---

## Epilogue: You Didn't Notice

That is the whole point. You never:

- Configured `but-ai`.
- Tagged your memories.
- Set expiration dates.
- Created cross-references.
- Defined motifs.
- Filed tensions.
- Wrote an onboarding document.

`but-ai` did all of this automatically:

- Classified every piece of work (call numbers, subject headings).
- Connected related memories (see-also graph, bidirectional links).
- Noticed recurring patterns (motif emergence at 3+ appearances).
- Tracked contradictions (tension detection with Weibull urgency).
- Let old knowledge fade naturally (three-state lifecycle with survival curves).
- Protected its own memory budget (mandatory reserves for catalog and coordination).
- Synced knowledge across teammates (CRDT gossip via git refs).

The best tool is the one you forget is there.

---

## Try It Yourself

The companion test file (`tests/tutorial_workflow.rs`) simulates this entire
week in code. Each test function represents one day and verifies the behind-
the-scenes behavior described here.

Run the full week:

```sh
cargo test -p but-ai --test tutorial_workflow
```

The tests use realistic content (not "test entry A") and construct proper
memory entries with meaningful classifications and subject headings. They
simulate the developer experience described above and verify the outcomes
at each step.

---

*This tutorial is part of the `but-ai` crate documentation. For a deep dive
into the internal mechanisms (survival curves, CRDT merge, budget modes),
see [TUTORIAL.md](TUTORIAL.md). For the full type reference, see
[src/types.rs](src/types.rs).*
