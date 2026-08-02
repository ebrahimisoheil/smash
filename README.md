<p align="center">
  <img src="logo.svg" alt="Smash" width="128">
</p>

<h1 align="center">Smash</h1>

<h2 align="center">Local memory for AI agents.</h2>

<p align="center">
  Smash gives Codex, Claude, Cursor, Kiro, VS Code, Copilot, Antigravity, and
  other local agents the same source-backed memory, stored locally as Markdown.
</p>

## What Is Smash?

Smash is an open-source memory layer for local AI agents. Raw sources become an
inspectable Markdown wiki. Explicit "remember this" requests become reviewable
memories. Agents retrieve compact, source-backed context through the CLI, MCP,
official skills, or the local viewer without dumping the whole wiki into a chat
window.

The wiki is the storage layer. The product is durable memory that stays on your
machine, remains readable in plain files, and can be shared across multiple
agents instead of locked inside one vendor profile.

<p align="center">
  <img src="docs/assets/smash-aha.gif" alt="smash recall finds a memory saved in completely different words — matched by meaning, not keywords" width="760">
</p>
<p align="center"><em>Ask in your own words; Smash matches by meaning, not keywords. All local, all plain files.</em></p>

## How It Works

Smash gives agents four simple moves:

1. **Capture** notes, transcripts, docs, screenshots, and project context in `raw/`.
2. **Structure** source-backed pages under `wiki/`.
3. **Remember** explicit preferences, decisions, facts, and project context as reviewable memory.
4. **Retrieve** compact query packets through the CLI, MCP, official skills, or the local web viewer.

Most agent sessions start from zero. You re-explain preferences, repo decisions,
project constraints, and why something matters. Smash turns that repeated context
into local memory agents can query.

| Pain                                | Smash's answer                                                            |
| ----------------------------------- | ------------------------------------------------------------------------- |
| Agents forget you between sessions. | Save reviewed preferences, decisions, facts, and project context.         |
| Notes are private or messy.         | Keep raw sources local, then turn them into source-backed Markdown.       |
| Context windows are expensive.      | Return compact query packets with provenance and follow-up actions.       |
| Memory needs trust.                 | Every page and memory can be inspected, reviewed, archived, or forgotten. |

Smash follows Andrej Karpathy's
[LLM Wiki pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f):
keep knowledge outside the chat window, make claims inspectable, and let context
compound over time.

## Why Smash Is Different

Every other agent-memory system stores memory as embeddings in a vector
database or as an LLM-extracted graph. Smash made four architectural
commitments those designs cannot bolt on:

1. **Memory you can read.** Every memory is a plain Markdown file — open it,
   grep it, git-diff it. If Smash disappeared tomorrow, your memory is still yours.
2. **Review-gated writes.** Agents propose; you approve. Even the automatic
   session hooks capture proposals, never facts.
3. **No LLM in the memory layer.** Ingestion and recall are deterministic —
   nothing can hallucinate a fact into your memory, because there is no model
   in the write path.
4. **Provably local.** CI blocks outbound network code in the runtime, and the
   optional semantic models load offline-only after one explicit setup.

And the claims are measured, not asserted — see the benchmarks below. Named
comparisons against Mem0/OpenMemory, Zep/Graphiti, and Letta:
[Why Smash?](docs/why-smash.html)

## Benchmarks

Plain files with no LLM in the memory layer, measured against the systems
that have one everywhere:

