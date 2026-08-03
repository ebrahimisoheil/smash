# ADR-0022: Phase E bounded retrieval contract

## Status

Accepted for Phase E implementation on `v2-publish-d`.

## Decision

Engrave V2 owns retrieval eligibility, ranking, packet bounds, provenance, and
degraded-mode semantics in the framework-free `engrave-core` crate. Storage
adapters provide canonical records and rebuildable projections; they do not
reimplement authorization or silently broaden a query scope.

The eligibility predicate is applied before lexical or dense candidate
generation and requires the active tenant, a server-derived permitted Area,
approved/current Memory, valid time bounds, applicable purpose, and permitted
visibility. Private records are not returned by the Light Search contract.

Dense retrieval is provider-neutral. A projection is identified by provider,
model, model version, dimension, projection version, and configuration
fingerprint. Mixed projection identities and dimension mismatches are rejected.
The exact normalized vector path is the correctness reference; ANN is not
enabled by this decision.

Light Search uses bounded entry pools, deterministic typed-ID tie breaks, RRF
or explicitly selected weighted fusion, explainable reasons, provenance,
lineage/contradiction warnings, and greedy token-budget packing. When dense
infrastructure is absent or stale, lexical results remain available and the
packet reports a degraded mode.

## Evidence

- `crates/core/src/retrieval.rs` contains the core contracts and deterministic
  tests for E1–E5.
- `migrations/20260805100000_phase_e_retrieval.sql` defines tenant-isolated,
  rebuildable Memory/Chunk vector projections and resumable index jobs.
- `eval/fixtures/sales/benchmark.toml` defines the V2 query, exclusion,
  comparison-profile, and exact-vector reference fixture.
- `cargo test --workspace --locked` and clippy run these checks in CI.

## Not decided

No embedding provider, model, dimension, ANN index, reranker, or tuning value
is promoted to a permanent production default by this ADR. Those require a
measured V2 benchmark report.
