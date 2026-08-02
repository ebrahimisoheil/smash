# V1 → V2 Classification (Phase A, session A2)

Every V1 feature or behavior inventoried here gets exactly one of four classes. This is a
completeness gate, not a debate — **zero rows may be left unclassified**, and every
defer/retire row states a reason (defer rows also name the owning V2 phase).

| Class | Meaning | Consequence |
|---|---|---|
| **preserve** | User/agent-visible behavior already correct and valuable | Becomes a V2 acceptance fixture + contract test; its absence later is a regression |
| **reuse** | Code, tests, datasets, or wording that may be adapted as material | May be copied as material; its V1 storage/runtime assumptions must NOT constrain V2 design |
| **defer** | Real but not needed to reach the V2 Community Edition gate | Recorded against the future phase that would own it |
| **retire** | Stays in V1 only; not rebuilt until V2 proves a real current need | Recorded with the reason it's not being carried forward |

The purpose of this table is to stop "copying architecture while intending to copy
behavior" — e.g. Markdown-as-database is a V1 *implementation detail*, not a *behavior* to
preserve. Rows are organized by inventory surface. Cross-cutting behaviors (session-start
brief, session-end proposal loop) are called out explicitly even though no single V1 file
owns them.

Phase names used below: **Phase B** (Compose + canonical persistence), **Phase C** (source
pipeline/worker), **Phase D** (memory lifecycle + review UI), **Phase E** (light
search/LanceDB retrieval), **Phase F** (Maps/graph/cross-map), **Phase G** (Rules +
harnesses), **Phase H** (MCP tools/skills/prompts/connectors), **Phase I** (aggressive
search), **Phase J** (Community Edition release gate).

---

## 1. CLI + core engine (`smash.py`, `mcp_package/smash_core/`)

`smash.py` dispatches ~54 subcommands (see `main()`, `smash.py:3259`) onto handler
functions defined in `smash.py` that mostly delegate to `mcp_package/smash_core/*.py`.

