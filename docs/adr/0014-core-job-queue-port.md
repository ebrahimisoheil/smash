# ADR-0014: Job queue as a core port

Status: Accepted  
Date: 2026-08-02

## Decision

The job queue is a trait in `smash-core`. It exposes enqueue, lease, lease
renewal, and completion using tenant and operation identities plus a lease
token. PostgreSQL `SELECT … FOR UPDATE SKIP LOCKED` is an adapter concern in
`storage`; managed queue infrastructure may replace it without changing
application services.

Lease expiry permits safe reclamation. Completion with an expired or foreign
lease is rejected, and handlers must be idempotent across retries.
