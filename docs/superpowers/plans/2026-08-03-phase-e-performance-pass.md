# Phase E Retrieval Performance Pass Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Light Search's HTTP path reuse persistent provider/DB clients instead of building them per request, run lexical retrieval and query embedding concurrently after authorization, complete the query-embedding cache key, re-confirm the ANN-vs-exact gate with fresh evidence, verify PostgreSQL query plans are indexed and authorization-prefiltered, and record all of it with real measurements in `docs/phase-e-ledger.md`.

**Architecture:** All changes are additive/refactoring inside the existing Phase E surfaces (`crates/api/src/main.rs`'s `AppState`/`search` handler, `crates/core/src/retrieval.rs`'s cache key shape, `crates/storage/src/lib.rs`'s LanceDB/Postgres adapters). No new crates, no schema changes unless `EXPLAIN ANALYZE` on the live local Postgres instance justifies one.

**Tech Stack:** Rust (Axum, tokio, sqlx, lancedb), Python benchmark scripts already in `scripts/`, local Docker Postgres (already running from the prior session) and LanceDB.

## Global Constraints

- No live Voyage/OpenAI API keys are configured in this environment (`VOYAGE_API_KEY`/`OPENAI_API_KEY` unset). Real external-provider cold/warm latency and cost numbers **cannot be freshly measured this session** — the existing real measurements already recorded in `docs/phase-e-ledger.md` (dated 2026-08-03, from a prior credentialed run) are cited as the still-valid baseline, explicitly labeled as carried over, not re-run. Everything else (concurrency, caching, Postgres, LanceDB exact/ANN, deterministic-provider end-to-end) is measured fresh on this machine.
- Never invent a number. Every figure in the ledger update must come from an actual command run this session, with the exact command shown.
- Do not touch Phase G/H/I. Do not weaken any Phase D/E authorization, lifecycle, or ranking invariant already covered by existing tests — all pre-existing tests must keep passing unmodified.
- Preserve exact-search-as-default: do not flip any default to ANN. The ANN gate (recall@20 within 1pp of exact AND improved p99) is evaluated with fresh evidence, not assumed.
- `docs/phase-e-ledger.md` is the target document for the final write-up — update it in place, do not create a parallel document.

---

### Task 1: Persistent provider clients and connections in `AppState`

**Files:**
- Modify: `crates/api/src/main.rs`

**Interfaces:**
- Consumes: `engrave_storage::{VoyageEmbeddingClient, OpenAiEmbeddingClient}` (already `Clone`, already used via `from_env()`).
- Produces: `AppState.voyage_client: Option<Arc<VoyageEmbeddingClient>>`, `AppState.openai_client: Option<Arc<OpenAiEmbeddingClient>>`, built once in `app_with_retrieval`/`main()`, never inside the `search` handler.

- [ ] **Step 1:** Add the two fields to `struct AppState` (`crates/api/src/main.rs:48-58`).
- [ ] **Step 2:** In `app_with_retrieval` (`crates/api/src/main.rs:860+`), construct them once: `voyage_client: VoyageEmbeddingClient::from_env().ok().map(Arc::new)`, same for OpenAI. This mirrors the existing `Option<Arc<PgRepository>>`/`Option<Arc<LanceProjectionAdapter>>` pattern exactly.
- [ ] **Step 3:** In `search` (`crates/api/src/main.rs:691-728` and `729-766`), replace `VoyageEmbeddingClient::from_env()` / `OpenAiEmbeddingClient::from_env()` with `state.voyage_client.clone()` / `state.openai_client.clone()`, mapping `None` to the same `"...provider configuration unavailable..."` degraded-mode reason the `Err(error)` branch already produces (so behavior when a key is absent is unchanged — still lexical fallback with an explanatory reason, not a different error).
- [ ] **Step 4:** Run `cargo test -p engrave-api --locked` — all 13 existing tests (8 original + 5 Phase F adversarial) must still pass unmodified, since no key is configured in the test environment and both clients are `None`, producing the same fallback path as before.

---

### Task 2: Concurrent lexical retrieval and query embedding

**Files:**
- Modify: `crates/api/src/main.rs`

**Interfaces:**
- Produces: the `search` handler resolves authorization first (unchanged), then runs the lexical branch and the full embedding-to-dense-hits branch as two futures joined with `tokio::join!`, so neither blocks the other, and a failed/degraded embedding branch never affects the lexical `Result`.

- [ ] **Step 1:** Extract the current embedding-through-`dense_hits` block (`crates/api/src/main.rs:612-774`) into a single `async fn resolve_dense_hits(state: &AppState, repository: &PgRepository, request: &CoreSearchRequest) -> (Vec<DenseHit>, DegradedMode)` that never returns `Err` — every failure path already assigns a `DegradedMode::LexicalOnly { reason }` and falls through, so this is a mechanical extraction, not new logic.
- [ ] **Step 2:** In `search`, after `request` is built, replace the sequential `let lexical = repository.search_lexical(&request).await...?;` followed by the embedding block with:
  ```rust
  let (lexical_result, (dense, degraded)) = tokio::join!(
      repository.search_lexical(&request),
      resolve_dense_hits(&state, &repository, &request),
  );
  let lexical = lexical_result.map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)?;
  ```
  This preserves "resolve authorization first" (already done before this point), bounded concurrency (the embedding branch still acquires `state.embedding_concurrency` exactly as before), and timeout limits (the existing 250 ms permit-acquire timeout is untouched, it now just runs concurrently with lexical instead of after it).
- [ ] **Step 3:** Add a unit-level regression test in `crates/api/src/main.rs`'s `#[cfg(test)] mod tests`: `search_lexical_result_is_unaffected_by_embedding_branch_failure` — call `/v1/search` with no `ENGRAVE_EMBEDDING_PROFILE` set (so the embedding branch takes the "no embedding provider configured" fallback path) against a repository-less `app()` (which already returns 503 before reaching this code — so instead, verify via the existing `app_with_retrieval(Some(repository), None)` pattern used by other tests, if one exists, or document why a full assertion requires a live Postgres and defer the concurrency proof to the benchmark step below). If no existing test scaffolding supports a non-503 `/v1/search` call without a live repository, skip adding a redundant unit test here and instead prove non-interference via the benchmark harness in Task 6 (measuring that lexical results are identical whether or not `ENGRAVE_EMBEDDING_PROFILE` is set to a failing profile) — record which approach was actually used.
- [ ] **Step 4:** `cargo test -p engrave-api --locked` still green.

---

### Task 3: Complete the query-embedding cache key

**Files:**
- Modify: `crates/api/src/main.rs` (cache-key construction, `~line 658`)
- Modify: `crates/core/src/retrieval.rs` (tests only, to prove profile isolation, invalidation, capacity)

**Interfaces:**
- Produces: cache key includes provider, model, model version, input type (hardcoded `"query"` for Light Search — the only input type this path ever embeds), projection version, configuration fingerprint, and a **normalized** query string (trimmed, whitespace-collapsed, lowercased) so two queries differing only in casing/whitespace hit the same cache entry.

- [ ] **Step 1:** Replace the cache-key `format!` at `crates/api/src/main.rs:658-665` with:
  ```rust
  let normalized_query = request.query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
  let cache_key = format!(
      "{}:{}:{}:query:{}:{}:{}",
      identity.provider, identity.model, identity.model_version,
      identity.projection_version, identity.configuration_fingerprint, normalized_query
  );
  ```
  Using `identity.provider` (already on `ProjectionIdentity`) instead of the loose `profile_name` string ties the key to the actual resolved provider identity, not just the configuration profile label.
- [ ] **Step 2:** Add tests to `crates/core/src/retrieval.rs`'s existing `query_embedding_cache_is_bounded_and_profile_scoped_by_key` test (or a new adjacent test) proving: cache hit (same key returns the stored vector), cache miss (different key returns `None`), profile isolation (two `ProjectionIdentity`s differing only in `provider` or `projection_version` produce different keys and do not collide), and capacity eviction (inserting beyond capacity drops an entry, `len()` stays at capacity). These map directly to the plan's required test coverage: "cache hit, miss, profile isolation, invalidation, and fallback behavior" — fallback behavior is covered by Task 2's concurrency test/benchmark (a cache miss/provider failure still degrades to lexical, never panics or 500s).
- [ ] **Step 3:** `cargo test -p engrave-core --lib retrieval:: --locked` green.

---

### Task 4: ANN gate re-confirmation with fresh evidence

**Files:** none changed — this is a benchmark-and-decision task, not a code task, unless the gate flips (it is not expected to, given the existing 10x p99 gap).

- [ ] **Step 1:** Run `cargo test -p engrave-storage --test live_lancedb -- --nocapture` (already exists, already produces exact vs. IVF_FLAT ANN p50/p95/p99 on a 1,000-row 1024-D fixture) against the current local LanceDB, and record the fresh output.
- [ ] **Step 2:** Compare against the prior recorded evidence (exact p99 ≈18.466 ms, ANN p99 ≈186.406 ms). If recall@20 and tail latency are not measured by the existing test, note this as a limitation rather than fabricating a recall number — `live_lancedb.rs`'s existing test measures latency and authorization filtering, not ranked recall against a gold set (recall@20 requires a gold-labeled query set, which only `scripts/benchmark-phase-e-retrieval.py`'s small fixture provides, not the 1,000-row synthetic latency fixture).
- [ ] **Step 3:** Record the gate decision explicitly in the ledger: exact remains default (ANN p99 is ~10x worse, not better, so the "p99 improves" half of the gate fails regardless of recall) — this is a re-confirmation, not a new decision.

