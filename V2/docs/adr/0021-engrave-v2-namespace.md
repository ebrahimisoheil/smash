# ADR-0021: Engrave V2 namespace and clean-break rename

**Status:** accepted

## Context

The V2 workspace and its Obsidian planning namespace were initially branded
SMASH. The product name is now Engrave. V2 is still separate from the legacy
root workspace, so the rename must be complete inside V2 without rewriting V1
files or the root `SMASH_V2.md` source document.

## Decision

Use Engrave as the canonical V2 namespace:

- Rust packages and internal dependency names use `engrave-*`.
- Product-specific environment variables use the `ENGRAVE_*` prefix.
- Compose, Docker, database, bucket, user, and volume identifiers use
  `engrave` / `engrave-*` naming.
- User-facing headings and API metadata use `Engrave` or `ENGRAVE V2`.
- The Obsidian namespace uses `ENGRAVE V2/` and `engrave/v2` tags.

This is a clean break. V2 does not accept legacy `SMASH_*` variables or
preserve compatibility aliases. Existing local `smash_*` Docker volumes are
left untouched; the renamed Compose project starts with fresh `engrave_*`
volumes and does not implicitly adopt old data.

The literal `SMASH_V2.md` filename remains in source-document links because
that file is outside the V2 rename boundary.

## Consequences

The rename changes local configuration, Cargo package selectors, generated
OpenAPI metadata, Compose resource names, and Obsidian paths. Existing V2
commands and local configuration must be updated before use. Historical V1
references in the V1-classification contract remain labelled as historical
source identifiers rather than being rewritten into inaccurate V2 names.