| What                                                                                                                         | Smash                                 | For comparison                                                                                                                                                                                                                   |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LoCoMo end-to-end QA** — full 1,540 questions under [mem0's own open harness](https://github.com/mem0ai/memory-benchmarks) | **84.8%**                             | mem0's cloud platform: **83.2%** under the same judge — with GPT-5 writing their answers and a budget model (claude-haiku-4-5) writing Smash's. Confirmed by a second, independent judge (Tencent Hunyuan 3): **85.5% vs 83.6%** |
| **LongMemEval evidence retrieval** — did the memory layer put the gold evidence in context? (deterministic, no LLM judge)    | **99.4%** of 500 questions            | of 102 answer failures, only 3 were retrieval misses — the rest happened with the evidence already retrieved                                                                                                                     |
| **Memory hygiene** — junk stored over a simulated multi-month session stream                                                 | **0%** (by construction, CI-enforced) | the same pipeline with governance off: 23.9%                                                                                                                                                                                     |
| **Bundled 1,176-case recall benchmark** — deterministic, runs offline in CI                                                  | hit@1 **0.749**, +rerank **0.839**    | gates every change; a regression fails the build                                                                                                                                                                                 |

Every number ships with its config, judge model, caveats, and the
experiments that _lost_ — including LongMemEval end-to-end, where we
re-judged both sides under the neutral Hunyuan 3 referee: mem0's GPT-5
answers score 91.0%, Smash's budget-model answers 80.6%. Their published
number holds up, and the gap tracks the answering model, not the memory
layer — that's what the 99.4% evidence-retrieval row above isolates.
Full methodology and reproduction steps:
[benchmarks/RESULTS.md](benchmarks/RESULTS.md).

## Quick Start

Two commands: see it work, then make it yours.

```bash
brew install /smash/Smash
smash proof                              # see the promise (~1 second, no setup)
smash onboard --agent claude-code --write   # wire your agent for real memory
```

`smash proof` creates a throwaway workspace, writes one reviewed memory, and
recalls it through the same path the CLI, skills, and MCP use — the core
promise (one local memory, reusable by different agents, no cloud profile) in
one second:

```text
Cross-agent memory continuity works
Memory: created and reviewed: Cross-agent Smash proof
Recall: found through the same bounded recall path used by CLI, skills, and MCP.
Result: proof passed
```

`smash onboard --agent claude-code --write` then creates `~/Smash`, provisions the
MCP runtime, and wires the agent — including the session hooks that capture
memory automatically as you work (swap `claude-code` for `codex`, `cursor`,
`kiro`, `copilot`, `antigravity`, or others). Drop `--write` to preview the
config without touching anything, or drop `--agent` to just create the
workspace.

The installed command is `smash` because `Smash` is already a POSIX/macOS system
utility. From a source checkout, use `python3 smash.py ...` instead.

Want the UI, graph, and source pages first? `smash try && smash serve smash-demo`.
Windows, source checkout, and skill-first paths are in the
[First 10 Minutes guide](docs/getting-started.html).

Or seed your current repo as a separate step so the first real recall is not empty:

```bash
cd /path/to/your/project
smash seed . ~/Smash
smash query "what is this project about?" ~/Smash --budget small
```

`smash seed` reads allowlisted project files such as `README.md`, `AGENTS.md`,
`CLAUDE.md`, `.cursorrules`, and editor rule files, blocks secret-looking
values, writes a source-backed project page, and rebuilds the graph. It does
not create durable memories; agents should still use reviewed memory proposals
for preferences and decisions.

The Homebrew formula is maintained in the public
[`/homebrew-Smash`](packaging/homebrew) tap.

Open:

```text
http://127.0.0.1:3000
http://127.0.0.1:3000/onboard
http://127.0.0.1:3000/graph
http://127.0.0.1:3000/health
```

Use `/onboard` when you want the same first-run checklist in the local UI:
readiness, project context seeding, first memory, agent wiring, and starter
prompts. The web viewer is for local use only. It binds to `127.0.0.1`, has no user
accounts or authentication, and should not be exposed to the internet unless you
add your own auth layer.

Try the value loop:

```bash
smash start smash-demo --task "working on agent memory"
smash query "why does Smash help agents?" smash-demo --budget small
smash brief "working on agent memory" smash-demo
smash benchmark "agent memory" smash-demo
smash health smash-demo
```

`smash benchmark` reports both performance and value evidence: cache/search/query
timings, graph payload shape, and an estimate of how much broad wiki context the
bounded Smash packet avoided sending to an agent.

The `/health` page mirrors the readiness loop in the browser: validation state,
interrupted writes, memory review status, and copyable repair commands. The
viewer stays document-first — common paths in the top nav, deeper tools under
`more`, and a contents outline plus graph-related links on structured pages.

The generated demo is the public proof wiki. Generated content inside `wiki/`,
`raw/`, and `smash-demo/` is ignored by git so personal memory is not published
by accident.

## SmashBar — the menu bar app (macOS)

Smash's memory, ambient. SmashBar puts the review gate in your menu bar: a
global palette (⌥⌘M) to recall or remember from any app, native
notifications with one-tap Accept when a session capture lands, a live
pulse while agents are writing, and a browser over every memory file —
all running on the same reviewed `smash` commands as the CLI.

<p align="center">
  <img src="docs/assets/smashbar-tour.gif" alt="SmashBar cycling through its tabs: review inbox with live agent pulse and capture previews, memory browser, status dashboard, and settings" width="424">
</p>

```bash
brew install --cask /smash/smashbar
```

Unsigned on purpose (no Apple fee inflating anything): the cask strips
the quarantine flag on install, so it opens like any app. Building from
source instead: `cd apps/SmashBar && bash Scripts/bundle.sh --install`.

## Killer Demo: One Memory, Two Agents

This is the moment Smash is built for:

1. In one agent, say:

   ```text
   remember that I prefer local, source-backed memory for AI agents
   ```

2. In another agent connected to the same `~/Smash` workspace, say:

   ```text
   start with Smash before we continue
   what does Smash remember about local agent memory?
   ```

3. The second agent should recall the reviewed memory from local Markdown
   instead of asking you to repeat yourself.

For a clean automated version of the same idea, run:

```bash
smash proof
```

## Ways To Use Smash

Pick the surface that matches how you work. They all read and write the same
local Markdown wiki.

These surfaces are independent. `smash serve` / `serve.py` is only the local web
viewer. CLI commands, official skills, and MCP tools read the same `wiki/` files
directly, so Claude, Codex, Kiro, Cursor, or another agent can use Smash even
when the web viewer is not running.

<table>
  <tr>
    <td width="33%">
      <strong><a href="docs/ui.html">Web UI</a></strong><br>
      Read the local wiki, then review memory, ingest, graph, audits, captures, and explanations.
    </td>
    <td width="33%">
      <strong><a href="docs/cli.html">CLI</a></strong><br>
      Script readiness, query packets, briefs, validation, backup, context-savings benchmark, and repair.
    </td>
    <td width="33%">
      <strong><a href="docs/cli.html">MCP</a></strong><br>
      Let Codex, Claude, Cursor, Kiro, VS Code, Copilot, and other agents recall memory.
    </td>
  </tr>
</table>

<p align="center">
  <img src="docs/assets/smash-ui-tour.gif" alt="Smash local console tour: home, memory dashboard, health, and graph" width="840">
</p>
<p align="center"><em>The local web viewer: browse source-backed memory and explore the knowledge graph — all on <code>127.0.0.1</code>, no accounts, no backend.</em></p>

Prefer skills instead of MCP? Smash ships small, lazy-loadable CLI skills under
`skills/`. They let an agent use `smash health`, `smash query`, `smash ingest-status`,
`smash session-end`, and `smash remember` directly, without MCP setup or a running
web viewer.

```text
skills/smash-health/SKILL.md
skills/smash-retrieve/SKILL.md
skills/smash-ingest/SKILL.md
skills/smash-memory/SKILL.md
```

Full guide: [Smash Skills](docs/skills.html).

## Install For Your Agent

Run one installer from the cloned checkout:

```bash
bash integrations/codex/install.sh
bash integrations/kiro/install.sh
bash integrations/claude-code/install.sh
bash integrations/cursor/install.sh
bash integrations/copilot/install.sh
bash integrations/vscode/install.sh
bash integrations/antigravity/install.sh
```

Installers create or update `~/Smash`, install or upgrade `smash-mcp`, write
lightweight agent instructions, and preserve existing wiki data on reinstall.
Use `--project` when a repo needs separate project memory.

On Windows, use the matching PowerShell installer:

```powershell
.\integrations\codex\install.ps1
.\integrations\kiro\install.ps1
.\integrations\claude-code\install.ps1
.\integrations\cursor\install.ps1
.\integrations\copilot\install.ps1
.\integrations\vscode\install.ps1
.\integrations\antigravity\install.ps1
```

Then ask your agent:

```text
is Smash ready?
start with Smash before we continue
seed this project into Smash
ingest raw/notes.md into Smash
remember that I prefer short release notes
query Smash for the release process
what does Smash remember about local personal memory?
end this session with Smash memory proposals
```

For CLI-first agents or Smash skills, use the same startup loop directly:

```bash
smash seed . ~/Smash
smash start ~/Smash --task "working on Smash release"
smash session-end session-notes.md ~/Smash --limit 3
```

If you want one guided setup for a real workspace and an agent, use
`smash onboard --agent AGENT`. If your agent already has instructions and you only
need MCP wiring, use the lower-level connection helper. Both preview the exact
config first; add `--write` when you want Smash to update the agent config file.

```bash
smash onboard --agent codex
smash onboard --agent codex --write
smash connect codex ~/Smash
smash connect codex ~/Smash --write
smash connect kiro ~/Smash --write
smash verify-mcp ~/Smash
```

For agents with session-hook support — Claude Code, Codex, and Cursor — add
`--hooks` (works with `smash onboard` too) to make the memory loop automatic:
the brief is injected at session start and proposal-only notes are captured at
session end, so memory no longer depends on the agent remembering to call
Smash. Empty sessions and duplicate end events are skipped, and when the
backlog builds up the brief nudges the agent to offer a read-only
`smash consolidate` pass. Durable memory still requires your approval. Codex and
Cursor hook support is new (wired to their documented schemas — report
issues).

```bash
smash connect claude-code ~/Smash --hooks --write
smash connect codex ~/Smash --hooks --write    # session-start brief (Codex has no session-end event)
smash connect cursor ~/Smash --hooks --write
smash consolidate ~/Smash                      # read-only backlog plan, apply only with approval
```

### Optional: hybrid semantic recall (still fully local)

Lexical recall is always the default and the fallback. Paraphrase matching is
opt-in: after the two setup commands below, "how should I structure my pull
requests" finds a memory saved about commit style. Until then, recall matches
on shared words, and a miss tells you how to turn paraphrase matching on.
Installing the optional semantic extra adds a small local static-embedding
model. Recall never touches the
network: the model loads offline-only after a one-time explicit setup,
embeddings live in plain JSON under `.smash-cache/`, similarity runs in-process
with no vector database, and semantic-only matches carry capped confidence
labels so agents verify before trusting them.

```bash
pip install "smash-mcp[semantic]"          # fast tier: tiny static model, instant load
pip install "smash-mcp[semantic-quality]"  # quality tier: contextual model, best recall
smash semantic ~/Smash --setup   # one-time model fetch, with your approval
smash semantic ~/Smash           # status: lexical only vs hybrid, active tier
python3 -m smash_mcp --semantic-setup --wiki ~/Smash/wiki   # MCP-only installs
```

Measured, not asserted: on the bundled 1,176-case benchmark, the quality
tier lifts token-overlap hit@1 from 0.589 to 0.749 and pure-paraphrase
(zero token overlap) hit@3/hit@5 by ~4×, at ~10 ms per recall with no
service or vector database. On the third-party LoCoMo retrieval track
(1,536 evidence-annotated questions over 5,882 conversation turns), hybrid
recall lifts any-evidence hit@10 from 0.628 to 0.737 (0.794 with the
opt-in rerank tier). Full methodology, honest limitations, and
reproduction steps: [benchmarks/RESULTS.md](benchmarks/RESULTS.md).

<details>
<summary>MCP-only install</summary>

```bash
python3 -m pip install --upgrade smash-mcp
python3 -m smash_mcp --version
```

```json
{
  "mcpServers": {
    "Smash": {
      "command": "python3",
      "args": ["-m", "smash_mcp", "--wiki", "~/Smash/wiki", "--surface", "slim"]
    }
  }
}
```

`--surface slim` is the recommended MCP surface for agents: six obvious tools
for recall, remember, ingest, review, status, and admin escape hatches. The full
compatibility surface is still available with `--surface full`.

On macOS/Homebrew Python, if pip reports `externally-managed-environment`, use a
dedicated venv:

```bash
python3 -m venv ~/.smash-mcp-venv
~/.smash-mcp-venv/bin/python -m pip install --upgrade pip smash-mcp
```

Full setup: [CLI reference](docs/cli.html).

</details>

Obsidian users can import an existing vault into `raw/` for agent ingest, or
open `~/Smash/wiki` directly as a vault for editing Smash pages:

```bash
smash init ~/Smash
smash import-obsidian ~/Documents/ObsidianVault ~/Smash
```

See the [Obsidian guide](docs/obsidian.html) for
the import, edit, and validation loop.

## Storage Model

Under the hood, Smash separates source-backed knowledge from durable agent memory:

1. Drop raw notes, transcripts, articles, and project context into `raw/`.
2. Agents compile those sources into inspectable pages under `wiki/`.
3. Explicit "remember" requests become reviewable memory pages.
4. Queries retrieve compact agent context from both the wiki and memory layer.

<p align="center">
  <img src="docs/assets/smash-memory-flow.svg" alt="Smash architecture: raw sources become wiki knowledge, explicit remembers become reviewed memory, and agents retrieve compact context" width="820">
</p>

The storage model is plain and inspectable:

| Layer            | What lives there                                                                         |
| ---------------- | ---------------------------------------------------------------------------------------- |
| `raw/`           | Original notes, transcripts, articles, PDFs, screenshots, and project files.             |
| `wiki/`          | Source-backed pages, concepts, entities, explorations, comparisons, and memories.        |
| Agent interfaces | CLI, skills, MCP, and local viewer paths that avoid dumping the whole wiki into context. |

If a raw file was already ingested and later edited, `smash ingest-status` marks it
as stale and tells your agent to refresh the existing source page instead of
creating a duplicate.

## What Agents Get

When an agent uses Smash through the recommended MCP surface, it gets six
model-facing tools. CLI and skill workflows call the same core behavior through
`smash`.

- `status`: readiness, schema state, validation, interrupted writes, and safe
  next actions.
- `recall`: the one read path for startup briefs, answer-ready query packets,
  wiki search, graph context, token budgets, and follow-up actions. Every
  recalled memory carries a `confidence` label (`strong`, `moderate`, `weak`)
  and a `match` field (`lexical`, `semantic`, `hybrid` when the optional local
  semantic tier is installed), so agents verify weak or paraphrase matches with
  the user instead of trusting them.
- `remember`: durable local memory only after explicit user approval, with
  duplicate/conflict checks, provenance, review state, visibility, optional
  `review_after`, and optional `expires_at`.
- `ingest`: exact next steps for raw files, source safety, stale ingest
  detection, validation, and rebuild checks.
- `review`: memory inbox, profile, audit, log, explain, archive, restore,
  forget, and lifecycle review workflows — plus `review(action="consolidate")`,
  a read-only backlog plan applied only with per-action user approval.
- `admin`: the escape hatch for backup, migrate, validate, graph export, pages,
  captures, rebuilds, compatibility actions, and advanced updates.

The stable agent-facing loop is documented at
[Smash Memory Contract](docs/memory-contract.html):
readiness first, bounded recall, explicit memory writes, audit tools, and
sharing semantics.

Use `review_after` for time-sensitive preferences or decisions. When that date
arrives, the memory reappears in Smash's review inbox so an agent can ask the
user to confirm, update, archive, or forget it instead of trusting stale context.
Use `expires_at` for temporary context that should automatically leave default
recall after a date; Smash keeps the Markdown page inspectable and asks the user
to update, archive, or delete it.
Use `visibility` to separate where a memory applies from who should see it:
`private` stays personal, `project` is intended for a project workspace, and
`team` means the user explicitly approved sharing it with a team.

For team handoff or security review, `smash compliance-export --output audit.json`
writes a redacted JSON packet with readiness, validation, memory review status,
operation markers, and recent audit log entries. Raw source contents and memory
bodies are not included.

For day-to-day auditability, `smash memory-log ~/Smash` shows what Smash recently
remembered, updated, reviewed, archived, restored, forgot, or accepted from raw
captures.

For recovery, `smash backup ~/Smash` creates a local archive and `smash
restore-backup <archive> ~/Smash` previews what would be restored. Passing
`--confirm` replaces local files after creating a safety backup when possible;
`raw/` is still excluded unless `--include-raw` is explicit. If a multi-file
write is interrupted, `smash operations ~/Smash` shows the marker and any rollback
snapshot; `smash operations ~/Smash --recover <marker> --confirm` restores the
snapshot after you review it.

For local proof of value, `smash wins ~/Smash` shows reusable memories, reviewed
memory, provenance, project continuity, freshness guardrails, and copyable
prompts without tracking user behavior.

For Git-backed team memory, `smash team-sync ~/Smash` checks whether the workspace
is ready to share reviewed `wiki/` pages while keeping `raw/`, caches, backups,
local MCP Python markers, and `wiki/log.md` private by default. The audit log is
local because it has a single-machine hash chain; merging multiple users' logs
would create false tamper alarms. Team sync also blocks "ready" status when the
memory inbox is not clear or active `visibility: private` memories would be
included by a broad `git add wiki`.

```bash
smash team-sync ~/Smash --remote git@example.com:team/smash-memory.git
```

For a teammate, reviewer, or another agent, `smash share` resolves a page,
memory, title, alias, or search phrase into a local viewer URL:

```bash
smash share "Prefer local memory" ~/Smash
```

For a static, read-only review packet, `smash snapshot` exports rendered wiki
HTML without `raw/`, captures, operation markers, live MCP state, or memory pages
by default. `--include-memories` exports only non-private memories; use
`--include-private-memories` only for a personal archive or an explicitly
approved review. It blocks export if wiki pages contain secret-looking values
unless you explicitly override it.

```bash
smash snapshot ~/Smash --output Smash-snapshot
smash snapshot ~/Smash --output Smash-snapshot --include-memories --force
smash snapshot ~/Smash --output personal-snapshot --include-memories --include-private-memories --force
```

## Agent Contract

For MCP clients, agents should use Smash in this order:

1. `status` to check readiness and safe next actions.
2. `recall` with an empty query once at the first substantive turn of a session.
3. `recall(query, budget="micro"|"small")` before broad file reads or asking the user to repeat durable context.
4. `ingest` before touching raw sources and after source edits for validation/rebuild checks.
5. `remember` only when the user explicitly asks Smash to remember something or approves a proposed memory.
6. `review` for memory inbox, profile, audit, log, explain, archive, restore, and forget workflows.
7. `admin` for backup, migration, graph export, captures, rebuilds, compatibility actions, and advanced maintenance.

Full command list: [CLI reference](docs/cli.html).

## Privacy And Safety

Smash itself is local-first:

- No telemetry in the installed CLI, MCP server, local web UI, or wiki runtime.
- No hosted backend.
- No external API calls from `serve.py` or `smash-mcp`.
- Raw sources and generated wiki pages are ignored by git by default.
- `smash backup` excludes `raw/` unless you explicitly pass `--include-raw`.
- Secret-looking API keys, provider tokens, JWTs, registry credentials, and
  private key blocks are detected in raw sources, captures, and release hygiene
  checks. `smash validate` and `smash doctor` also fail if secret-looking values
  are found inside wiki pages before they can be served through the local UI or
  returned through agent context.
- Optional semantic recall stays local: models load offline-only at recall
  time (only the explicit `smash semantic --setup` may fetch a model, once), and
  embeddings live in plain JSON under `.smash-cache/`.
- Automatic session hooks store proposal-only notes; transcript extraction
  skips tool calls and outputs, and no durable memory is written without review.
- The local web server binds to `127.0.0.1` and is not meant to be exposed to
  the internet without additional auth.

Before sharing a repo, demo, or wiki:

```bash
python3 smash.py doctor
python3 smash.py validate
python3 scripts/check_release_hygiene.py
```

More detail: [Security guide](docs/security.html).

## Documentation

| Need                                     | Go here                                                                                |
| ---------------------------------------- | -------------------------------------------------------------------------------------- |
| Run Smash for the first time             | [First 10 minutes](docs/getting-started.html)                      |
| "Does Smash read my conversations?"      | [The three questions everyone asks](docs/getting-started.html#faq) |
| Decide whether Smash fits                | [Why Smash?](docs/why-smash.html)                                  |
| Use the local viewer                     | [Web UI](docs/ui.html)                                             |
| Understand raw/wiki/memory               | [Concepts](docs/concepts.html)                                     |
| Use local CLI workflows                  | [CLI reference](docs/cli.html)                                     |
| Find a command                           | [CLI reference](docs/cli.html)                                     |
| Use Smash without extra agent wiring     | [Official skills](docs/skills.html)                                |
| Use local HTTP endpoints                 | [HTTP API](docs/api.html)                                          |
| Review security boundaries               | [Security model](docs/security.html)                               |
| Check scale limits and measure your wiki | [Smash Scale](docs/scale.html)                                     |
| Evaluate Smash for a small team          | [Team security review](docs/team-security.html)                    |
| Fix setup issues                         | [Troubleshooting](docs/troubleshooting.html)                       |

## Contributing

Contributions should come through pull requests targeting `main`. The `develop`
branch is a maintainer integration branch for larger release work before it is
proposed to `main`.

Before opening a PR:

```bash
python3 -m ruff check .
python3 -m pytest tests
python3 scripts/check_release_hygiene.py
python3 scripts/check_runtime_duplication.py
python3 scripts/check_tool_contract.py
git diff --check
```

Full contributor guide: [Contributing](docs/contributing.html).

Do not include personal wiki data, raw sources, registry tokens, `.env` files, or
local MCP credentials in a PR.

If Smash helps your agents remember better, [star it on GitHub](https://github.com//smash)
so more people can find it.
