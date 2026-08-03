# V2 Eval Fixtures

The Sales fixture is the deterministic Phase E retrieval corpus. The lifecycle
fixture remains the canonical source of entities, evidence, and review history;
`fixtures/sales/benchmark.toml` adds retrieval queries, eligibility expectations,
comparison profiles, and a tiny exact-vector reference corpus.

The benchmark manifest is intentionally provider-neutral. Its vectors are a
correctness reference only, not a production embedding model or dimension
decision. A retrieval implementation must apply the manifest's authorization,
Area, lifecycle, and applicability exclusions before ranking candidates.

Phase E metrics are defined in `docs/contracts/retrieval-math.md`: Recall@5/10,
MRR, nDCG@10, latency percentiles, packet tokens, unauthorized-result rate,
wrong-Area rate, and visible degraded-mode behavior.
