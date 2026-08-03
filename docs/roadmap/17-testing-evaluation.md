# 17 — Testing and Evaluation Strategy

> Source: the historical roadmap source §18

Testing follows **product invariants**, not framework layers alone.

Because the domain and application core is a framework-free crate, **contract tests run against `core` directly** — no HTTP server, no Axum test client. The API, worker, and MCP surfaces get thinner tests that check translation, middleware behavior, and error mapping. A rule that only holds when called over HTTP is in the wrong place.

The workspace also gates on `cargo clippy` with warnings denied, `cargo audit`, and `cargo deny`. See [15 — Release gates](15-auth-security.md#release-gates).

## 17.1 Contract tests

Test:

- Memory lifecycle;
- proposal review;
- duplicate refusal;
- contradiction handling;
- supersession;
- applicability;
- expiry;
- visibility;
- Cross-Map mapping;
- Rule precedence;
- Event emission;
- idempotency;
- optimistic concurrency.

## 17.2 Storage tests

Test:

- SQLx migrations from an empty database and from supported previous versions;
- transaction rollback;
- foreign-key integrity;
- PostgreSQL backup/restore;
- MinIO upload and finalization;
- object deletion coordination;
- LanceDB reconciliation;
- full index rebuild.

## 17.3 Retrieval tests

- Adapt V1 benchmark datasets to V2.
- Maintain lexical-only, hybrid, and reranked configurations.
- Add multi-Area, Cross-Map, visibility, stale-memory, contradiction, and aggressive-search traces.
- Measure regressions in CI with **deterministic fixtures**.
- Run model-dependent suites in a controlled evaluation pipeline.

Metrics list: [09 — Retrieval architecture §9.5](09-retrieval-architecture.md#95-ranking-evaluation).

## 17.4 Security tests

Test:

- cross-environment ID guessing;
- vector-filter bypass;
- blocked Cross-Map traversal;
- Source prompt injection;
- malicious archives;
- oversized documents;
- connector token isolation;
- Rule bypass;
- approval replay;
- audit immutability.

## 17.5 End-to-end tests

The **essential end-to-end proof**:

> One Source set produces a reviewed Memory that two different agents retrieve, followed by a Rule mechanically blocking unsafe reuse of private evidence.

Test the complete path through upload → worker → review → retrieval → MCP → Activity.

## 17.6 UI tests

Test:

- keyboard and mobile navigation;
- Source upload progress;
- Review editing;
- Graph search;
- saved record details;
- Light/Aggressive search distinction;
- empty states;
- error recovery.

**Accessibility is a release requirement:** semantic structure, focus management, contrast, labels, reduced motion.
