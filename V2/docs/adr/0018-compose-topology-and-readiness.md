# ADR-0018: Compose topology and readiness

Status: Accepted  
Date: 2026-08-02

## Decision

Community Edition runs PostgreSQL, MinIO, API, worker, web, and one-shot
migration/initialization jobs in one Compose project. PostgreSQL and MinIO use
named volumes and health checks. API and worker depend on successful migration
completion, not merely container start order. Readiness checks are non-mutating.

Local ports are configurable with defaults: API 3000, web 3001, PostgreSQL
5432, MinIO API 9000, and MinIO console 9001. Local credentials are explicit
development-only placeholders and are never production defaults.
