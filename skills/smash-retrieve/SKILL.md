---
name: smash-retrieve
description: Use before answering work that may depend on user memory, project history, source-backed notes, or prior decisions; retrieve compact Smash context through the CLI without loading the whole wiki or requiring MCP.
---

# Smash Retrieve

Use bounded CLI commands so the agent does not dump the whole wiki into context. Load this skill proactively at the first substantive turn of a session, before project/release/debug/design work, or whenever the answer may depend on prior Smash memory. In a source checkout, replace `smash` with `python3 smash.py`.

1. If readiness is unclear, start with:
   ```bash
   smash health [Smash-root]
   ```
2. If the user is inside a project repo and Smash has no project context yet, seed allowlisted source-backed context before broad searching:
   ```bash
   smash seed . [Smash-root]
   ```
   This reads project docs/rule files, blocks secret-looking values, and does not create durable memories.
3. For most questions, use a compact query packet:
   ```bash
   smash query "<question or task>" [Smash-root] --budget micro
   ```
   Read `recall_capsule` first. Increase to `--budget small`, `--budget medium`, or `--budget large` only when the packet says more context is needed.
4. Before longer work, prime from memory:
   ```bash
   smash brief "<current task>" [Smash-root]
   ```
5. For graph context, stay bounded:
   ```bash
   smash graph-summary "<topic>" [Smash-root] --limit 40 --depth 1
   ```
6. For performance checks, use:
   ```bash
   smash benchmark "<topic>" [Smash-root] --budget small
   ```

Do not enumerate every page, grep raw files, or request the full graph unless the user explicitly asks for an export or exhaustive audit, or the compact packet is insufficient and tells you which follow-up to use.

Recalled memories carry `confidence` labels and, when the optional local semantic tier is installed, a `match` field: `lexical`, `hybrid`, or `semantic`. Treat `semantic` matches (paraphrase similarity, capped confidence) and `weak` matches as hints to verify with the user, not facts to act on.
