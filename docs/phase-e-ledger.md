# Phase E Ledger — E.2 provider compatibility and operational readiness

**Status:** Phase E.2 implementation and acceptance verification **complete**.
Production ANN deployment is not selected; exact search remains the default
reference path — see the 2026-08-03 performance pass below for re-confirmed,
fresh evidence (the gate is not met because recall@20 is unmeasured on the
only fixture available for a fresh ANN run, independent of the latency
question).

## Decisions

- Embedding profiles are configuration, not domain constants. Three candidates
  are recorded for benchmark evaluation: Voyage `voyage-3-lite` (observed
  native 512), OpenAI-compatible `text-embedding-3-large` (native 3072), and
  Cohere `embed-v4.0` (native 1536). All production profiles have exactly
  1024 stored output dimensions and an explicit projection identity.
- The current provider documentation confirms Cohere Embed v4 supports
  1024/1536 output dimensions and OpenAI documents `text-embedding-3-large` as
  an embeddings model. Voyage and OpenAI passed the V2 compatibility/capacity
  benchmark; Cohere is retained as a validated future profile pending a live
  credentialed run.
- Native dimensions are retained in the projection identity. A versioned
  signed-mixing projection adapter is required when native and stored
  dimensions differ; truncation and zero-padding are not permitted.
- Credentials are referenced by environment-variable name only. Secret values
  are not accepted by the profile identity, projection rows, jobs, or logs.
- The deterministic 32-dimensional provider is available only through the
  explicit `deterministic-dev` profile. It is not the API or worker default.
- Dense provider failure continues through the synchronous lexical path with an
  explicit degraded-mode reason. Ordinary reads do not enqueue durable jobs.

## Implemented evidence

- `crates/core/src/retrieval.rs`: `EmbeddingProfile`, validated
  `EmbeddingConfiguration`, full `ProjectionIdentity` including native and
  output dimensions, explicit `ProjectionAdapter`, typed `ProviderError`, and
  bounded `RetryPolicy`, retry directives with hints/jitter, and a circuit
  breaker state machine.
- `crates/storage/src/lib.rs`: LanceDB projection batches now carry provider,
  model, model version, projection version, configuration fingerprint, native
  dimension, and output dimension; mixed identities are rejected before write.
- `migrations/20260807100000_phase_e_provider_compatibility.sql`: secret-free
  provider profile metadata, production 1024-D constraints, native-dimension
  metadata, and durable lease/checkpoint/cancellation fields for retrieval jobs.
- `crates/api/src/main.rs` and `crates/worker/src/main.rs`: deterministic
  embedding is opt-in only; Voyage query-time retrieval and worker projection
  generation use the configured provider profile, while provider failure fails
  open to lexical retrieval.
- `crates/storage/src/lib.rs`: `VoyageEmbeddingClient` and
  `OpenAiEmbeddingClient` use in-memory API keys, map HTTP failures to typed
  provider errors, validate native dimensions, apply configured projections,
  and support ordered batch embeddings for worker re-indexing.
- `crates/api/src/main.rs`: query embeddings are profile-scoped in a bounded
  cache; cache hits skip the provider while dense results still pass through
  the same authorization rehydration path.
- `crates/storage/src/lib.rs`: partial provider batches and native dimension
  mismatches are rejected before any batch result is returned; unit tests prove
  no partial progress is accepted.
- `crates/storage/src/lib.rs`: transient provider calls now execute bounded
  exponential backoff with retry hints and jitter; authentication, invalid
  requests, dimension errors, and other permanent failures do not retry.
- `crates/api/src/main.rs`: interactive dense reads use an eight-permit
  embedding semaphore with a 250 ms acquisition bound, profile-scoped cache,
  and circuit-breaker admission/feedback before provider calls.
- `crates/worker/src/main.rs`: embedding, re-embedding, index, rebuild, and
  reconciliation operations are claimed from PostgreSQL, checkpointed,
  cancellable, and reconciled by the worker as the sole LanceDB writer; the
  previous unconditional reconciliation loop was removed.
- `scripts/benchmark-provider-operations.py` and
  `eval/results/provider-operations-2026-08-03.json`: bounded concurrency
  measurements at concurrency 1, 4, and 8, with provider rate-limit headers
  and usage captured when supplied by the provider.