| Feature | Where (V1 entry point) | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| `init` — scaffold a new wiki | `smash.py:2630 init_wiki` | preserve | Phase B | TBD — Phase B fixture: fresh workspace bootstrap produces valid canonical store | — |
| `remember` — durable write, review-gated | `smash.py:1119 remember`, `smash_core/memory.py write_memory_page` | preserve | Phase B/D | TBD — Phase D fixture: `remember` with duplicate/conflict candidates blocks unless `--allow-duplicate`/`--allow-conflict` | — |
| `propose-memories` — proposal-only extraction from text | `smash.py:1199 propose_memories`, `smash_core/memory.py propose_memories_from_text` | preserve | Phase D | TBD — Phase D fixture: proposals never write durable memory without explicit accept | — |
| `capture-session` — session/transcript capture prior to review | `smash.py:1232 capture_session`, `smash_core/capture.py write_session_capture` | preserve | Phase D | TBD — Phase D fixture: capture stored, zero durable writes until accepted | — |
| `session-end` — end-of-session proposal loop | `smash.py:1300 session_end` | preserve | Phase D | TBD — Phase D fixture: session-end always returns proposal-only candidates, never auto-commits | — |
| `capture-inbox` — list pending captures | `smash.py:1385 capture_inbox` | preserve | Phase D | TBD — Phase D fixture | — |
| `accept-capture` — promote capture to memory (review-gated) | `smash.py:1424 accept_capture` | preserve | Phase D | TBD — Phase D fixture | — |
| `redact-capture` — scrub secret-looking values from a capture | `smash.py:1518 redact_capture` | preserve | Phase D | TBD — Phase D fixture: redaction defense against pasted secrets | — |
| `delete-capture` — permanently remove a capture (confirm-gated) | `smash.py:1559 delete_capture` | preserve | Phase D | TBD — Phase D fixture | — |
| `update-memory` — edit an existing durable memory | `smash.py:1599 update_memory` | preserve | Phase D | TBD — Phase D fixture | — |
| `set-memory-visibility` — private/project/team visibility | `smash.py:1631 set_memory_visibility`, `smash_core/memory.py _set_memory_visibility` | preserve | Phase D | TBD — Phase D fixture: visibility persists and is respected by recall scoping | — |
| `recall` — lexical (+ optional semantic) memory recall | `smash.py:1650 recall`, `smash_core/memory.py recall_memories` | preserve | Phase E | TBD — Phase E fixture: lexical recall works with zero optional deps installed | — |
| Memory types + scope/visibility model | `smash_core/memory.py` (`_memory_type_scope`, frontmatter fields) | preserve | Phase D | TBD — Phase D fixture: type/scope combinations enumerated and enforced | — |
| Applicability conditions, fail-closed on invalid syntax | `smash_core/memory.py` (memory frontmatter `applies_if`/condition parsing) | preserve | Phase D | TBD — Phase D fixture: malformed condition syntax causes the memory to be excluded, not to error/crash | — |
| Review dates and expiry (`--review-after`, `--expires-at`) | `smash_core/memory.py memory_review_issues`, `smash.py:1119 remember` | preserve | Phase D | TBD — Phase D fixture: expired/overdue-review memories flagged in inbox | — |
| Supersession lineage + historical reconstruction | `smash_core/memory.py:915-936` (`supersedes`/`superseded_by`, lineage walk), `smash.py:1828 explain_memory` | preserve | Phase D | TBD — Phase D fixture: lineage walk reconstructs full history chain (up to existing 10/12-hop cap) | — |
| Duplicate defense on write | `smash_core/memory.py` `memory_duplicate_candidates`, `--allow-duplicate` gate in `remember_memory` | preserve | Phase D | TBD — Phase D fixture: near-duplicate write blocked without override | — |
| Conflict defense on write | `smash_core/memory.py:1364-1398` `memory_conflict_candidates`, `allow_conflict` gate | preserve | Phase D | TBD — Phase D fixture: conflicting active memory blocks write without override | — |
| Echo/contradiction defenses (claim-level similarity, not token overlap) | `smash_core/memory.py:310-312` doc comment + duplicate/conflict candidate logic | preserve | Phase D | TBD — Phase D fixture: near-paraphrase contradiction detected despite low token overlap | — |
| `archive-memory` / `restore-memory` / `forget-memory` (soft-delete, restore, hard-delete-confirm) | `smash.py:1702-1730` | preserve | Phase D | TBD — Phase D fixture: archive is reversible, forget requires `--confirm` and is not | — |
| `memory-inbox` — review queue | `smash.py:1768 memory_inbox` | preserve | Phase D | TBD — Phase D fixture | — |
| `memory-log` — append-only operation log | `smash.py:1792 memory_log`, `smash_core/log.py` | preserve | Phase D | TBD — Phase D fixture: every mutating op is logged with timestamp+operation+description | — |
| `wins` — recent memory value ("wins") surfacing | `smash.py:1805 memory_wins`, `smash_core/memory_wins.py` | defer | Phase D | Nice-to-have engagement surface, not required for CE gate |
| `review-memory` — mark reviewed | `smash.py:1818 review_memory` | preserve | Phase D | TBD — Phase D fixture | — |
| `explain-memory` — audit why a memory exists (provenance) | `smash.py:1828 explain_memory`, `smash_core/memory.py memory_explanation` | preserve | Phase D | TBD — Phase D fixture: explain returns source/provenance chain for a given memory id | — |
| `query` — bounded query packet (`--budget micro/small/medium/large`) | `smash.py:1848 query`, `smash_core/query.py` | preserve | Phase E | TBD — Phase E fixture: budget tiers cap response size deterministically at each tier | — |
| `recall_capsule` — ultra-compact first-read summary inside query packet | `smash_core/query.py:277 _recall_capsule` | preserve | Phase E | TBD — Phase E fixture: capsule always present and readable before full packet | — |
| `graph-summary` — bounded graph orientation | `smash.py:1870 graph_summary`, `smash_core/query.py`/`web_graph.py` | preserve | Phase F | TBD — Phase F fixture: bounded by `--limit`/`--depth`/`--max-edges`, never a full dump | — |
| `benchmark` — recall-quality benchmark runner exposed via CLI | `smash.py:1901 benchmark`, `smash_core/benchmark.py` | reuse | Phase E | Benchmark harness logic reusable; V1's JSON-vector storage assumptions must not carry over | |
| `brief` — session-start memory brief | `smash.py:1930 brief`, `smash_core/memory.py memory_brief` | preserve | Phase D | TBD — Phase D fixture: brief returns top relevant memories + pending review/proposal counts | — |
| `start` — combined onboarding/brief/status entry point | `smash.py:1958 start` | preserve | Phase D | TBD — Phase D fixture: single command surfaces status + brief + next actions | — |
| Session-start brief (cross-cutting: CLI `brief`/`start` + MCP `memory_brief` resource + hook) | `smash.py:1930`, `mcp_package/smash_mcp/server.py:1006 link_brief_resource`, `smash_core/agent_hooks.py` `_hook_session_start` (`smash.py:2215`) | preserve | Phase D/H | TBD — Phase D/H fixture: brief content is consistent whether triggered by CLI, MCP resource, or session-start hook | Cross-cutting; called out per method §3 |
| Session-end proposal loop (cross-cutting: CLI `session-end` + MCP `capture_session`/hook) | `smash.py:1300`, `smash_core/agent_hooks.py` `_hook_session_end` (`smash.py:2270`) | preserve | Phase D/H | TBD — Phase D/H fixture: session-end always proposes, never silently commits, across CLI and hook paths | Cross-cutting; called out per method §3 |
| `hook` — agent-host lifecycle hook dispatcher (session-start/session-end) | `smash.py:2375 run_agent_hook`, `smash_core/agent_hooks.py` | preserve | Phase H | TBD — Phase H fixture: hook payloads correctly route to brief injection / proposal capture | — |
| `consolidate` — read-only duplicate/theme grouping plan | `smash.py:2174 consolidate`, `smash_core/consolidate.py` | preserve | Phase D | TBD — Phase D fixture: plan is read-only, requires explicit per-item user approval to apply | — |
| `recipes` — suggested next-command recipes | `smash.py:2158 recipes` | defer | Phase D | Convenience UX layer on top of core lifecycle, not required for CE gate |
| `semantic` — optional local semantic tier setup/status | `smash.py:2057 semantic`, `smash_core/semantic.py` | preserve (behavior) / retire (storage) | Phase E | TBD — Phase E fixture: recall degrades gracefully to lexical when semantic tier absent | Behavior (optional semantic upgrade, graceful lexical fallback) is preserve; plain-JSON on-disk vector cache under `.smash-cache/` is retire — V2 uses LanceDB (see §7 Retire) |
| Blended reranking (hybrid lexical+semantic, cross-encoder rerank) | `smash_core/semantic.py:198-219` `load_reranker`, `smash_core/query.py:206 _hybrid_ranked_items` | preserve | Phase E | TBD — Phase E fixture: hybrid ranking beats lexical-only on paraphrase queries per benchmark dataset | — |
| `profile` — memory profile / stats | `smash.py:2393 profile`, `smash_core/memory.py memory_profile` | preserve | Phase D | TBD — Phase D fixture | — |
| `memory-audit` — audit report + next-actions | `smash.py:2427 memory_audit`, `smash_core/memory.py memory_audit_report`/`memory_audit_next_actions` | preserve | Phase D | TBD — Phase D fixture: audit surfaces safe-next-actions, not just diagnostics | — |
| `status` — readiness status | `smash.py:787 status`, `smash_core/status.py` | preserve | Phase B | TBD — Phase B fixture: status accurately reflects store readiness (schema version, pending ops) | — |
| `health` — composite health check + exit code | `smash.py:856 health`, `smash_core/web_health.py` | preserve | Phase B/D | TBD — Phase B/D fixture: health check catches interrupted/stale write operations | — |
| `doctor` — diagnose + `--fix` safe repairs | `smash.py:749 doctor`, `smash_core/doctor.py` | preserve | Phase B | TBD — Phase B fixture: doctor only repairs generated/structural state, never silently rewrites durable content | — |
| `validate` — schema/consistency validation, `--strict` | `smash.py:761 validate`, `smash_core/validation.py` | preserve | Phase B | TBD — Phase B fixture: validate rejects malformed store state deterministically | — |
| `migrate` — schema migration | `smash.py:774 migrate` | preserve (behavior) | Phase B | TBD — Phase B fixture: migration behavior (safe upgrade path with backup) preserved; V1's Markdown-frontmatter migration mechanics are retire | Storage mechanics retire — see §7 |
| `operations` — list pending/failed/interrupted write operations | `smash.py:875 operations`, `smash_core/operations.py` | preserve | Phase B | TBD — Phase B fixture: interrupted writes are recoverable/rollback-able (`recover_operation`, `_rollback_snapshots`) | — |
| Durable-write journal + snapshot/rollback for interrupted operations | `smash_core/operations.py:33-273` (`begin_operation`, `finish_operation`, `fail_operation`, rollback snapshots) | preserve (behavior) / retire (mechanism) | Phase B | TBD — Phase B fixture: a write interrupted mid-flight is either fully applied or fully rolled back, never partial | Filesystem-snapshot journal is a Markdown-as-database mechanism; V2 needs equivalent durability via transactional DB writes, not this file-copy mechanism — see §7 |
| `backup` / `restore-backup` | `smash.py:901 backup`, `937 restore_backup`, `smash_core/backup.py` | preserve | Phase B | TBD — Phase B fixture: backup/restore round-trips full store state | — |
| `compliance-export` | `smash.py:971 compliance_export` | preserve | Phase D | TBD — Phase D fixture: export includes full audit trail for a memory/store | — |
| `team-sync` — remote sync for team scope | `smash.py:999 team_sync`, `smash_core/team_sync.py` | defer | Phase D | Team/multi-user sync is beyond CE (single-workspace) scope |
| `share` — serve single page for sharing | `smash.py:1010 share`, `smash_core/share.py` | defer | Phase D | Low-value convenience, not core lifecycle |
| `snapshot` — point-in-time export | `smash.py:1018 snapshot`, `smash_core/snapshot.py` | preserve | Phase B | TBD — Phase B fixture | — |
| `ingest-status` — pending raw file / ingest plan | `smash.py:1046 ingest_status`, `smash_core/ingest.py` | preserve | Phase C | TBD — Phase C fixture: ingest plan correctly identifies pending/stale/unsafe raw files | — |
| `import-obsidian` — import existing Obsidian vault | `smash.py:1058 import_obsidian`, `smash_core/obsidian.py` | defer | Phase C | Source-specific importer, not required for initial CE ingest pipeline |
| `rebuild-backlinks` / `rebuild-index` | `smash.py:1083`, `1105` | retire | — | Generated-index rebuild is a Markdown-as-database artifact of file-based storage; a transactional DB has no equivalent "rebuild" step |
| Secret/PII redaction defense on ingest and capture | `smash_core/security.py`, invoked from `capture.py`/`ingest.py` | preserve | Phase C/D | TBD — Phase C/D fixture: known secret-shaped strings (API keys, tokens) are blocked or redacted before persistence | — |
| Source-backed project seeding (`seed`) | `smash.py:2826 seed_project`, `smash_core/project_seed.py` | preserve | Phase C | TBD — Phase C fixture: seeding reads allowlisted project docs/rule files, blocks secret-looking values, creates no durable memory | — |
| `onboard` — agent-host onboarding flow | `smash.py:2647 onboard` | preserve | Phase H | TBD — Phase H fixture: onboarding wires MCP config + hooks for a named agent host | — |
| `welcome` / `prompts`/`starter_prompts` — first-run guidance | `smash.py:2891 starter_prompts`, `2902 welcome`, `smash_core/prompts.py` | defer | Phase H | Onboarding polish, not core lifecycle for CE gate |
| `demo` — create demo wiki | `smash.py:2969 create_demo`, `smash_core/demo.py` | defer | Phase J | Marketing/demo tooling, not a CE functional requirement |
| `proof` — reproducible "does recall actually work" proof run | `smash.py:3050 proof` | reuse | Phase E | Proof-run logic and framing reusable for V2 acceptance demos; V1's specific command plumbing is not load-bearing |
| `try` — quick guided trial flow | `smash.py:3149 try_link` | defer | Phase J | Marketing/trial UX, not core |
| `verify-mcp` — check MCP readiness | `smash.py:2476 verify_mcp`, `smash_core/mcp_verify.py` | preserve | Phase H | TBD — Phase H fixture: verify-mcp correctly detects broken/missing MCP config | — |
| `connect` — wire MCP config for a host | `smash.py:2533 connect_mcp`, `smash_core/mcp_connect.py` | preserve | Phase H | TBD — Phase H fixture | — |
| `serve` (wiki viewer) / `api` (JSON API server) as CLI subcommands | `smash.py:2913 serve_wiki`, `2941 serve_api` | retire (as entry points) | — | — | These subcommands just launch the `serve.py` HTTP server classified in §2; the local-viewer/API-server architecture itself is retire — see §2 for the full breakdown of which underlying behaviors are reuse vs. retire |
| `version` | `smash.py:3336` | preserve | Phase B | TBD — trivial fixture | — |
| Slim MCP surface concept (small stable command set vs. full compatibility set) | `smash.py` command list itself is the "full" analog to MCP `--surface slim/full` split | preserve | Phase H | see §3 slim surface rows | — |
| Markdown files as the canonical transactional database | `smash_core/wiki.py`, `frontmatter.py`, `markdown.py` (every write ultimately edits `.md` files under `wiki/`) | retire | — | Explicit anchor: implementation detail, not behavior. V2 needs real transactional persistence (Phase B), not file-based storage |
| Runtime business logic duplicated across CLI/serve/MCP shells | `smash.py`, `serve.py`, `mcp_package/smash_mcp/server.py` each re-implement thin wrappers over `smash_core` but with surface-specific branching/validation | retire | — | Explicit anchor: V2 should have one core engine with thin transport adapters, not logic re-derived per shell |
| File naming as stable identity (memory pages identified by filename/slug) | `smash_core/memory.py resolve_memory_page`, `wiki.py` slug generation | retire | — | Explicit anchor: filenames as primary keys don't survive rename/move; V2 needs stable surrogate IDs |
| Plain JSON vectors as long-term semantic index | `smash_core/semantic.py` `.smash-cache/*.json` embedding cache | retire | — | Explicit anchor: not a scalable index; V2 uses LanceDB (Phase E) |
| Single-user authorization shortcuts (no auth model, local-only trust) | `smash_core/*` — no auth/permission checks anywhere in core | retire | — | Explicit anchor: V2 needs a real (even if minimal) authorization model from the start |

