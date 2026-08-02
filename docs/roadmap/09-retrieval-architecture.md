# 09 — Retrieval Architecture

> Source: SMASH_V2.md §10

## 9.1 Retrieval inputs and outputs

Every retrieval request includes: query, actor, agent, session, active Area, task context, mode, result budget, token budget, optional time perspective.

**The server derives permissions and Rules. Clients do not supply trusted visibility filters.**

The output is a **retrieval packet**, not a list of opaque vector matches. It contains:

- selected Memory with clean claims;
- why each record was selected;
- applicability and confidence labels;
- evidence summaries and Source references;
- relevant lineage or contradiction warnings;
- Areas and Cross-Map mappings traversed;
- estimated token size;
- follow-up actions for deeper Source or graph inspection;
- search trace appropriate to the selected mode.

## 9.2 Light search

Light search is the **default retrieval reflex for every agent run**: deterministic where possible, low latency, bounded, inexpensive.

Pipeline:

1. Resolve identity, permissions, active Area, and Shared Memory access.
2. Apply status, validity, applicability, and visibility filters.
3. Generate PostgreSQL full-text candidates.
4. Generate LanceDB vector candidates using security prefilters.
5. Merge lexical and semantic rankings using a measured strategy.
6. Apply Area affinity, applicability, confidence, recency, importance, and graph-proximity signals.
7. Demote stale, contradictory, or out-of-context records.
8. Return a compact number of Memory records.
9. Optionally attach small evidence summaries — not full Sources.
10. Record latency, candidate counts, and selection reasons.

**Light search must not require a generative model.** Optional local embeddings or configured embedding providers improve paraphrase matching, but lexical retrieval remains functional when vector infrastructure is unavailable.

If LanceDB is stale or down, the system **degrades visibly to lexical retrieval** rather than failing all Memory access.

**Budgets** are configurable service objectives, not hard-coded assumptions. A useful initial design goal: sub-second end-to-end retrieval, with most database and vector work completing in a few hundred milliseconds or less on a normal Community Edition deployment.

## 9.3 Aggressive search

Aggressive search is a **deliberate investigation mode** for questions that require multi-step reasoning, cross-Area evidence, contradiction analysis, or primary-source inspection. It is **not** Light search with a larger `limit`.

The pipeline may:

- decompose a question into subqueries;
- search multiple permitted Areas;
- expand through approved Cross-Map mappings;
- retrieve Memory and Source chunks separately;
- traverse bounded graph neighborhoods;
- inspect exact evidence spans;
- issue temporal or contradiction-specific queries;
- rerank a larger candidate set with a cross-encoder or configured model;
- iteratively retrieve missing evidence;
- synthesize an answer packet with citations and uncertainty;
- produce optional Memory Proposals **without activating them**.

Aggressive search always exposes **progress and a trace**. The trace records subqueries, Areas, mappings, candidate stages, Source reads, model/reranker use, Rules, latency, and cost signals. A trace can redact sensitive content while retaining decision metadata.

### Escalation triggers

The router escalates when:

- the user explicitly requests deep verification;
- Light search confidence is low;
- top results contradict one another;
- the query spans Areas;
- a high-impact action requires primary evidence;
- a Rule requires stronger verification.

**Limits prevent an agent from recursively searching without bound.**

Router diagram: [23 — Diagrams §24.7](23-diagrams.md#247-light-and-aggressive-retrieval-router).

## 9.4 Cross-Map retrieval

Search begins inside the active Area and permitted Shared Memory. Cross-Map expansion occurs **only when the query, router, or user intent justifies it**.

- Mappings generate additional concept aliases and Area targets.
- The retrieval engine preserves the mapping path and original labels.
- Identity deduplication may group results, but **meaning is not flattened**.
- Permissions are checked **before** generating candidates in another Area.
- A blocked mapping or restrictive Rule terminates that traversal.

## 9.5 Ranking evaluation

SMASH continues the V1 discipline of **measuring retrieval rather than asserting quality**. Preserve existing benchmark datasets and build V2 adapters so the new engine can be compared with V1.

Track at least:

- hit rate, recall, MRR, and nDCG at bounded cutoffs;
- source evidence coverage;
- wrong-Area and unauthorized-result rate;
- applicability accuracy;
- contradiction exposure;
- stale-memory exposure;
- light-search latency and packet tokens;
- aggressive-search evidence completeness and trace quality;
- duplicate, junk, and incorrect-admission rates;
- reviewer edit, rejection, and merge rates.

**Graph complexity is not a success metric.** Retrieval quality and downstream task correctness matter more than the number of nodes or edges.
