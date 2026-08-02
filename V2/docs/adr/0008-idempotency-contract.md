# ADR-0008: Scoped idempotency contract

## Status

Accepted

## Context

Retries are normal for network calls, workers, and agent tool invocations. A
retry must not create a second Proposal, Memory version, payment-like side
effect, or audit event. At the same time, a key from one tenant or command must
not suppress an unrelated command.

## Decision

Mutating commands accept an `Idempotency-Key` with 1–255 visible ASCII
characters. The effective scope is the authenticated tenant, actor or agent
identity, command/endpoint name, and target resource when one exists. The
server stores the key together with a canonical request fingerprint, status,
response or command result, and expiry. The database enforces uniqueness on
the effective scope plus key.

Processing rules:

1. The first request claims the scoped key and runs the command in the same
   transaction as its state change and event.
2. A retry with the same scope, key, and fingerprint returns the recorded
   result without executing the command again. This includes committed
   failures that are safe to replay.
3. A retry while the original request is unresolved returns
   `IDEMPOTENCY_IN_PROGRESS`; it does not execute concurrently.
4. Reusing a key with a different fingerprint returns
   `IDEMPOTENCY_CONFLICT` and never runs the second request.
5. If the transaction rolls back before a terminal result is stored, the key
   claim is released or safely expired so a corrected retry can proceed.

The API/Tower layer captures and validates the header, but `core` command
contracts carry the idempotency context so workers and non-HTTP surfaces obey
the same rule. Successful and terminal results are retained for at least the
configured replay window; the default is 24 hours for synchronous commands
and until completion plus 24 hours for asynchronous Operations. Longer-lived
business deduplication belongs to domain identity and exact-duplicate rules,
not to indefinite idempotency-key retention.

The fingerprint excludes transport noise and includes the normalized command
payload and relevant target. Raw secrets are not stored in the fingerprint.

## Consequences

- Safe retries are a database invariant rather than a best-effort client rule.
- The same semantics work for HTTP, worker retries, and MCP calls.
- Storage retains result metadata for the replay window and must garbage
  collect expired records without breaking uniqueness for active keys.
- Clients must preserve a key across retries and generate a new key for a new
  intent.
- Commands that are deliberately non-idempotent must be explicitly documented
  and cannot silently bypass the contract.

## Alternatives rejected and why

- **Client-side deduplication only** — rejected because crashes and retries can
  happen after the server has committed but before the client receives a
  response.
- **One global key namespace** — rejected because unrelated tenants or
  commands would collide and create denial-of-service or correctness hazards.
- **Key lookup without request fingerprinting** — rejected because accidental
  key reuse would replay the wrong result instead of exposing a conflict.
- **Permanent key retention** — rejected because it creates unbounded storage;
  durable domain identity and duplicate detection handle longer horizons.

## Supersedes / superseded by

None.