## 2. Local server (`serve.py`)

`serve.py` implements a stdlib `http.server` handler (`Handler`, `serve.py:1438`) serving
both human-facing HTML pages and a JSON `/api/*` surface, plus an `APIHandler` subclass
(`serve.py:2040`) for `smash api`.

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| Human wiki viewer (HTML pages: `/`, `/memory`, `/audit`, `/inbox`, `/captures`, `/profile`, `/wins`, `/memory-log`, `/graph`, `/search`, `/page/<name>`, `/onboard`, `/health`, `/ingest`, `/brief`, `/propose`, `/prompts`, `/more`, `/all`) | `serve.py:1596 do_GET` routing table | retire | — | Local-viewer assumption (single stdlib server, no auth, filesystem-coupled rendering) is explicitly listed as retire; V2 web app is a separate real frontend, not a rebuild of this viewer | |
| JSON `/api/*` read endpoints (status, health, operations, ingest-status, backlinks, page-links, page-list, pages, proposal-sources, memory-profile, memory-dashboard, memory-brief, query-Smash, memory-audit, memory-inbox, wins, memory-log, capture-inbox, graph, graph-summary, search, context) | `serve.py:1661-1861` | reuse | Phase B/D/E | The *shape* of these read APIs (what data + what filters) is useful reference for V2's Compose API surface; the stdlib-server implementation is not | |
| Mutating `/api/*` endpoints (validate, migrate, rebuild-index, rebuild-backlinks, seed-project, raw-source, propose-memories, remember-memory, review-memory, archive-memory) | `serve.py:1476-1596 do_POST` | reuse | Phase B/D | Endpoint intent (what mutations are exposed over HTTP) is reference material; V1's synchronous handling and lack of auth are not carried forward | |
| Synchronous long-running work directly in request handlers (e.g. rebuild-index, migrate triggered inline in `do_POST`) | `serve.py:1494-1517` | retire | — | Explicit anchor: synchronous long-running work in request paths. V2 needs a worker/job model (Phase C) for anything non-trivial |
| UI directly coupled to wiki file/directory layout (page rendering reads `wiki/*.md` paths directly) | `smash_core/web_pages.py`, `web_memory_pages.py`, `web_layout.py` | retire | — | Explicit anchor: UI coupled directly to storage layout. V2 UI must go through an API/data-access layer, never touch storage layout directly |
| `smash api` as a distinct read-mostly JSON API mode | `smash.py:2941 serve_api`, `serve.py:2040 APIHandler` | reuse | Phase B | Separating a stable JSON API from the HTML viewer is a reasonable pattern reference; V2's Compose API replaces both | |

