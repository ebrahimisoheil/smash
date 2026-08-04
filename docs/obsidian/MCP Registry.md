# MCP Registry

## Purpose

This note tracks ENGRAVE’s future publication to the official Model Context
Protocol Registry. It is not the internal connector catalog and it is not the
plugin manifest.

The official Registry publishes server metadata, not the application artifact.
The package/binary/container must be published separately. The Registry is
currently documented as preview technology, so schema and publication behavior
must be checked again at release time. The current package-type reference
(checked 2026-08-04) documents `npm`, `pypi`, `nuget`, `oci`, and `mcpb`; it does
not provide a generic native-binary package type.

Official reference: [MCP Registry publish quickstart](https://modelcontextprotocol.io/registry/quickstart)

## Identity

| Field | Value | Status |
|---|---|---|
| Registry name | `io.github.ebrahimisoheil/engrave` | Reserved by roadmap; verify ownership before publication |
| Display name | ENGRAVE | Draft |
| Repository | `https://github.com/ebrahimisoheil/engrave` | Confirm actual public repository before publishing |
| Publisher | `ebrahimisoheil` | Verify with the chosen Registry authentication method |
| Current version | `0.1.0` workspace baseline | Must match release metadata |
| Publication status | Not published | Do not publish yet |
| Transport now | stdio | Implement first |
| Transport later | Streamable HTTP | Requires OAuth and protected-resource metadata |

## Registry package metadata draft

Create `server.json` only when the MCP server has a releasable artifact. Phase
J provides `scripts/generate-registry-metadata.py`: its disposable
`MCP_REGISTRY_TYPE=local-test` mode binds synthetic package entries to the
tagged local artifacts, while official generation requires an explicitly
published package identifier (and an MCPB checksum). The empty example below
is a draft and must not be published.

```json
{
  "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
  "name": "io.github.ebrahimisoheil/engrave",
  "description": "Governed ENGRAVE Memory and evidence MCP server",
  "repository": {
    "url": "https://github.com/ebrahimisoheil/engrave",
    "source": "github"
  },
  "version": "0.1.0",
  "packages": []
}
```

The exact `registryType` and package publication mechanism must be confirmed
for the selected ENGRAVE distribution before publishing. Do not invent a
Registry package type for the Rust binary. Publish a verified platform package,
MCPB artifact, or container distribution first, then represent that supported
package type accurately.

## Required publication sequence

1. Tag a release from a clean, verified commit.
2. Build the standalone stdio MCP artifact for supported platforms.
3. Publish the artifact to its package/distribution channel.
4. Generate `server.json` from the exact release version.
5. Ensure the server identity matches the package verification metadata.
6. Validate the metadata against the current official schema.
7. Authenticate with `mcp-publisher` using the correct namespace owner.
8. Publish metadata with `mcp-publisher publish`.
9. Query the Registry API and compare the published record to the release.
10. Record the publication response, version, checksum, repository commit, and
    rollback/revocation plan.

## Local plugin configuration

The plugin is separate from Registry metadata. For local development:

```json
{
  "mcpServers": {
    "engrave": {
      "type": "stdio",
      "command": "cargo",
      "args": ["run", "-p", "engrave-mcp"]
    }
  }
}
```

The published plugin may later point at a packaged executable. A hosted MCP
endpoint is a separate release path and must not be declared until HTTP OAuth,
audience validation, protected-resource metadata, tenant isolation, audit,
rate limits, and revocation are complete.

## Capability inventory

| Capability | Type | Version | Status |
|---|---|---:|---|
| `status` | Tool | 1.0.0 | verified locally |
| `recall` | Tool | 1.0.0 | verified locally |
| `propose_memory` | Tool | 1.0.0 | verified locally |
| `ingest_source` | Tool | 1.0.0 | verified locally |
| `review` | Tool | 1.0.0 | verified locally |
| `rules` | Tool/resource | 1.0.0 | verified locally |
| governed evidence | Resource | 1.0.0 | verified locally |
| session prompts | Prompt | 1.0.0 | verified locally |

Every capability must route through the Phase G evaluator and gateway. Tool
descriptions are not trust boundaries.

## Publication blockers

- [x] Standalone stdio MCP server exists.
- [x] Tool/resource/prompt schemas are deterministic.
- [x] Structured errors use core application error codes.
- [x] All calls carry host, actor, agent, tenant, Area, session, and purpose context.
- [x] Rule decisions are durable and auditable.
- [x] Connector credentials are tenant-bound and isolated.
- [x] Current Registry package types are confirmed; no generic native-binary
  type is claimed.
- [ ] Package verification metadata is implemented.
- [x] The local validator checks the current schema's required identity,
  package, transport, and version/tag fields.
- [ ] `server.json` is validated by the official Registry service for the
  selected published package.
- [ ] Artifact publication is reproducible.
- [ ] Security review is complete.
- [ ] Publication is approved by the project owner.

## Do not publish yet

The Registry is a discovery channel, not a trust authority. ENGRAVE must keep
its own trusted connector catalog, publisher verification, allowed scopes,
permitted tools, Rule restrictions, security notices, and revocation state.
