# ADR-0007: Transactional domain events

## Status

Accepted

## Context

Activity, audit, replay, evaluation, and downstream projections all depend on
knowing what changed and why. An event emitted after a database commit can be
lost; an event emitted before a failed commit can describe a change that never
existed. The domain model therefore requires every state-changing action to
append an immutable `Event` in the same transaction as the state change.

## Decision

Every state-changing command in `engrave-core` produces an explicit event intent
as part of its result or mutation contract. The `storage` adapter persists the
aggregate change and the corresponding event atomically in one PostgreSQL
transaction. A command is successful only when both writes commit; neither is
visible when either fails.

An event contains, at minimum:

- `EventId`, tenant and Area scope, and a UTC timestamp;
- actor, agent identity, session, and request identifiers when available;
- action name and target type plus typed target ID;
- previous and resulting version tokens;
- reason, policy result, and idempotency key when applicable;
- a schema/version marker and a redacted, structured payload.

Events are append-only. Corrections are new events, never updates or deletes.
The event is the audit/activity record; telemetry spans remain operational
observability and are not a substitute for the tenant decision ledger.

Downstream projections and jobs consume committed events after the transaction
and must be idempotent. A later event-dispatch mechanism may use an outbox or
logical change feed, but it must preserve the atomicity rule and event identity.

## Consequences

- The core mutation contract makes the normally-forgotten audit emission
  visible in code review and tests.
- Audit history cannot claim a state transition that rolled back.
- Consumers must tolerate duplicate delivery and preserve event ordering per
  aggregate where ordering is meaningful.
- Event payload schemas need compatibility discipline; private evidence must
  remain referenced by authorization-aware IDs rather than copied into events.
- Transactional writes add database work and require adapters to pass a
  transaction context through the mutation.

## Alternatives rejected and why

- **Emit after commit in application code** — rejected because a process crash
  can leave committed state without its required event.
- **Publish to a broker before committing** — rejected because a rollback can
  leave a false event; broker delivery is a downstream concern.
- **Database triggers as the only event contract** — rejected because triggers
  hide domain intent from `core` signatures and cannot express all actor,
  agent, policy, and request context cleanly.
- **Treat tracing spans as audit events** — rejected because telemetry has
  different retention, sampling, and tenant-governance requirements.

## Supersedes / superseded by

None.