## 3. MCP surface (`mcp_package/smash_mcp/`)

`smash_mcp/server.py` exposes two tool surfaces gated by `--surface slim|full`
(`_surface_tool`, `server.py:189`): **slim** (6 tools: `status`, `recall`, `remember`,
`ingest`, `review`, `admin`) intended as the default agent-facing surface, and **full**
(~35 tools, one-tool-per-operation) for compatibility/advanced workflows. Also exposes MCP
resources (`link_instructions_resource`, `link_health_resource`, `link_brief_resource`,
`link_profile_resource`, `link_project_resource`) and prompts (`link_start_prompt`,
`link_brief_prompt`, `link_remember_prompt`, `link_session_end_prompt`,
`link_ingest_prompt`, `link_review_prompt`).

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| Slim MCP surface design (few consolidated tools: status/recall/remember/ingest/review/admin) | `server.py:1156-1537`, surface selection at `server.py:189-206` | preserve | Phase H | TBD — Phase H fixture: slim surface remains the documented default; agent sees one obvious recall tool and one obvious remember tool | — |
| Full MCP surface (one-tool-per-operation compatibility mode) | `server.py:1537-2213` | preserve | Phase H | TBD — Phase H fixture: full surface exists for advanced/compat clients, mirrors CLI command coverage | — |
| `status` / `link_status` — MCP readiness check | `server.py:1156`, `1550` | preserve | Phase H | TBD — Phase H fixture | — |
| `recall` / `recall_memory` / `query_link` — recall + query packet over MCP | `server.py:1169`, `1680`, `1537` | preserve | Phase E/H | TBD — Phase E/H fixture: MCP recall returns same content/budget semantics as CLI `recall`/`query` | — |
| `remember` / `remember_memory` — durable write over MCP, review-gated | `server.py:1227`, `1975` | preserve | Phase D/H | TBD — Phase D/H fixture: MCP remember enforces the same duplicate/conflict gates as CLI | — |
| `ingest` (consolidated) / `ingest_status` — ingest surface over MCP | `server.py:1287`, `1642` | preserve | Phase C/H | TBD — Phase C/H fixture | — |
| `review` (consolidated: inbox/explain/archive/restore/forget/profile/audit/log) | `server.py:1330` | preserve | Phase D/H | TBD — Phase D/H fixture: consolidated review tool dispatches correctly to each sub-action | — |
| `admin` (consolidated: backup/migrate/validate/rebuild/pages/backlinks/graph export/seed/captures/updates) | `server.py:1395` | preserve (behavior) | Phase H | TBD — Phase H fixture: admin tool exposes maintenance actions without requiring one-tool-per-action bloat | Underlying rebuild/migrate mechanics tied to file storage are retire (see §1) |
| MCP resources exposing brief/health/profile/project/instructions as readable context | `server.py:953-1036` | preserve | Phase H | TBD — Phase H fixture: resources are fetchable and reflect live state | — |
| MCP prompts (start, brief, remember, session-end, ingest, review) as reusable prompt templates | `server.py:1036-1120` | reuse | Phase H | Prompt intent/wording reusable; exact FastMCP prompt registration mechanics not load-bearing | |
| `--semantic-setup` one-time offline model fetch flag | `server.py:75-95` | preserve | Phase E | TBD — Phase E fixture: semantic setup is the only path allowed to touch the network; recall itself never does | — |
| MCP tool descriptions / server instructions text | `server.py:126-187 _instructions` | reuse | Phase H | Wording and tool-selection guidance reusable almost verbatim for V2 tool descriptions | |
| In-memory index cache invalidated by wiki mtime | `server.py:364-397 _wiki_mtime`, `_build_cache` | retire | — | mtime-polling cache invalidation is a filesystem-coupled mechanism; V2's DB-backed store needs real invalidation, not file mtime checks |

