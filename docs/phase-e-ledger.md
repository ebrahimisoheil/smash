# Phase E Ledger — E.2 provider compatibility and operational readiness

**Status:** Phase E.2 implementation and acceptance verification **complete**.
Production ANN deployment is not selected; exact search remains the default
reference path because the measured ANN p99 is higher on the capacity fixture.

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

## Research links

- [OpenAI embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)
- [OpenAI text-embedding-3-large pricing and rate limits](https://developers.openai.com/api/docs/models/text-embedding-3-large)
- [Cohere Embed API](https://docs.cohere.com/v2/reference/embed)
- [Cohere Embed model details](https://docs.cohere.com/docs/cohere-embed)
- [Voyage pricing](https://docs.voyageai.com/docs/pricing)
