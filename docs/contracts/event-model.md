# Event model

Every state-changing command returns or records the event it will append. The
domain change and event append commit in one database transaction. An event is
not emitted for a mutation that rolls back.

## Fields

| Field | Requirement |
|---|---|
| `event_id` | Opaque UUIDv7, globally unique |
| `tenant_id` | Always present for tenant-owned events |
| `actor_id`, `agent_identity_id`, `session_id` | Caller context; nullable only when explicitly system-owned |
| `action` | Initial taxonomy below |
| `target_type`, `target_id` | Stable target identity |
| `previous_version`, `resulting_version` | Version transition, when applicable |
| `reason` | Human-readable safe reason; no private Source body |
| `policy_result` | Rule effect, Rule ID/version, rationale when evaluated |
| `request_id`, `idempotency_key` | Correlation and replay context |
| `occurred_at` | UTC instant |

## Action taxonomy

`create`, `update`, `transition`, `approve`, `reject`, `merge`, `withdraw`,
`supersede`, `expire`, `archive`, `delete`, `quarantine`, `enqueue`, `lease`,
`retry`, `complete`, `cancel`, `replay`, `correct`.

## Target types

`tenant`, `actor`, `membership`, `role`, `agent_identity`, `area`,
`area_grant`, `placement`, `source`, `source_version`, `artifact`, `chunk`,
`entity`, `relationship`, `map_version`, `cross_map_mapping`, `memory`,
`memory_version`, `evidence_link`, `proposal`, `rule`, `operation`, `ai_run`,
`decision_envelope`.

Events are append-only. Redaction, where legally required, creates an explicit
redaction event while preserving the audit envelope and sequence.
