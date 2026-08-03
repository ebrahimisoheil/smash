# API conventions

## Resource shape

- All public routes use `/v1` and stable opaque IDs.
- Resources are nouns. An action endpoint is allowed only when a state
  transition cannot be represented as a resource update.
- Collections use cursor pagination with an opaque `next_cursor`; offsets are
  not part of the contract.
- Long work returns an Operation resource and a polling URL. Blocking requests
  are reserved for bounded, fast validation and reads.

## Mutation headers

| Header | Required for | Semantics |
|---|---|---|
| `Idempotency-Key` | POST/commands that can create or enqueue work | Scoped to tenant, authenticated principal, route, and command family. A replay returns the original response; a payload mismatch is a conflict. Retention is at least the operation replay window. |
| `If-Match` | Updates and transitions of versioned resources | Contains the version token the client reviewed. A stale token returns the current safe representation and requires reconciliation. |
| `X-Request-Id` | Optional input, always returned | Accepted only as a validated correlation value; the service generates one when absent. |

## Authorization and cross-cutting behavior

Authentication identifies the caller. `core` makes the authorization and Rule
decision before candidate generation, Source reads, Cross-Map traversal, or
mutation. Handlers translate validated requests to application commands and do
not reimplement policy.

## Responses

Success responses use resource DTOs from `engrave-contracts`. Errors use the
structured body defined in `error-model.md`. Bodies never include private
Source content, raw tokens, or secret material.
