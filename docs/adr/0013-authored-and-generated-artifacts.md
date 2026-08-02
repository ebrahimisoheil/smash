# ADR-0013: Authored and generated artifacts

Status: Accepted  
Date: 2026-08-02

## Decision

Rust domain types, configuration schemas, ADRs, and SQL migrations are
authored artifacts. OpenAPI is generated from `smash-contracts` through
Utoipa. The committed OpenAPI description is tested in CI and drift is a
failure. Generated output is never hand-edited to conceal a type mismatch.

## Consequences

The contract crate remains the single origin for published wire schemas. A
schema change is reviewed as a Rust type change plus regenerated output.
