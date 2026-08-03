# V2 retrieval algorithm

## Status and provenance

This document is the Phase E algorithm contract. It carries forward the
measured retrieval structure from the legacy `memory-engrave` project, while
replacing legacy storage and identity assumptions with the V2 contracts.

The algorithm is deliberately provider-neutral. A provider, embedding model,
dimension, or ANN index becomes a V2 decision only after the Phase E benchmark
gate measures it on V2 fixtures. The legacy project reported strong
LongMemEval-S results, but those results are evidence for a starting profile,
not a claim that V2 has already matched or exceeded the market.

## Invariants

1. **Authorization precedes retrieval.** Tenant, Area, role, agent identity,
   visibility, status, validity, and Rule filters are resolved by the server
   before either candidate channel runs. Unauthorized candidates are never
   retrieved and filtered only at the end.
2. **Recall first, precision later.** Entry channels fetch a bounded but wide
   pool. Relevance floors, fusion, graph expansion, reranking, and packet
   packing narrow it in later stages.
3. **Lexical fallback is always available.** Vector infrastructure, embedding
   providers, and rerankers are optional. Their failure is visible in the
   search trace and does not make lexical retrieval unavailable.
4. **Indexes are projections.** PostgreSQL remains authoritative for identity,
   authorization, lifecycle, and canonical content. LanceDB and other indexes
   are rebuildable projections.
5. **Every stage is bounded and observable.** Each stage records candidate
   counts, latency, configuration, and fallback state. No recursive or
   unbounded search is allowed in Light search.
6. **Deterministic ties.** Every ranking tie is broken by the stable V2 typed
   identifier from ADR-0005, never by process order or an unseeded random value.

## Light-search pipeline

```text
request
  -> resolve actor, agent, tenant, Area, purpose, Rules, and time perspective
  -> normalize query and optionally expand approved aliases
  -> run lexical BM25 and dense vector retrieval in parallel
  -> apply channel-specific relevance floor to lexical results
  -> fuse channels (RRF baseline; weighted blend only when benchmark-selected)
  -> optionally expand a bounded authorized graph neighborhood
  -> optionally rerank the bounded head with a cross-encoder
  -> apply recency, confidence, applicability, importance, and lineage signals
  -> deduplicate by logical Memory and preserve contradiction/supersession data
  -> greedily pack clean claims and small evidence summaries into token budget
  -> return retrieval packet plus trace and degraded-mode indicators
```

The order is intentional:

- query expansion must precede both embedding and BM25 so both channels see
  the same vocabulary;
- lexical and dense channels must share the same authorization prefilter;
- fusion must precede reranking so expensive pairwise scoring sees a bounded
  union rather than the corpus;
- graph expansion starts from fused entries, not vector-only entries;
- recency and confidence affect final ordering but never eligibility;
- token packing happens last, after the ranking is stable.

## Stage contract

### 1. Query preparation

Preserve the original query for audit and metrics. Normalize only for search
inputs. Optional alias expansion appends deterministic, deduplicated aliases;
it never deletes or rewrites the original words. Expansion is disabled unless
the benchmarked configuration enables it, because domain aliases can add
noise.

The query cache key is `(embedding_provider, model, model_version, input_type,
expanded_query)`. A provider or model change therefore cannot reuse an
incompatible vector.

### 2. Authorized candidate generation

The server derives a predicate equivalent to:

```text
(tenant = active_tenant)
AND (Area in permitted_Areas)
AND (visibility permitted for actor/agent/purpose)
AND (status and validity permit retrieval)
AND (Rule and time-perspective constraints permit retrieval)
```

The predicate is applied as a database/vector prefilter where supported. Every
returned canonical record is rehydrated from PostgreSQL and re-authorized
before it can enter the packet.

The baseline candidate pool is `K_entry = 30` per channel, inherited from the
legacy benchmark profile. Phase E may change it only with a benchmark report
showing the recall/latency trade-off.

### 3. Lexical channel

