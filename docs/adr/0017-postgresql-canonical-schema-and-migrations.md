# ADR-0017: PostgreSQL canonical schema and forward-only migrations

Status: Accepted  
Date: 2026-08-02

## Decision

PostgreSQL is the canonical store for all structured and transactional Phase B
state. SQLx migrations are authored, forward-only, and run before API or worker
work acceptance. Constraints, foreign keys, tenant predicates, current-version
pointers, scoped idempotency, and event append-only behavior belong in the
database. No ORM model generator owns the schema.

The application role has no `BYPASSRLS`; repository queries always include an
explicit tenant predicate, with Row-Level Security as defense in depth.

## Consequences

Fresh-database migration and restart migration are separate gates. The
committed SQLx offline cache is part of the build artifact once query macros are
introduced.