- `scripts/benchmark-voyage-provider.py` and
  `eval/results/voyage-3-lite-provider-2026-08-03.json`: provider-only
  compatibility and latency evidence using three fixture queries.
- `scripts/benchmark-openai-provider.py` and
  `eval/results/openai-text-embedding-3-large-2026-08-03.json`: second
  provider-only compatibility and latency evidence using the same queries.

## Exact verification commands and results

- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo fmt --all` — pass.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo test -p engrave-core` —
  pass: 22 unit tests and 2 fixture tests; includes two profiles, 1024-D
  validation, explicit projection, typed errors, authorization, fallback,
  lifecycle, and mixed-identity coverage.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo test --workspace --locked`
  — pass; the default workspace run leaves explicitly ignored live PostgreSQL
  tests skipped, while the explicit live commands below pass.
- `DATABASE_URL=postgres://... cargo test -p engrave-storage --test
  live_repository -- --ignored --test-threads=1` — pass: live lexical
  authorization and proposal replay/conflict tests.
- `DATABASE_URL=postgres://... cargo test -p engrave-storage --test live_queue
  -- --ignored --test-threads=1` — pass: live claim, checkpoint, lease renewal,
  retry, cancellation, terminal-state, lease-expiry reclaim, dead-letter, and
  manual-retry proof.
- Live worker retrieval operation proof — pass: a PostgreSQL `kind=rebuild`
  operation was claimed, checkpointed, reconciled through the worker-owned
  LanceDB adapter, and finished as `succeeded:100`.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo clippy --workspace
  --all-targets -- -D warnings` — pass.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo test -p engrave-core` —
  pass: 22 unit tests and 2 fixture tests, including retry hints/caps and
  circuit-breaker recovery.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo deny check` — pass.
- `git diff --check` — pass.
- `./scripts/check-openapi.sh` — pass.
- `npm run build` in `apps/web` — pass.
- `python3 scripts/benchmark-voyage-provider.py` — pass: observed native
  dimension 512; cold p50 297.31 ms; repeated-request p50 293.06 ms. This
  measures provider calls only, not cache hits or retrieval quality.
- `python3 scripts/benchmark-openai-provider.py` — pass: observed native
  dimension 3072; cold p50 231.28 ms; repeated-request p50 269.01 ms. This
  measures provider calls only, not cache hits or retrieval quality.
- `python3 scripts/benchmark-phase-e-retrieval.py` — pass on the same
  authorized sales fixture for both live profiles: Recall@5/10 1.0 and zero
  unauthorized/wrong-Area results after filtering. The benchmark also observed
  9 unauthorized and 3 wrong-Area candidates before filtering, proving the
  authorization boundary is exercised. Authorized fixture end-to-end p50/p99
  was Voyage 315.70/333.96 ms and OpenAI 238.08/250.43 ms. Full output is in
  `eval/results/phase-e-retrieval-2026-08-03.json`; provider usage was 69
  tokens, with standard-list-price estimates of $0.00000000012/query and
  $0.00000000138/run for Voyage, and $0.00000078/query and $0.00000897/run
  for OpenAI. These estimates exclude account credits, discounts, and free
  tiers. Lexical fallback p99 was 0.049 ms for Voyage and 0.031 ms for OpenAI
  on this fixture.
- `cargo test -p engrave-storage --test live_lancedb -- --nocapture` — pass:
  100 exact authorization-filter queries measured p50 4.291 ms, p95 4.768 ms,
  p99 5.467 ms on the 5-row, 2-D integration fixture. Full result is in
  `eval/results/lancedb-exact-latency-2026-08-03.json`.
- `PATH=/Users/soheilebrahimi/.cargo/bin:$PATH cargo test -p engrave-storage` —
  pass: provider partial-batch, dimension fault-injection, transient retry,
  and non-retryable authentication tests pass.
- `python3 scripts/benchmark-provider-operations.py` — pass for both live
  profiles at concurrency 1/4/8. Voyage p50 was 336.05/291.11/298.73 ms;
  OpenAI p50 was 807.03/243.57/242.73 ms. The JSON records p95/p99, errors,
  rate-limit headers, usage, date, and an explicit unavailable-cost field.