Use PostgreSQL full-text search as the canonical Community Edition lexical
path. BM25 is the baseline scoring function with `k1 = 1.2` and `b = 0.75`.
The indexed text is clean claim text plus explicitly selected searchable
metadata; raw private Source bodies are not copied into the retrieval packet.

Apply a per-query lexical floor before fusion when enabled:

```text
keep hit h when BM25(h) >= floor_fraction * max_h(BM25(h))
```

The legacy profile used `floor_fraction = 0.35`. This is a tunable candidate
default, not a universal truth. The floor applies to lexical results only;
the dense channel is not filtered by a fraction-of-top heuristic.

### 4. Dense channel

Embed the prepared query and search tenant/Area-scoped vector projections.
Vectors are L2-normalized before comparison. The initial V2 correctness mode
is exact vector search. ANN may be enabled only after its recall@20 is within
one percentage point of exact search and its latency gate passes on the same
fixture and authorization distribution.

The provider and model are configuration, not domain identity. Each vector
projection records provider/model version, dimension, content hash, and index
state so it can be invalidated and rebuilt safely.

### 5. Hybrid fusion

The baseline is Reciprocal Rank Fusion with `k = 60`, because rank fusion is
stable when BM25 and vector scores have different scales. A weighted blend is
allowed as an evaluated profile; the legacy profile used dense weight
`alpha = 0.9` and lexical weight `1 - alpha = 0.1` after per-channel
normalization.

The fusion union retains IDs found by either channel. Missing a channel is not
itself a reason to discard a hit. Every tie is resolved by the typed ID.

### 6. Bounded graph expansion

Graph expansion is optional for Light search and must preserve authorization,
Area, status, validity, and visibility filters. The starting set is the fused
head. The baseline candidate is breadth-first depth `2`, capped at `2 * K`
new nodes. Graph-only nodes remain behind direct retrieval hits unless a
measured V2 rank policy proves otherwise.

Cross-Map traversal is never implicit: it requires a permitted mapping and a
traceable mapping path.

### 7. Cross-encoder reranking

Reranking is optional and applies only to a bounded head, baseline `N = 30`.
It scores `(prepared_query, clean_claim_text + rationale)` jointly. The raw
logit is higher-is-better; no softmax is needed for ranking. A provider or
model failure fails open to the fused order and records the fallback.

The legacy `ms-marco-MiniLM-L-6-v2` configuration is a reproducible baseline
candidate, not a V2 lock-in.

### 8. Final ordering and packet packing

After retrieval and optional reranking, apply only bounded ranking modifiers:
recency, confidence, applicability, importance, and lineage/contradiction
state. These modifiers cannot make an ineligible record eligible.

Deduplicate by logical Memory while preserving version, evidence, and
contradiction metadata. Pack the highest-ranked clean claims and small evidence
summaries greedily under the caller's token budget. Do not silently include
full Sources; deeper evidence inspection is an explicit follow-up action.

## Retrieval trace

Every packet includes enough trace metadata to explain selection without
leaking private content:

- query hash over the original query and the active algorithm/config version;
- tenant/Area scope and policy decision summary;
- channel counts, fusion mode/parameters, floor, reranker, graph limits;
- candidate and returned IDs, selection reasons, provenance, and confidence;
- latency by stage, token estimate, fallback/degraded mode, and index versions.

The trace is an Event/observability concern, not a substitute for the
canonical domain records or Decision Envelope.

## Aggressive search boundary

Aggressive search may reuse these stages with subqueries, larger bounded
pools, cross-Area retrieval, exact evidence reads, and iterative verification.
It must expose progress and remain bounded. It is not permitted to change the
Light-search semantics silently.

## What was intentionally not imported

- Legacy Blake3 content IDs: V2 uses typed UUIDv7 identities (ADR-0005).
- A mandatory Voyage provider or fixed 1024 dimensions: V2 records provider
  and model versions and selects them through Phase E measurement.
- Legacy Python module names, Lance-specific implementation details, and
  environment-variable names: V2 owns the Rust/API configuration surface.
- Legacy benchmark headline numbers as V2 acceptance evidence: V2 must rerun
  the benchmark with V2 fixtures, policies, and corpus boundaries.