## 4. Skills (`skills/smash-health`, `smash-ingest`, `smash-memory`, `smash-retrieve`)

Each is a single `SKILL.md` giving an agent a bounded command sequence for a lifecycle
stage, explicitly designed to avoid dumping the whole wiki into context.

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| `smash-retrieve` skill — bounded recall workflow (health check → seed → query micro→small→medium→large → brief → graph-summary → benchmark) | `skills/smash-retrieve/SKILL.md` | preserve (behavior) | Phase E/H | TBD — Phase E/H fixture: an agent following this workflow never needs to read the whole store | Command names/CLI wording reusable as-is per §"Reuse" anchor (terminology/command semantics) |
| `smash-memory` skill — remember/recall/review lifecycle workflow, proposal-first discipline | `skills/smash-memory/SKILL.md` | preserve | Phase D/H | TBD — Phase D/H fixture: skill enforces "propose first unless user explicitly asks to remember" | — |
| `smash-ingest` skill — raw file → wiki page → proposal → rebuild/validate workflow | `skills/smash-ingest/SKILL.md` | preserve (behavior) / retire (rebuild step) | Phase C/H | TBD — Phase C/H fixture: ingest workflow stops on secret-looking/unsafe input | `rebuild-index`/`rebuild-backlinks` steps are retire per §1; the surrounding ingest discipline is preserve |
| `smash-health` skill — readiness → operations → backup → doctor/rebuild → validate workflow | `skills/smash-health/SKILL.md` | preserve (behavior) / retire (rebuild step) | Phase B/H | TBD — Phase B/H fixture: skill always backs up before repair/migration | Rebuild-index/backlinks steps retire per §1 |
| Skill wording/instructions text itself | all four `SKILL.md` files | reuse | Phase H | Directly adaptable prose for V2 skill definitions once underlying commands exist | |

