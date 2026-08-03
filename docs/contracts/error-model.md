# Error model and stable registry

`engrave-core` owns one application error enum. The API has one mapping from that
enum to HTTP status and this body shape:

```json
{
  "code": "memory.version_conflict",
  "message": "The memory changed; reconcile with the current version.",
  "request_id": "req_…",
  "rule": null
}
```

`rule` is present only for policy outcomes and contains `rule_id`, `version`,
`effect`, and a safe rationale. The mapping is deterministic and panics are
defects.

## Registry

| Code | HTTP | Meaning |
|---|---:|---|
| `auth.unauthenticated` | 401 | No valid caller authentication |
| `auth.forbidden` | 403 | Caller lacks the required tenant/Area/purpose grant |
| `tenant.not_found` | 404 | Tenant is not visible to the caller |
| `resource.not_found` | 404 | Resource is not visible or does not exist |
| `request.invalid` | 400 | Payload or query cannot be parsed/validated |
| `request.idempotency_conflict` | 409 | Key was reused with a different command |
| `resource.version_conflict` | 409 | Optimistic version token is stale |
| `policy.blocked` | 403 | A Rule blocked the operation |
| `policy.approval_required` | 409 | A Rule requires an approval transition |
| `source.quarantined` | 409 | Source cannot enter normal processing |
| `operation.not_found` | 404 | Operation is not visible or does not exist |
| `operation.failed` | 502 | Operation ended with a recorded failure |
| `dependency.unavailable` | 503 | Required dependency is unavailable |
| `internal.unexpected` | 500 | Safe generic fallback; details remain server-side |

Codes are stable once published. A new semantic failure gets a new code; it is
not silently aliased to an unrelated code.
