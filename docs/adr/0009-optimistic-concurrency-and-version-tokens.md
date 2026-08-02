# ADR-0009: Optimistic concurrency and version tokens

## Status

Accepted

## Context

Memory writes, proposal review, rule changes, and other mutable aggregates
may be edited by several actors or retried by workers. Last-write-wins would
silently discard a reviewed change and make the audit trail misleading. The
memory write contract explicitly requires a client to submit the version it
reviewed and to reconcile when another actor changed the record.

## Decision

Every mutable aggregate exposes a monotonically increasing version token. The
token is opaque at the API boundary and is serialized as a string; the initial
storage representation is a strictly increasing integer. Create operations
start at version `1`; each committed mutation increments the version exactly
once. The token is scoped to the aggregate and is not a global clock.

Mutating commands that operate on an existing aggregate must supply the
expected version token. The adapter performs the compare-and-set as part of
the same transaction as the state change and event. A mismatch affects zero
rows and returns `STALE_VERSION` with the current safe representation, current
version token, and request ID. The stale request never overwrites the current
state and never emits a domain event.

The HTTP surface accepts the token in the documented request field and may
also expose it as a strong `ETag`; `If-Match` is an equivalent transport form,
not a second concurrency model. Successful responses return the new token.
Clients reconcile by showing or merging the returned current state and then
submitting a new command with the new token. Force overwrite is not a hidden
fallback; any future administrative merge must be a separately named command
with its own authorization and event.

Idempotent retries of a committed command return the recorded result under
ADR-0008. A retried command with a new idempotency key still performs the
version check and therefore cannot bypass concurrency control.

## Consequences

- Lost updates become explicit, recoverable conflicts instead of silent data
  loss.
- Mutation signatures and database predicates must carry the expected token.
- Conflict responses can support good reconciliation UX without exposing
  private content beyond the caller's authorization scope.
- Version counters must not be reused or reset during normal updates; restores
  and migrations need a deliberate compatibility plan.
- Reads need to return the token that the client is expected to echo.

## Alternatives rejected and why

- **Last-write-wins** — rejected because it discards reviewed changes silently.
- **Pessimistic row locks for every request** — rejected as the default because
  long-running agent and review flows would hold locks across user think time
  and reduce availability.
- **Timestamp-only comparison** — rejected because clock precision and skew do
  not provide a reliable per-aggregate compare-and-set.
- **Opaque token with no server-side monotonic version** — rejected because
  the storage invariant and deterministic tests benefit from a simple strictly
  increasing counter; the counter remains hidden behind the token boundary.

## Supersedes / superseded by

None.