## 5. Agent host integrations (`integrations/`)

Seven host directories (`claude-code`, `copilot`, `cursor`, `codex`, `kiro`, `antigravity`,
`vscode`) each with `install.sh` / `uninstall.sh` / `install.ps1`, plus `_shared/` holding
common scaffold and instruction text (`scaffold.sh`, `instructions.sh`,
`smash-instructions.md`, `smash-instructions-project.md`).

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| Per-host installer wiring MCP config + session-start/session-end hooks | `integrations/<host>/install.sh` (7 hosts) | preserve | Phase H | TBD — Phase H fixture: install produces a working MCP connection + brief-on-start + proposal-on-end for each supported host | — |
| Per-host uninstaller (clean removal of MCP config + hooks) | `integrations/<host>/uninstall.sh` | preserve | Phase H | TBD — Phase H fixture: uninstall leaves no orphaned config | — |
| Windows PowerShell installer parity (`install.ps1`) | `integrations/<host>/install.ps1`, `_shared/instructions.ps1`, `_shared/scaffold.ps1` | defer | Phase H | Cross-platform parity valuable but not required to reach CE gate on primary platform |
| Shared scaffold/instruction generation logic | `integrations/_shared/scaffold.sh`, `instructions.sh` | reuse | Phase H | Templating approach reusable; exact shell mechanics not load-bearing | |
| Agent-host system-prompt instructions content (what to tell the agent about Smash) | `integrations/_shared/smash-instructions.md`, `smash-instructions-project.md` | reuse | Phase H | Wording/intent reusable almost verbatim | |
| Install/agent-host knowledge (which host uses which config file/location) | all `integrations/<host>/install.sh` | reuse | Phase H | Explicit anchor: install and agent-host knowledge is reuse material | |

