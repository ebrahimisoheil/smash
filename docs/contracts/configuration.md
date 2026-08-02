# Configuration contract

## Resolution and boot

Services resolve **defaults → file → environment → secret references**. The
resolved value is parsed, cross-field validated, and frozen before the service
binds its listener or accepts queue work. Invalid or missing required values
fail fast with a safe field name and no secret value.

## Required/optional matrix

| Setting | API | Worker | MCP | Behavior when missing |
|---|---|---|---|---|
| `service.environment` | required | required | required | Boot failure |
| `service.bind` | required | optional | optional | API boot failure; non-HTTP services use default |
| `database.url` | required for readiness | required for work | required for reads | Boot failure |
| `object_store.endpoint` | required for Source work | required | optional | Worker/API readiness failure if Source features enabled |
| `queue.poll_interval` | optional | optional | n/a | Validated default |
| `auth.issuer`, `auth.audience` | required before network exposure | required for authenticated work | required | Boot failure |
| `secrets.application_key_ref` | required when connector secrets exist | required when connector secrets exist | required when connector secrets exist | Boot failure on first configured secret, never plaintext fallback |
| `telemetry.endpoint` | optional | optional | optional | Local structured logs only; no product ledger sampling |

Environment variables may override non-secret values. Secret references resolve
through the configured secret provider; raw secret material is never persisted
in the database or emitted in logs.