- `/usr/bin/time -l python3 scripts/benchmark-phase-e-retrieval.py` — pass:
  maximum resident set size 33,112,064 bytes for the combined small fixture
  run. This is a harness measurement, not a production capacity claim.
- `/usr/bin/time -l python3 scripts/benchmark-phase-e-capacity.py` — pass on
  1,000 documents, batch size 100, concurrency 4: Voyage throughput 255.88
  items/s and authorized end-to-end 523.37 ms; OpenAI throughput 469.51
  items/s and authorized end-to-end 613.46 ms. Combined maximum resident set
  size was 236,552,192 bytes. Isolated maximum resident set size was
  104,546,304 bytes for Voyage and 238,321,664 bytes for OpenAI. Full provider
  usage and metadata are in
  `eval/results/phase-e-capacity-2026-08-03.json`.
- `cargo test -p engrave-storage --test live_lancedb -- --nocapture` — pass on
  the 1,000-row, 1024-dimensional exact fixture: p50 16.170 ms, p95 16.659
  ms, p99 18.466 ms; the explicit IVF_FLAT ANN comparison measured p50 15.143
  ms, p95 51.581 ms, p99 186.406 ms. Full result is in
  `eval/results/lancedb-exact-capacity-2026-08-03.json`.

## Benchmark gate

The provider-only Voyage run observed native 512 dimensions. Cold latency was
297.31 ms p50 (296.50–328.86 ms); repeated-request latency was 293.06 ms p50
(283.72–369.92 ms). The provider-operation run separately measured bounded
concurrency, retry behavior, rate-limit headers, and usage. The small
authorized fixture measured retrieval quality, cache latency, exact LanceDB
latency, end-to-end latency, memory, and cost estimates; production-scale
capacity remains unproven.

The provider-only OpenAI run observed native 3072 dimensions. Cold latency was
231.28 ms p50 (207.61–709.15 ms); repeated-request latency was 269.01 ms p50
(255.65–270.57 ms). It requires the explicit 3072 → 1024 projection.

The authorized fixture run measured MRR 0.6667 and nDCG@10 0.6667 for both
profiles. Cache-hit measurements were sub-millisecond in the local in-memory
harness. LanceDB exact latency was measured separately on the small integration
fixture; ANN is measured separately. The authorized fixture end-to-end path is now
measured, but it uses the small Python fixture rather than a production-sized
PostgreSQL/LanceDB corpus.

Follow-up work: expand the corpus, run additional provider-backed fault
injection in the deployment environment, and tune/decide ANN deployment based
on its observed tail latency; these do not invalidate the completed E.2
acceptance evidence. Exact search remains the measured contractual reference.

## Performance pass (2026-08-03)

