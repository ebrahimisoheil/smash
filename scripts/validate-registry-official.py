#!/usr/bin/env python3
"""Validate server.json with the official MCP Registry; never publish."""
import json
import os
import sys
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

if len(sys.argv) != 2:
    raise SystemExit("usage: validate-registry-official.py SERVER_JSON")

metadata_path = sys.argv[1]
endpoint = os.environ.get(
    "MCP_REGISTRY_VALIDATE_URL",
    "https://registry.modelcontextprotocol.io/v0.1/validate",
)
payload = json.dumps(json.load(open(metadata_path)), separators=(",", ":")).encode()
request = Request(endpoint, data=payload, headers={"content-type": "application/json"}, method="POST")
try:
    with urlopen(request, timeout=20) as response:
        result = json.load(response)
except (HTTPError, URLError, TimeoutError) as error:
    raise SystemExit(f"official Registry validation request failed: {error}") from error

if result.get("valid") is not True:
    raise SystemExit(f"official Registry validation rejected metadata: {json.dumps(result)}")
print(f"official Registry schema validation passed: {metadata_path}")
