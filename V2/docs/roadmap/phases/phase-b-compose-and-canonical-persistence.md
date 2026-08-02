# Phase B — Docker Compose and Canonical Persistence

> Source: SMASH_V2.md §20, Phase B

## Goal

A reproducible stack in which all canonical state is durable, transactional, and outside container filesystems.

## Scope

### Compose stack

Create the reproducible Compose stack with PostgreSQL, MinIO, the Axum API, the worker, and Next.js. Add:

- health checks;
- SQLx migration execution;
- bucket initialization;
- named volumes;
- environment examples;
- safe reset and backup instructions.

`api` and `worker` build from one Cargo workspace via a cached multi-stage Docker build and ship as slim runtime images containing compiled release binaries.

### Canonical PostgreSQL persistence

Implement persistence through **SQLx**, with compile-time-checked queries and a committed offline query cache, for:

- tenants, memberships, enterprise roles;
- Areas and Map versions;
- Sources and Source versions;
- chunks;
- entities and relationships;
- Memory;
- Proposals;
- Rules;
- Events;
- AI runs and decision envelopes;
- Operations and idempotency.

### MinIO

Implement object upload, decision snapshots, verification, and retrieval using **least-privilege service credentials**.

## Acceptance criteria

- [ ] A new contributor can start the complete stack with documented prerequisites.
- [ ] Startup waits for healthy dependencies and successful migrations.
- [ ] Restarts preserve state and do not duplicate initialization.
- [ ] All canonical mutations are transactional and append Events.
- [ ] Normal-user and Enterprise Admin requests are isolated correctly inside the same built-in tenant contract.
- [ ] Original Source bytes survive container replacement.
- [ ] Backup and restore is demonstrated on the fixture.
- [ ] No required durable state exists only inside a container filesystem.
- [ ] CI builds the workspace without a live database, using the committed SQLx offline cache.

## References

- [05 — Canonical storage responsibilities](../05-storage-responsibilities.md)
- [06 — Service architecture §6.4](../06-service-architecture.md#64-docker-compose-is-the-community-edition-product-unit)
- [17 — Testing §17.2 Storage tests](../17-testing-evaluation.md#172-storage-tests)
- [23 — Diagrams §24.2](../23-diagrams.md#242-community-edition-docker-compose-architecture)