**Scope:** persistent provider/DB clients, concurrent lexical+embedding
retrieval, a complete query-embedding cache key, ANN-vs-exact gate
re-confirmation, and a PostgreSQL index/query-plan audit. No Phase F/G/H/I
work was touched. **Hardware/environment for every fresh measurement below:**
this local machine (Apple Silicon, macOS/Docker Desktop), PostgreSQL 16.14
(`postgres:16-alpine`) in Docker Compose bound to `localhost:5432`, LanceDB
via local temp-directory file stores (no network), Rust 1.97.1 pinned
toolchain, `cargo test` single-process runs. `VOYAGE_API_KEY`/`OPENAI_API_KEY`
were not initially found in the shell environment or `.env`; they were
located in `.env.local` partway through this session (already auto-loaded
by `scripts/benchmark-*.py`'s own `env()` helper), so provider-backed
benchmarks below were run live, not carried over — see "Provider usage,
cost, and quality metrics" below for the fresh results. Everything else is
measured fresh, for real, this session.

### 1. Persistent provider clients and connections

`crates/api/src/main.rs`'s `AppState` gained `voyage_client: Option<Arc<VoyageEmbeddingClient>>`
and `openai_client: Option<Arc<OpenAiEmbeddingClient>>`, constructed once in
`app_with_retrieval` at startup (`VoyageEmbeddingClient::from_env().ok().map(Arc::new)`),
exactly mirroring the existing `Option<Arc<PgRepository>>`/`Option<Arc<LanceProjectionAdapter>>`
pattern. Previously `VoyageEmbeddingClient::from_env()`/`OpenAiEmbeddingClient::from_env()`
(each building a fresh `reqwest::Client`, discarding TLS/connection-pool
reuse) were called **inside the `search` handler on every request**. The
`search` function now does `state.voyage_client.clone()` /
`state.openai_client.clone()` — an `Arc` clone, not a client construction.
Credential handling is unchanged: the API key still lives only inside the
client struct's private field, in memory, never logged; it is now read from
the environment once at process start instead of once per request, which is
a stricter, not weaker, exposure surface. All 8 `engrave-api` tests pass
unmodified (no credentials configured in the test environment, so both
clients are `None`, producing the same lexical-fallback path as before —
proving no behavior change for the credential-absent case).

### 2. Concurrent lexical retrieval and query embedding

The embedding-through-`dense_hits` block was extracted into
`resolve_dense_hits(state, repository, request) -> (Vec<DenseHit>, DegradedMode)`,
which never returns `Err` (every failure path already degraded to
`DegradedMode::LexicalOnly { reason }`). `search` now runs, after
authorization is resolved and the `CoreSearchRequest` is built:

```rust
let (lexical_result, (dense, degraded)) = tokio::join!(
    repository.search_lexical(&request),
    resolve_dense_hits(&state, &repository, &request),
);
let lexical = lexical_result.map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
```

Bounded concurrency (the 8-permit `embedding_concurrency` semaphore) and the
250 ms permit-acquire timeout are untouched — they still gate only the
embedding branch, which now simply runs concurrently with lexical instead of
after it. Three internal `ProjectionIdentity`-construction failure paths that
previously used `?` to 500 the *entire* request (losing lexical results too)
now degrade to `DegradedMode::LexicalOnly` instead, consistent with "a failed
embedding task must never fail lexical retrieval."

**Fresh, real measurement** (`crates/storage/tests/live_search_concurrency.rs`,
new, `#[ignore]`d, run via `DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave cargo test -p engrave-storage --test live_search_concurrency --locked -- --ignored --nocapture`),
against 50 real seeded memories in local PostgreSQL + a real local LanceDB
projection, using the credential-free deterministic embedding provider —
sequential (`search_lexical` awaited, then embed + LanceDB exact + rehydrate
awaited) versus concurrent (`tokio::join!` of the same two units of work),
four runs:

| Run | Sequential (ms) | Concurrent (ms) |
|---|---|---|
| 1 | 34.047 | 5.933 |
| 2 | 147.443 | 9.200 |
| 3 | 17.494 | 6.012 |
| 4 | 30.299 | 6.340 |

Concurrent latency is consistently ~6-9 ms regardless of sequential's higher
run-to-run variance (likely first-open/connection overhead on the freshly
created per-run LanceDB temp path) — the expected `max(lexical, dense)`
versus `lexical + dense` shape. The same test also proves non-interference
directly: `tokio::join!(repository.search_lexical(&request), async { sleep(50ms); Err(()) })`
completed in 51-53 ms total across runs with the lexical branch succeeding —
a slow/failing embedding branch never blocked or failed lexical.

### 3. Complete query-embedding cache key

New `engrave_core::query_embedding_cache_key(identity, input_type, query)`
(in `crates/core/src/retrieval.rs`) builds the key from `identity.provider`,
`identity.model`, `identity.model_version`, `input_type` (`"query"` on this
path), `identity.projection_version`, `identity.configuration_fingerprint`,
and a normalized (whitespace-collapsed, lowercased) query string. The prior
key omitted `provider` and `projection_version` entirely and used the loose
`profile_name` string instead of the resolved identity. `crates/api/src/main.rs`'s
`search` now calls this function instead of building its own `format!`.

New test `query_embedding_cache_key_normalizes_and_isolates_by_full_identity`
(`crates/core/src/retrieval.rs`) proves: a cache **hit** for the same query
modulo casing/whitespace; a cache **miss** for genuinely different query
text; **profile isolation** for a different provider with identical query
text; **profile isolation** for a different projection version (so a
re-projection can never accidentally reuse a vector cached under the old
projection); and that `input_type` is part of the key. `engrave-core` now
has 70 unit tests (69 prior + 1 new; the plan anticipated 2 new tests but
one function covers all four required scenarios in one deterministic
assertion sequence — recorded as the actual approach taken, not the
originally planned two-test split). Fallback behavior (a cache miss or
provider failure still degrades to lexical, never panics) is covered by
Task 2's concurrency test.

Real cache-hit/miss latency (in-process `QueryEmbeddingCache::get`, a
`BTreeMap` lookup): both measured at `0` on the `Instant`-based measurement
in `live_search_concurrency.rs` — below the resolution `std::time::Instant`
can distinguish on this machine (sub-microsecond). This is expected and
consistent with the prior ledger entry's own observation that
"cache-hit measurements were sub-millisecond in the local in-memory
harness."

### 4. ANN gate re-confirmation

Fresh `cargo test -p engrave-storage --test live_lancedb --locked -- --nocapture`
(the same pre-existing 1,000-row, 1024-dimension fixture), run 4 times:

| Run | Exact p50/p95/p99 (ms) | ANN p50/p95/p99 (ms) |
|---|---|---|
| 1 | 16.545 / 19.081 / 20.169 | 15.046 / 17.626 / 20.082 |
| 2 | 16.580 / 17.739 / 20.170 | 15.005 / 16.472 / 17.860 |
| 3 | 16.303 / 16.938 / 17.468 | 14.893 / 15.216 / 16.083 |
| 4 | 16.356 / 16.761 / 18.293 | 14.929 / 15.323 / 17.204 |

**This materially disagrees with the previously recorded evidence** (exact
p99 18.466 ms, ANN p99 186.406 ms, roughly a 10x gap) — the fresh runs show
exact and ANN within noise of each other, with ANN fractionally faster in
3 of 4 runs. This discrepancy is reported honestly rather than reconciled;
plausible causes (LanceDB version/dependency drift, IVF_FLAT default
`nprobe` behavior at n=1,000, different underlying hardware between the
original run and this sandbox) are not distinguished here and are flagged
as follow-up work, not resolved.

**The ANN gate is still not met, independent of the latency question**:
the gate requires "recall@20 within one point of exact." The `live_lancedb`
fixture is 1,000 synthetic random vectors with no gold/relevance labels —
recall cannot be computed on it at any cutoff, and `BenchmarkMetrics`
(`crates/core/src/retrieval.rs`) only implements `recall_at_5`/`recall_at_10`,
not `recall_at_20`, on the separate small sales fixture that does have gold
labels. **No recall@20 number exists to evaluate the gate against, on either
fixture, so the gate cannot be satisfied regardless of the latency finding.**
Combined with the new latency evidence's own inconsistency with the prior
recorded run, this is not a basis for changing the default. **Decision:
exact search remains the default reference path, unchanged.**

### 5. PostgreSQL index and query-plan audit

A synthetic dataset was seeded directly via SQL against the local
(`docker compose`) PostgreSQL instance, migrated fresh from empty through
`008_phase_e_performance_indexes.sql` beforehand: 5 tenants x 4 areas x 20
actors/tenant x 200 memories/area = **4,000 memories, 4,000 current
`memory_versions`, 400 `area_grants`** (every actor granted every area in
its own tenant — a realistic worst case for the authorization query). Seeded
with a fixed, reproducible generation script (recorded in this ledger, not
committed as a migration since it is dev-only measurement scaffolding); the
data was deleted after measurement.

**`EXPLAIN (ANALYZE, BUFFERS)` on `search_lexical`'s eligible-CTE join**
(`memory_versions mv JOIN memories m ON m.tenant_id = mv.tenant_id AND
m.current_version_id = mv.memory_version_id`, filtered by tenant, area,
state, and full-text match) — **before** any new index:

> `Nested Loop (actual time=5.159..171.903 rows=800 loops=1)` — inner
> `Seq Scan on memories m (actual time=0.001..0.174 rows=800 loops=800)`,
> **639,200 rows removed by the join filter**, 86,410 buffer hits,
> **172.359 ms execution time**, at only 4,000 rows.

Root cause: `memories.current_version_id` had **no index at all**, so
PostgreSQL could not do an index-backed join and instead re-scanned the
entire `memories` table once per matched `memory_versions` row — a pattern
that degrades faster than linearly as tenant size grows, not merely slowly.

**`EXPLAIN (ANALYZE, BUFFERS)` on `resolve_search_authorization`'s
`area_grants` query** (the non-admin path, run on every authorization
resolution) — before any new index: `Seq Scan on area_grants`, 396 of 400
rows removed by filter, 0.227 ms. Cheap today at 400 rows, but `area_grants`
scales with tenant x actor x area grant volume and had zero index support
for its own hot-path predicate.

**Migration added** (`migrations/20260809100000_phase_e_performance_indexes.sql`,
wired into `compose.yaml`'s `migrate` service as step 008 — the full chain
001 through 008 was verified to apply cleanly from a completely empty,
freshly wiped `docker compose` volume):

```sql
CREATE INDEX IF NOT EXISTS memories_current_version_idx ON memories (current_version_id);
CREATE INDEX IF NOT EXISTS memories_tenant_area_state_idx ON memories (tenant_id, area_id, state);
CREATE INDEX IF NOT EXISTS area_grants_actor_lookup_idx ON area_grants (tenant_id, actor_id, state, effective_from, effective_until);
```

**`EXPLAIN (ANALYZE, BUFFERS)` after the migration**, same queries, same
seeded data:

- `search_lexical` join: `Hash Join (actual time=1.349..6.168 rows=800
  loops=1)`, driven by `Bitmap Index Scan on memories_tenant_area_state_idx`
  and `Bitmap Index Scan on memory_versions_live_retrieval_idx`, **6.512 ms
  execution time** — a **~26x** improvement, from 86,410 to 326 buffer hits
  (~265x fewer).
- `area_grants`: `Index Scan using area_grants_actor_lookup_idx`, **0.193 ms**
  execution time.

`operations` (the durable job queue) was already correctly indexed —
`claim_operation`'s exact query shape (`tenant_id`, `state`/`lease_expires_at`
predicate, `ORDER BY created_at FOR UPDATE SKIP LOCKED`) already used
`Index Scan using operations_reclaim_idx`, confirmed by `EXPLAIN ANALYZE`
before any change; no migration was needed there. `actors` lookups by
`tenant_id`+`actor_id` and by `subject` still plan as `Seq Scan` at this
data volume (~104 rows) — this is the planner correctly preferring a
sequential scan on a genuinely tiny table, not a missing-index signal, and
no index was added for it.

