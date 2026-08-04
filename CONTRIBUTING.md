# Contributing to ENGRAVE V2

ENGRAVE V2 is a Rust workspace with a Docker Compose Community Edition. Read
the relevant roadmap phase and ADR before changing a contract or a security
boundary. Authorization belongs in `engrave-core` and the shared gateway;
prompts, skills, UI code, and release scripts must not become alternate policy
engines.

## Local setup

Install Rust, Docker Engine with Compose v2, Node.js/npm, and PostgreSQL
client tools. For a local stack:

```sh
cp .env.example .env
# replace every placeholder secret in .env
docker compose --env-file .env up --build -d
```

The complete operator workflow is documented in
[`docs/community-edition.md`](docs/community-edition.md). Keep `.env`,
connector credentials, database dumps, and generated release directories out
of commits.

## Checks before review

Run the fast checks for every change:

```sh
cargo fmt --all
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
git diff --check
```

Changes involving PostgreSQL, migrations, worker behavior, MCP, connectors,
Compose, or release artifacts also need the relevant disposable integration
suite. Record exact commands and outcomes in the applicable phase ledger;
ignored tests are evidence only when run against disposable infrastructure.

## Change boundaries

- Recheck tenant, Area, actor, agent, host, session, purpose, and Rule state at
  the governed backend boundary.
- Keep Memory proposal/review/approval explicit; retrieval and aggressive
  search must never activate Memory.
- Treat connector output as untrusted evidence and credentials as secrets;
  never put either in prompts, MCP arguments, logs, fixtures, or commits.
- Preserve deterministic provenance, bounded budgets, cancellation, partial
  results, and durable audit identifiers.
- Add or update adversarial tests when changing authorization, publication,
  connector, provenance, or release behavior.

## Release changes

Release artifacts are built only from a clean exact `vX.Y.Z` tag. Use
`scripts/build-release.sh` and `scripts/verify-release.sh`; neither publishes
images, packages, Registry metadata, or remote MCP endpoints. A release
requires explicit owner/security review for any external publication and must
leave unresolved OAuth, package ownership, rollback, or production-quality
gates visible in `docs/phase-j-ledger.md`. Use
[`docs/release-review.md`](docs/release-review.md) as the handoff checklist.

Please include a concise description of the changed contract, tests run, data
or migration implications, and any remaining blocker in the pull request.