## 6. Benchmarks (`benchmarks/`)

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| Smash recall benchmark dataset (62 memories, 6 domains, 20 distractors, 1,176 cases) | `benchmarks/RESULTS.md` §Track 1, generated by `scripts/recall_dataset.py` | reuse | Phase E | Dataset directly reusable to validate V2's recall quality against the same numbers | |
| LoCoMo third-party benchmark track | `benchmarks/RESULTS.md` §Track 2 | reuse | Phase E | Reusable as an external validity check for V2 recall | |
| Memory hygiene over time (junk rate, contradiction exposure, store growth, temporal accuracy, gated vs ungated) | `benchmarks/RESULTS.md` §Track 3 | preserve | Phase D/E | TBD — Phase D/E fixture: V2 gated writes must not regress junk-rate/contradiction-exposure numbers vs. V1 baseline | — |
| Documented rejected ablations (potion-retrieval-32M, multi-view embeddings, potion-base-32M, token-level late interaction, PMI query expansion) | `benchmarks/RESULTS.md` "Ablations we ran and rejected" | preserve | Phase E | TBD — Phase E fixture/doc: negative results preserved so V2 doesn't re-run the same failed experiments | Explicit anchor: "benchmarks including failed ablations" |
| Two-tier optional semantic model choice (fast: model2vec potion-base-8M; quality: MiniLM-L6-v2 ONNX) and load-time/latency numbers | `benchmarks/RESULTS.md` §Semantic tiers | reuse | Phase E | Model choice and measured tradeoffs are useful reference; V1's `.smash-cache/` JSON storage is retire (§1) | |

