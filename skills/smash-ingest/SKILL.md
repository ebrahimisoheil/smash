---
name: smash-ingest
description: Use when raw files are present, source pages look stale, or a user asks to ingest notes into Smash; refresh source-backed wiki pages, propose memories, and validate updates through the CLI without MCP.
---

# Smash Ingest

Use `smash ingest-status` as the source of truth. Load this skill when the user drops files into `raw/`, mentions new notes/transcripts/docs, or asks what Smash should learn next. In a source checkout, replace `smash` with `python3 smash.py`. The command tells you which raw files need work and which checks must run next.

1. Inspect the ingest plan:
   ```bash
   smash ingest-status [Smash-root]
   ```
2. If Smash reports secret-looking values, unreadable files, or unsafe paths, stop and ask the user to fix or redact them.
3. Read only the pending raw files named by the ingest plan. Create or update one `wiki/sources/...` page per raw file, and update existing concept/entity/exploration/memory pages before creating thin duplicates.
4. Keep durable memory proposal-only until the user approves it:
   ```bash
   smash propose-memories raw/<file> [Smash-root]
   ```
5. After writing wiki pages, rebuild generated indexes and validate:
   ```bash
   smash rebuild-index [Smash-root]
   smash rebuild-backlinks [Smash-root]
   smash validate [Smash-root]
   smash health [Smash-root]
   ```

Do not put raw source contents into chat unless needed for the current ingest task. Preserve source paths and provenance on generated pages.
