#!/usr/bin/env python3
"""Validate the release-local shape of MCP Registry metadata; never publish."""
import json
import re
import sys
from pathlib import Path

if len(sys.argv) != 3:
    raise SystemExit("usage: verify-registry-metadata.py SERVER_JSON ARTIFACT_DIR")

metadata_path = Path(sys.argv[1])
artifact_dir = Path(sys.argv[2])
metadata = json.loads(metadata_path.read_text())
official_schema = "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json"
supported_registry_types = {"npm", "pypi", "nuget", "oci", "mcpb"}
version = metadata.get("version")
if not isinstance(version, str) or not re.fullmatch(r"\d+\.\d+\.\d+", version):
    raise SystemExit("Registry metadata must contain a semantic version")
if metadata.get("name") != "io.github.ebrahimisoheil/engrave":
    raise SystemExit("Registry metadata has the wrong server name")
if metadata.get("$schema") != official_schema:
    raise SystemExit(f"Registry metadata must use the current schema URL: {official_schema}")
if not isinstance(metadata.get("description"), str) or not metadata["description"].strip():
    raise SystemExit("Registry metadata must contain a description")
repository = metadata.get("repository")
if not isinstance(repository, dict) or not repository.get("url") or not repository.get("source"):
    raise SystemExit("Registry metadata must contain repository url and source")
release_tag_path = artifact_dir / "RELEASE_TAG"
if not release_tag_path.is_file() or release_tag_path.read_text().strip() != f"v{version}":
    raise SystemExit("Registry metadata release tag file does not match version")
packages = metadata.get("packages")
if not isinstance(packages, list) or not packages:
    raise SystemExit("Registry metadata must contain at least one package")
for package in packages:
    if not isinstance(package, dict):
        raise SystemExit("each Registry package must be an object")
    registry_type = package.get("registryType")
    identifier = package.get("identifier")
    transport = package.get("transport")
    if registry_type != "local-test" and registry_type not in supported_registry_types:
        raise SystemExit(f"unsupported Registry package type: {registry_type!r}")
    if not isinstance(identifier, str) or not identifier.strip():
        raise SystemExit("each Registry package needs a non-empty identifier")
    if not isinstance(transport, dict) or transport.get("type") not in {
        "stdio",
        "sse",
        "streamable-http",
    }:
        raise SystemExit("each Registry package needs a supported transport type")
    if package.get("version") not in (None, version):
        raise SystemExit("package version must match the server version")
    if registry_type == "local-test":
        relative = Path(identifier)
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit("local-test package identifier must stay inside the artifact directory")
        local_artifact = artifact_dir / relative
        if not local_artifact.is_file():
            raise SystemExit(f"Registry package artifact is missing: {local_artifact.name}")
    elif registry_type == "mcpb":
        if not re.fullmatch(r"[0-9a-fA-F]{64}", str(package.get("fileSha256", ""))):
            raise SystemExit("mcpb packages require a 64-character fileSha256")
    elif registry_type == "oci":
        if not re.match(r"^(docker\.io/|ghcr\.io/|[^/]+\.pkg\.dev/|[^/]+\.azurecr\.io/|mcr\.microsoft\.com/)", identifier):
            raise SystemExit("OCI package identifiers must use a supported registry host")
print(f"registry metadata shape verified: {metadata_path}")