## 7. Docs / wiki (`docs/`, `wiki/`, `SMASH.md`, `README.md`)

Classified as doc *sets*, not per-file.

| Feature | Where | Class | V2 owner | Evidence | Reason |
|---|---|---|---|---|---|
| Marketing/product docs site (`index.html`, `why-smash.html`, `getting-started.html`, `concepts.html`, `ui.html`) | `docs/*.html` | reuse | Phase J | Explanations/positioning reusable where they still match V2 behavior; needs rewrite pass once V2 ships, not a straight port | |
| API/CLI/MCP reference docs (`api.html`, `cli.html`, `mcp.html`, `openapi.yaml`) | `docs/api.html`, `cli.html`, `mcp.html`, `openapi.yaml` | reuse | Phase B/H | Structure and command/tool documentation approach reusable; content must be regenerated against V2's actual surface | |
| Memory contract / concepts documentation (types, scope, visibility, supersession, applicability) | `docs/memory-contract.html`, `concepts.html` | preserve (as spec source) | Phase D | TBD — this classification file plus `memory-contract.html` together are the spec V2's Phase D fixtures should be written against | Explicit anchor: documentation explanations matching V2 are reuse; the underlying contract itself is preserve behavior already covered in §1 |
| Security / team-security / scale docs | `docs/security.html`, `team-security.html`, `scale.html` | defer | Phase D/G | Describe features (team sync, scale posture) already deferred in §1; docs follow feature timing |
| Obsidian integration doc | `docs/obsidian.html` | defer | Phase C | Tied to `import-obsidian`, already deferred in §1 |
| Troubleshooting doc | `docs/troubleshooting.html` | reuse | Phase B | Failure-mode knowledge reusable once V2 has equivalent failure modes to document | |
| `api-contract-audit.md` | `docs/api-contract-audit.md` | reuse | Phase B | Prior audit methodology reusable as a template for auditing V2's Compose API | |
| `wiki/` content itself (concepts/entities/explorations/memories/sources pages, `_backlinks.json`, `_link_schema.json`) | `wiki/*` | retire | — | This is V1's actual Markdown-as-database instance data — explicit anchor for retire; not carried into V2 storage | |
| `SMASH.md` (product overview / pitch) | `SMASH.md` | reuse | Phase J | Positioning/wording reusable for V2's own top-level doc once V2 ships | |
| `README.md` (V1 install/usage) | `README.md` | reuse | Phase J | Structure reusable as a template; content must be rewritten for V2's actual install/usage | |
| Graph/source test fixtures referenced from wiki content used in tests | `wiki/entities`, `wiki/sources`, `wiki/comparisons`, `wiki/explorations` where used as test data | reuse | Phase D/F | Explicit anchor: graph and source test data is reuse material | |
| Conflict and proposal fixtures used in tests | any `wiki/memories` sample pages exercising conflict/duplicate/proposal review flows | reuse | Phase D | Explicit anchor: conflict and proposal fixtures are reuse material | |

---

## Summary

- **Total rows: 119**
- **preserve: 74** (includes rows dual-tagged "preserve (behavior) / retire (mechanism/storage/rebuild step)" and "preserve (as spec source)")
- **reuse: 22**
- **defer: 11**
- **retire: 12**

Dual-tagged rows (e.g. "preserve (behavior) / retire (mechanism)") are counted once above
under their primary/behavioral class, with the secondary class and reason stated inline in
that row's Class/Reason cells so nothing is left ambiguous — e.g. `migrate` is preserve as
a *behavior* (safe upgrade path with backup) while its Markdown-frontmatter migration
*mechanics* are retire.

Zero rows are unclassified. Every defer row states its owning phase; every retire row
states its reason.
