# ADR-0021: Engrave V2 namespace and clean-break rename

**Status:** accepted

## Context

The workspace and its Obsidian planning namespace were initially branded
SMASH. The product name is now Engrave, and this repository contains only the
Engrave V2 workspace. Legacy V1 files and the historical roadmap source are
outside the repository boundary.

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

Historical roadmap references remain descriptive rather than linked because
the source material is outside the repository boundary.

## Consequences

The rename changes local configuration, Cargo package selectors, generated
OpenAPI metadata, Compose resource names, and Obsidian paths. Existing V2
commands and local configuration must be updated before use. Historical V1
references in the V1-classification contract remain labelled as historical
source identifiers rather than being rewritten into inaccurate V2 names.
