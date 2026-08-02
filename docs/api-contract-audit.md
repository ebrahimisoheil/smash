# Smash API Contract Audit

Date: August 1, 2026

Scope:
- dedicated runner: `python3 api.py --root /tmp/smash-api-smoke --port 3012`
- live workspace: disposable demo created with `python3 smash.py demo /tmp/smash-api-smoke --force`
- audit style: real HTTP calls against the loopback runner, including success and error cases

## Result

Live smoke passed for 36 endpoint calls against the dedicated `api.py` runner.

The API implementation is healthier than the current OpenAPI document. The main
contract risk is not endpoint failure; it is that `docs/openapi.yaml` is still
too permissive for many responses because it uses broad `GenericObject` shapes.

## Main Findings

1. The live API surface is broader than the human HTML route list.
   - `GET /api/operations` is live.
   - `GET /api/backlinks` is live.
   - These routes were working in the runner and are now documented in the
     machine-readable spec, but they should also remain visible in the human
     route inventory.

2. Error responses are not one universal shape.
   - `GET /api/search` without `q` returns:
     - `{ "error": "q parameter required", "results": [] }`
   - `GET /api/explain-memory` without `memory` returns:
     - `{ "found": false, "error": "memory parameter required" }`
   - `POST /api/rebuild-index` without the local-action header returns:
     - `{ "rebuilt": false, "error": "X-Smash-Local-Action header required for local mutations" }`
   - This means a single generic error schema is too weak to describe actual
     contracts for client code.

3. The success payloads have stable top-level structure today.
   - `GET /api` consistently returns:
     - `api_version`, `name`, `description`, `local_only`, `recommended`,
       `endpoints`, `write_header`
   - `GET /api/status` consistently returns:
     - readiness counts, validation summary, cache/index status, next actions,
       and `api_version`
   - Memory mutation endpoints consistently return top-level action fields like
     `saved`, `created` or `updated`, plus stable identifiers such as `name`,
     `path`, and `title`.

4. `GET /api/validate` is state-sensitive by design.
   - Before repair, the demo returned `422` for stale backlinks.
   - After rebuilds during the audit flow, the endpoint returned `200`.
   - This is correct behavior, but clients must treat `422` as a normal
     contract outcome rather than a transport failure.

5. `POST /api/propose-memories` does not require the local-action header.
   - This is consistent with the current implementation.
   - The endpoint proposes memory candidates but does not write durable memory.
   - Clients should not assume every POST route is guarded by
     `X-Smash-Local-Action`.

## Live Endpoints Audited

Read:
- `GET /api`
- `GET /api/status?validate=true`
- `GET /api/health`
- `GET /api/operations`
- `GET /api/prompts?project=alpha`
- `GET /api/ingest-status`
- `GET /api/pages`
- `GET /api/page-list?limit=5&offset=0`
- `GET /api/backlinks`
- `GET /api/page-links?page=Smash&limit=5&offset=0`
- `GET /api/validate?strict=true`
- `GET /api/graph`
- `GET /api/graph-summary?topic=Smash&limit=5&depth=1`
- `GET /api/search?q=Smash`
- `GET /api/context?topic=Smash`
- `GET /api/memory-profile?project=alpha`
- `GET /api/memory-dashboard?project=alpha`
- `GET /api/memory-brief?q=agent+memory&project=alpha`
- `GET /api/query-Smash?q=why+does+Smash+help+agents%3F&budget=small`
- `GET /api/memory-audit?project=alpha`
- `GET /api/memory-inbox?project=alpha`
- `GET /api/wins?project=alpha`
- `GET /api/memory-log?limit=10`
- `GET /api/capture-inbox?project=alpha`
- `GET /api/explain-memory?memory=prefer-local-personal-memory`
- `GET /api/proposal-sources`
- `GET /api/proposal-source?path=raw/contract-audit-note.md`

Write:
- `POST /api/raw-source`
- `POST /api/propose-memories`
- `POST /api/remember-memory`
- `POST /api/update-memory`
- `POST /api/review-memory`
- `POST /api/archive-memory`
- `POST /api/restore-memory`
- `POST /api/rebuild-backlinks`
- `POST /api/rebuild-index`

## Error Cases Audited

- `GET /api/search` -> `400`
- `GET /api/context` -> `400`
- `GET /api/explain-memory` -> `400`
- `GET /api/raw-source` -> `405`
- `GET /api/propose-memories` -> `405`
- `GET /api/rebuild-index` -> `405`
- `POST /api/rebuild-index` without local-action header -> `403`

## Recommended Next Work

1. Replace `GenericObject` response references in `docs/openapi.yaml` with
   endpoint-specific schemas for at least the top-level keys.
2. Add a contract test that snapshots top-level keys for every documented route.
3. Keep `docs/api.html`, `docs/openapi.yaml`, and the route inventory in
   `serve.py` synchronized from one source of truth.
