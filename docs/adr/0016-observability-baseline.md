# ADR-0016: Observability baseline

Status: Accepted  
Date: 2026-08-02

## Decision

Use Rust `tracing` with OpenTelemetry-compatible span context. A Tower layer
establishes request/span context before handlers; workers and outbound calls
propagate it. Operational telemetry may be sampled and excludes sensitive
content by default. The tenant decision ledger is canonical product data and
is never sampled away.

Structured records carry request, tenant, actor, agent, session, AI Run,
Operation, and trace identifiers. Forensic, execution, and counterfactual
replay never repeat external side effects by default.