---

### Task 5: PostgreSQL index and query-plan verification

**Files:**
- Read-only investigation against the local Postgres instance; modify a migration **only if** `EXPLAIN ANALYZE` shows a missing index causing a sequential scan on a tenant/Area/visibility/lifecycle/actor/lexical/queue predicate.

- [ ] **Step 1:** Connect to the local `docker compose` Postgres (already running, already migrated through `007_phase_f_live_adapter.sql`) and run `EXPLAIN ANALYZE` on: the `search_lexical` query shape (tenant/Area/lifecycle/visibility/applicability predicates against `memories`/`memory_versions`), the `claim_operation` queue-claim query (`FOR UPDATE SKIP LOCKED`), and a representative Phase F query (`entities`/`relationships` by tenant+area).
- [ ] **Step 2:** Cross-reference against existing indexes (`migrations/20260802120000_initial_schema.sql`'s `CREATE INDEX` statements, e.g. `tenant_actor_idx`, `area_tenant_idx`, `source_tenant_area_idx`, plus any full-text/GIN index for lexical search added in `20260806100000_phase_e_live_retrieval.sql`).
- [ ] **Step 3:** If every relevant predicate already hits an index (expected, since Phase E's own ledger already claims authorization-first prefiltering), record the `EXPLAIN ANALYZE` output as evidence and do not add a migration. If a genuine gap is found, add a narrowly-scoped `CREATE INDEX IF NOT EXISTS` migration and re-run `EXPLAIN ANALYZE` to prove the plan changed from a sequential scan to an index scan.

---

### Task 6: Full benchmark run and `docs/phase-e-ledger.md` update

**Files:**
- Modify: `docs/phase-e-ledger.md`

- [ ] **Step 1:** Run, on this machine, with the deterministic-dev provider (no external credentials required) and the local Postgres/LanceDB: `scripts/benchmark-phase-e-retrieval.py` (recall@5/10/20, MRR, nDCG@10, unauthorized/wrong-Area rates, cache-hit vs. cache-miss latency, end-to-end latency), plus a small before/after sequential-vs-concurrent timing comparison of the `/v1/search` handler (Task 2's change) using the deterministic profile.
- [ ] **Step 2:** Re-run `cargo test -p engrave-storage --test live_lancedb -- --nocapture` for fresh exact/ANN numbers (Task 4).
- [ ] **Step 3:** Capture `EXPLAIN ANALYZE` output (Task 5) verbatim.
- [ ] **Step 4:** Append a new dated section to `docs/phase-e-ledger.md` (do not delete prior sections) recording: exact commands, dataset size/seed, hardware (this machine, local Docker), concurrency/batch size used, provider/model versions and projection fingerprints for anything provider-dependent, every benchmark category requested, decisions (persistent clients, concurrency, cache key, ANN-stays-default, Postgres-indexes-sufficient-or-added), and limitations (no live provider credentials this session; external cold/warm latency and cost are carried over from the prior dated entry, not re-measured).

---

### Task 7: Full verification gate

- [ ] Run in order and record exact output: `cargo fmt --all -- --check`, `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `./scripts/check-openapi.sh`, `npm run build` (from `apps/web`), plus the relevant live PostgreSQL/LanceDB/worker/authorization tests (`live_repository`, `live_lancedb`, `live_queue`, `live_phase_f`) with `--ignored` against the local database.
- [ ] Do not report the performance pass complete unless every one of these passes and the benchmark evidence is actually recorded in `docs/phase-e-ledger.md`.

## Self-review notes

- Spec coverage: all five numbered requirements from the user's request map to Tasks 1–5; the benchmark/testing/ledger requirements map to Task 6; the gate list maps to Task 7.
- No placeholders: every code change is described with the exact before/after; the one intentionally-open item (Task 2 Step 3's test-or-benchmark choice) is explicit about why and what the fallback proof is, not a vague TODO.
- Honesty constraint carried through explicitly in Global Constraints and Task 4/6: no external-provider numbers will be fabricated; missing-credential limitation is stated as a limitation, not hidden.
