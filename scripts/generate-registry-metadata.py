#!/usr/bin/env python3
"""Generate release-bound MCP Registry metadata; never publishes."""
import json
import os
import subprocess
import sys
from pathlib import Path

version = sys.argv[1] if len(sys.argv) > 1 else ""
artifact_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("dist") / version
tag = subprocess.run(["git", "describe", "--tags", "--exact-match"], capture_output=True, text=True, check=False).stdout.strip()
if tag != f"v{version}":
    raise SystemExit(f"exact tag v{version} required; found {tag or 'none'}")
artifacts = sorted(path.name for path in artifact_dir.glob("engrave-mcp-*") if path.is_file())
if not artifacts:
    raise SystemExit(f"no MCP artifacts found in {artifact_dir}")
registry_type = os.environ.get("MCP_REGISTRY_TYPE", "").strip()
if not registry_type:
    raise SystemExit("MCP_REGISTRY_TYPE must be set after confirming the current official Registry schema")
if registry_type == "local-test":
    packages = [
        {
            "registryType": registry_type,
            "identifier": artifact,
            "version": version,
            "transport": {"type": "stdio"},
        }
        for artifact in artifacts
    ]
else:
    identifier = os.environ.get("MCP_REGISTRY_IDENTIFIER", "").strip()
    if not identifier:
        raise SystemExit(
            "MCP_REGISTRY_IDENTIFIER must name the separately published official package"
        )
    package = {
        "registryType": registry_type,
        "identifier": identifier,
        "version": version,
        "transport": {"type": "stdio"},
    }
    if registry_type == "mcpb":
        file_sha256 = os.environ.get("MCP_REGISTRY_FILE_SHA256", "").strip()
        if not file_sha256:
            raise SystemExit("MCP_REGISTRY_FILE_SHA256 is required for an MCPB package")
        package["fileSha256"] = file_sha256
    packages = [package]
print(json.dumps({
    "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
    "name": "io.github.ebrahimisoheil/engrave",
    "description": "Governed ENGRAVE Memory and evidence MCP server",
    "repository": {"url": "https://github.com/ebrahimisoheil/engrave", "source": "github"},
    "version": version,
    "packages": packages,
}, indent=2))
