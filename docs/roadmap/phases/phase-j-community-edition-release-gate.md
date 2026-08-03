# Phase J — Community Edition Release Gate

> Source: the historical roadmap source §20, Phase J

## The gate

The Community Edition is ready when a **non-maintainer** can:

1. install it;
2. add Sources;
3. review Memory;
4. connect at least two agents;
5. retrieve through Light and Aggressive modes;
6. inspect provenance;
7. enforce a Rule;
8. back up data;
9. upgrade through migrations

— **without repository knowledge.**

## Release artifacts

- versioned containers built from the Cargo workspace;
- prebuilt MCP server binaries for supported platforms, so local install needs no Rust toolchain;
- Compose files;
- configuration documentation;
- SQLx migration notes;
- MCP package and metadata (crates.io plus Registry metadata);
- skills;
- prompts;
- benchmark results;
- security policy;
- contribution guide;
- an export format.

## Honesty requirements

The open-source release is honest about supported formats, deployment boundaries, and scale.

- **Do not market a single-node Compose stack as an enterprise cluster.**
- **Do not hide essential memory governance behind managed-only services.**

## Forward compatibility

Community Edition already emits the stable **session, run, retrieval, Rule, tool, and decision identifiers** required by future AI Tracer.

It does not need the full analytics product, but **it must not discard the causal links that analytics and replay will require**.

## References

- [16 — Observability and operations](../16-observability-operations.md)
- [02 — Philosophy §2.5 Local ownership and managed convenience share one contract](../02-philosophy.md#25-local-ownership-and-managed-convenience-share-one-contract)
- [21 — Definition of product success](../21-product-success.md)
- Next: [20 — Post-Community managed service focus](../20-managed-service.md)
