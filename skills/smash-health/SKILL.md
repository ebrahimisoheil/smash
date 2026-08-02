---
name: smash-health
description: Use at the start of Smash work when readiness is unclear, after installs or upgrades, and before repairs; verify health, inspect interrupted writes, back up, and repair generated indexes without MCP.
---

# Smash Health

Use the `smash` CLI. Load this skill before trusting a new or changed Smash wiki, after installs/upgrades, and before broad repair or restore work. In a source checkout, replace `smash` with `python3 smash.py`. MCP and the local web viewer are optional; `smash serve` is only for humans to browse the wiki.

1. Check readiness first:
   ```bash
   smash health [Smash-root]
   ```
2. If the output mentions interrupted or stale operations, inspect them before repair:
   ```bash
   smash operations [Smash-root]
   ```
3. Before broad repairs, migrations, or restore work, create a backup:
   ```bash
   smash backup [Smash-root]
   ```
4. Repair only generated or structural state that Smash reports as safe:
   ```bash
   smash doctor --fix [Smash-root]
   smash rebuild-index [Smash-root]
   smash rebuild-backlinks [Smash-root]
   ```
5. Validate before saying the wiki is healthy:
   ```bash
   smash validate [Smash-root]
   smash health [Smash-root]
   ```

If the user asks whether MCP is ready, run `smash verify-mcp [Smash-root]`. Do not start `smash serve` for MCP or CLI work.

To check whether optional local semantic recall is active (lexical is always the fallback):

```bash
smash semantic [Smash-root]
```

It reports the provider tier, model, and index state, and prints the exact setup command when the layer is available but not yet enabled.
