---
name: smash-memory
description: Use after important user-approved decisions, when durable context should be proposed or reviewed, and for explicit Smash memory lifecycle work: remember, recall, review, update, archive, restore, forget, or explain local memories through the CLI without requiring MCP.
---

# Smash Memory

Use this skill after important user-approved decisions, preference changes, project conventions, or long work sessions that may deserve durable context. In a source checkout, replace `smash` with `python3 smash.py`. Do not silently save durable memory; propose first unless the user directly asks to remember, approves a proposal, or explicitly confirms an important decision should become durable memory.

If Smash session hooks are installed for this agent, the memory brief is injected automatically at session start — skip step 1 and go straight to task-specific recall.

1. Prime before work:
   ```bash
   smash brief "<current task>" [Smash-root]
   ```
2. Recall specific memory:
   ```bash
   smash recall "<topic>" [Smash-root]
   ```
3. End a session with proposal-only memory candidates:
   ```bash
   smash session-end <session-notes-or-transcript> [Smash-root] --limit 3
   ```
   Use `-` as the input when piping a transcript on stdin. Show the proposals to the user; do not save durable memory until the user approves one.
4. Save an explicit memory:

   ```bash
   smash remember "<user-approved memory>" [Smash-root] --type note --scope user
   ```

   Use `--project <slug>` for project-scoped memory, `--visibility private|project|team` for sharing intent, `--review-after YYYY-MM-DD` for stale-risk memories, and `--expires-at YYYY-MM-DD` for temporary context.
   When a brief or recall reports a memory backlog (pending captures or reviews above threshold), offer the user a short consolidation pass:

   ```bash
   smash consolidate [Smash-root]
   ```

   The plan is read-only: it groups duplicates and recurring themes and prints accept/discard/review commands. Apply an action only after the user approves it.

5. Review and explain before trusting uncertain memory:
   ```bash
   smash memory-inbox [Smash-root]
   smash explain-memory <name-or-title> [Smash-root]
   smash review-memory <name-or-title> [Smash-root]
   ```
6. Change lifecycle safely:
   ```bash
   smash update-memory <name-or-title> "<new text>" [Smash-root]
   smash archive-memory <name-or-title> [Smash-root] --reason "<why>"
   smash restore-memory <name-or-title> [Smash-root]
   smash forget-memory <name-or-title> [Smash-root] --confirm
   ```

When duplicate or conflict warnings appear, prefer updating, reviewing, or archiving existing memory over creating another page.
