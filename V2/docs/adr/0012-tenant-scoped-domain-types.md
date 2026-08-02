# ADR-0012: Tenant-scoped domain types

Status: Accepted  
Date: 2026-08-02

## Decision

Every tenant-owned command, DTO, repository port, and domain record carries a
non-optional `TenantId`. Area-owned records additionally carry `AreaId`.
Authorization is evaluated in `core`; storage queries must still constrain
tenant explicitly, with PostgreSQL Row-Level Security as defense in depth.

Opaque IDs, not slugs or user-provided names, identify records. This narrows
the blast radius of a missing predicate without pretending the compiler can
prove SQL authorization.