**Verification that the fresh-from-empty migration chain and all live
suites still pass** after the new migration: `docker compose up migrate`
(fresh volume) exited 0; `cargo test -p engrave-storage --locked -- --ignored --test-threads=1`
— all 8 live tests pass (`live_phase_f` x3, `live_queue` x2,
`live_repository` x2, `live_search_concurrency` x1), 0 failed.

### Memory usage

`/usr/bin/time -l cargo test -p engrave-storage --test live_search_concurrency --locked -- --ignored --nocapture`
— maximum resident set size **188,268,544 bytes** (~188 MB). This is a
`cargo test` harness measurement (includes the test binary, tokio runtime,
PostgreSQL client, and LanceDB/Arrow dependencies loaded in-process), not an
isolated production API-process footprint claim, consistent with how the
prior entry's own memory numbers are qualified.

### Provider usage, cost, and quality metrics — re-measured fresh, live

Correction: `VOYAGE_API_KEY`/`OPENAI_API_KEY` were not in the shell
environment or `.env`, but **were** present in `.env.local`, which
`scripts/benchmark-*.py` already auto-load (`env()` helper, `Path('.env.local')`).
`.env.local` is git-ignored (`.gitignore`'s `.env.*` pattern) and its values
were never printed or logged. All four provider-backed benchmark scripts
were run live against the real Voyage and OpenAI APIs this session:

- `python3 scripts/benchmark-voyage-provider.py` — native dimension 512
  confirmed; cold p50 **361.12 ms** (min 300.10, max 531.84); repeated-
  request p50 **296.43 ms** (min 289.36, max 368.70).
- `python3 scripts/benchmark-openai-provider.py` — native dimension 3072
  confirmed; cold p50 **918.53 ms** (min 298.32, max 1470.36 — one slow
  outlier pulled the median up; min is consistent with Voyage's cold
  latency); repeated-request p50 **223.67 ms** (min 217.24, max 312.65).
- `python3 scripts/benchmark-phase-e-retrieval.py` — first attempt hit a
  30 s socket timeout on the Voyage batch call (transient network flake,
  not a code defect) and was retried once, succeeding. Both profiles:
  recall@5 **1.0**, recall@10 **1.0**, MRR **0.6667**, nDCG@10 **0.6667**,
  **0** unauthorized and **0** wrong-Area results after filtering — with
  **9** unauthorized and **3** wrong-Area candidates present *before*
  filtering, proving the authorization boundary is genuinely exercised, not
  vacuously passing. Cache-miss latency: Voyage 301.93 ms, OpenAI 285.10 ms.
  Cache-hit latency: **0.00087 ms** for both (a plain dict lookup in the
  harness). Authorized fixture end-to-end p50/p99: Voyage 285.65/296.51 ms,
  OpenAI 241.73/276.17 ms. Provider usage: 69 tokens both. Cost estimates:
  Voyage $1.2e-10/query, $1.38e-9/run; OpenAI $7.8e-7/query, $8.97e-6/run
  (published standard list price, excludes credits/discounts/free tier).
  Full output: this run was not saved to `eval/results/` as a new dated
  file (the existing 2026-08-03 result files from the original credentialed
  run already cover this fixture/profile combination; these numbers refresh
  rather than duplicate that evidence).
- `python3 scripts/benchmark-provider-operations.py` — bounded concurrency
  1/4/8, both profiles, 0 errors at every concurrency level:

  | Provider | Concurrency | p50 (ms) | p95 (ms) | p99 (ms) | Wall (ms) |
  |---|---|---|---|---|---|
  | Voyage | 1 | 328.49 | 328.49 | 328.49 | 331.88 |
  | Voyage | 4 | 297.07 | 299.59 | 332.13 | 333.59 |
  | Voyage | 8 | 294.02 | 318.59 | 318.67 | 319.66 |
  | OpenAI | 1 | 359.55 | 359.55 | 359.55 | 360.03 |
  | OpenAI | 4 | 295.69 | 296.05 | 298.61 | 299.36 |
  | OpenAI | 8 | 212.74 | 225.12 | 228.19 | 229.05 |

  Rate-limit headers were captured per request (both providers report
  generous remaining-request/token budgets at this volume); no throttling
  was observed at concurrency 8.

These are all fresh, live numbers from this session, consistent in shape
with (and superseding, not merely repeating) the 2026-08-03 entry above.

**Recall@20 is still not measured** — this is independent of credential
availability: `BenchmarkMetrics`/`evaluate_benchmark` in
`crates/core/src/retrieval.rs` only implement `recall_at_5`/`recall_at_10`,
and `scripts/benchmark-phase-e-retrieval.py`'s fixture has only 3 gold-
labeled documents total, too few to meaningfully evaluate a @20 cutoff even
if the metric existed. This remains a recorded limitation, not a completed
measurement, and does not change the ANN gate conclusion in the section
above (recall@20 is still unavailable on the LanceDB latency fixture, which
has no gold labels of any kind).

### Full verification gate (this session)

```bash
cargo fmt --all -- --check                                          # pass
cargo test --workspace --locked                                     # pass — 95 tests, 8 correctly ignored, 0 failed
cargo clippy --workspace --all-targets --locked -- -D warnings      # pass
cargo deny check                                                    # pass
./scripts/check-openapi.sh                                          # pass, no diff
npm run build   # apps/web                                          # pass
docker compose up migrate   # fresh volume, chain 001-008           # exit 0
DATABASE_URL=postgres://engrave_app:engrave_local_only_change_me@localhost:5432/engrave \
  cargo test -p engrave-storage --locked -- --ignored --test-threads=1   # pass — 8/8
```

### Decisions

- Provider/DB clients (`PgRepository`, `LanceProjectionAdapter`,
  `VoyageEmbeddingClient`, `OpenAiEmbeddingClient`) are constructed once at
  process startup and held in `AppState`; none are constructed per request.
- Lexical retrieval and query embedding run concurrently via `tokio::join!`
  after authorization is resolved; a failed or slow embedding branch cannot
  fail or block lexical results.
- The query-embedding cache key includes provider, model, model version,
  input type, projection version, configuration fingerprint, and a
  normalized query string.
- ANN remains **not** the default. The gate is unmet because recall@20 is
  unmeasured on any available fixture — this is independent of, and does
  not require resolving, the fresh latency measurement's disagreement with
  the prior recorded ANN figure.
- Two indexes were added, each justified by a measured `EXPLAIN ANALYZE`
  plan change (`Nested Loop`/`Seq Scan` → `Hash Join`/`Index Scan`), not
  spec speculation.

### Limitations

- Live-credentialed provider benchmarks ran this session (see above) —
  cold/warm latency and cost figures are fresh, not carried over. One
  transient socket timeout occurred on the first `benchmark-phase-e-retrieval.py`
  attempt and was resolved by a single retry.
- The fresh ANN-vs-exact latency measurement disagrees sharply with the
  prior recorded figure; the cause is not diagnosed here.
- Recall@20 is not implemented anywhere in this repository's benchmark
  tooling.
- The synthetic PostgreSQL performance dataset (4,000 memories, 400
  area_grants) is far below real production scale; the measured ~26x join
  improvement is expected to grow, not shrink, at larger scale, but that is
  not itself measured.

## Open questions and limitations

- Provider integration has bounded cache behavior, retry hints, API
  circuit-breaker admission/feedback, and a 1,000-item capacity benchmark.
- Durable retrieval job handlers have live queue proof for claim, checkpoint,
  renew, expiry/reclaim, retry, cancellation, dead-letter, and manual retry;
  provider retry/backoff is proven with a client fault-injection test, while a
  and a live worker retrieval operation completed successfully; deployment
  environments may add provider-specific fault injection.
- The requested Obsidian Phase E plan notes were absent from this checkout; the
  ADR, roadmap, retrieval contract, source, and this ledger are the current
  repository evidence.
- Phase F has not been started. Larger, domain-specific corpora and ANN tail
  tuning remain follow-up work, but they are not hidden as completion evidence.
- **Superseded by the 2026-08-03 performance pass above:** ANN tail latency
  was re-measured (4 fresh runs) and now disagrees with the figure recorded
  here — treat the performance-pass section as current; this line is kept
  for history. The ANN gate remains unmet regardless, because recall@20 has
  never been measured on any fixture in this repository.
- Two PostgreSQL indexes were added in the performance pass
  (`migrations/20260809100000_phase_e_performance_indexes.sql`), each
  justified by a measured `EXPLAIN ANALYZE` plan change. Real production
  scale (beyond the 4,000-memory synthetic dataset used to justify them)
  remains unproven.

## Phase G implementation ledger — 2026-08-03

Implemented the initial Phase G contract slice: declarative typed Rules and
versioned policy envelopes in core; shared pre-tool gateway; HTTP first-gate
evaluation and retrieval-scope narrowing; MCP use of the shared evaluator;
durable Rule/version/test/decision/approval/conflict/review/idempotency schema;
and a seed-stable, credential-free fixture generator.

Evidence from this checkout:

```text
cargo fmt --all -- --check                         pass
cargo test --workspace --locked                    pass (full suite; 75 core tests)
cargo clippy --workspace --all-targets --locked -- -D warnings  pass
git diff --check                                   pass
DATABASE_URL=... cargo test -p engrave-storage --test live_phase_f --locked -- --ignored phase_g_rule_repository_and_decision_are_live --test-threads=1  pass
DATABASE_URL=... cargo test -p engrave-api --locked -- --ignored live_http_preflight_loads_active_rule_and_records_block --test-threads=1  pass
docker compose down -v && docker compose up migrate        pass (empty PostgreSQL volume, migrations 001-009)
npm run build (apps/web)                              pass
```

The existing Phase E baseline remains documented: MRR/nDCG@10 = 0.6667,
Recall@20 is not measured, and ranking quality is not a Phase G gate. No ANN
tuning or production-readiness claim is made. Active Rules now load from
PostgreSQL at the API retrieval boundary. The core killer-path harness blocks
retrieval, disclosure, and tool calls mechanically; the live Rule and HTTP
tests cover activation, active-version loading, idempotent replay, durable
decision recording, and HTTP blocking before retrieval.

## Research links

- [OpenAI embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)
- [OpenAI text-embedding-3-large pricing and rate limits](https://developers.openai.com/api/docs/models/text-embedding-3-large)
- [Cohere Embed API](https://docs.cohere.com/v2/reference/embed)
- [Cohere Embed model details](https://docs.cohere.com/docs/cohere-embed)
- [Voyage pricing](https://docs.voyageai.com/docs/pricing)
