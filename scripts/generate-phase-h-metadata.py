#!/usr/bin/env python3
"""Generate draft Registry metadata from the tagged package version.

This intentionally does not publish and refuses to guess a Registry package
type; refresh the official schema/package mapping at release time.
"""
import json
import pathlib
import sys

version = sys.argv[1] if len(sys.argv) > 1 else "0.1.0"
metadata = {
    "$schema": "https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json",
    "name": "io.github.ebrahimisoheil/engrave",
    "description": "Governed ENGRAVE Memory and evidence MCP server",
    "repository": {"url": "https://github.com/ebrahimisoheil/engrave", "source": "github"},
    "version": version,
    "packages": [],
    "_phase_h_note": "Package type must be filled from the current official Registry schema for the exact release artifact; publication is intentionally disabled.",
}
json.dump(metadata, sys.stdout, indent=2)
sys.stdout.write("\n")
