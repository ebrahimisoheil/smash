# ADR-0006: Core error model and stable code registry

## Status

Accepted

## Context

The same domain failure can be reached through the HTTP API, worker, MCP
adapter, or direct contract tests. If each surface invents its own errors,
clients cannot reliably retry, reconcile, or display policy outcomes. HTTP
status alone is also too coarse to distinguish a stale write, a denied Area,
and an unavailable dependency.

The roadmap requires one application error enum in `core`, one HTTP mapping,
stable machine codes, and policy context without leaking private Source data or
token material.

## Decision

`engrave-core` owns one authoritative `AppError` enum. Domain and application
ports return this type (or a domain-specific error converted into it); adapters
may add transport context only at the boundary. The enum is converted exactly
once into the API error body.

Every externally meaningful variant has a stable, uppercase machine code. The
initial registry is:

| Code | HTTP status | Meaning |
|---|---:|---|
| `INVALID_ARGUMENT` | 400 | The request shape or value is invalid |
| `UNAUTHENTICATED` | 401 | No valid caller identity was established |
| `FORBIDDEN` | 403 | The caller is authenticated but lacks authority |
| `NOT_FOUND` | 404 | The resource is absent or intentionally undiscoverable |
| `CONFLICT` | 409 | The command conflicts with current domain state |
| `IDEMPOTENCY_CONFLICT` | 409 | A key was reused with a different request fingerprint |
| `IDEMPOTENCY_IN_PROGRESS` | 409 | The same command is currently being completed |
| `STALE_VERSION` | 409 | The caller's optimistic-concurrency token is stale |
| `UNPROCESSABLE` | 422 | The request is well-formed but violates a domain rule |
| `RATE_LIMITED` | 429 | A caller or resource limit was exceeded |
| `DEPENDENCY_UNAVAILABLE` | 503 | A required adapter or service is unavailable |
| `INTERNAL` | 500 | An unexpected defect or unclassified failure occurred |

The wire body contains `code`, a safe human `message`, `request_id`, and
optional structured fields: `field_errors`, `resource`, `current_version`, or
`policy`. A policy outcome may include the typed `RuleId`, rule version, and
rationale. It never includes raw Source content, credentials, access tokens,
SQL, or stack traces.

Codes are append-only and versioned in this ADR and the contract crate. A
meaning change or code reuse requires a new code and a superseding ADR, not a
silent reinterpretation. Unknown internal errors map to `INTERNAL` while the
full cause is logged with the request ID and redaction rules.

## Consequences

- API, worker, MCP, and direct core callers share retry and reconciliation
  semantics.
- Clients can depend on codes rather than parsing human messages.
- The registry becomes compatibility surface and must be reviewed like an API.
- Each adapter must deliberately map its native failures; accidental leakage
  of database or token details is avoided by construction.

## Alternatives rejected and why

- **HTTP status-only errors** — rejected because one status cannot express
  idempotency, stale versions, or policy-specific remediation.
- **One error type per transport** — rejected because it duplicates domain
  semantics and makes worker/MCP behavior drift from HTTP behavior.
- **Free-form string codes** — rejected because typos and code reuse would
  silently break clients.
- **Expose internal causes to aid debugging** — rejected because database
  details, tokens, and private evidence are security-sensitive; correlation
  belongs in logs keyed by `request_id`.

## Supersedes / superseded by

None.
