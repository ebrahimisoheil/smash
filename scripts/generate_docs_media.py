#!/usr/bin/env python3
"""Validate checked-in docs media assets.

This script used to generate synthetic GIFs. Smash now uses real product
captures and hand-maintained diagrams for public docs, so this script is a
non-destructive verifier. It keeps the old command name for maintainer muscle
memory while refusing to overwrite product screenshots.
"""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "docs" / "assets"

REQUIRED_ASSETS = {
    "favicon.svg",
    "logo.svg",
    "logo-512.png",
    "smash-site.png",
    "smash-web-ui.png",
    "smash-home.png",
    "smash-health.png",
    "smash-graph.png",
    "smash-cli.png",
    "smash-mcp.png",
    "smash-memory-flow.svg",
    "smash-aha.svg",
    "smash-truth.svg",
    "smash-truth.gif",
    "smash-aha.gif",
    "smash-ui-tour.gif",
    "smash-cli-tour.gif",
    "smash-mcp-agent-chat.gif",
    "smash-product-tour-dark.gif",
}

OPTIONAL_REAL_CAPTURES = {
    "smash-home-dark.png",
    "smash-ingest-dark.png",
    "smash-brief-dark.png",
    "smash-memory-dashboard-dark.png",
    "smash-explain-memory-dark.png",
    "smash-graph-dark.png",
}

SUPPORTED_BINARY_TYPES = {
    ".gif": "gif",
    ".png": "png",
}


def _asset_kind(path: Path) -> str:
    if path.suffix == ".svg":
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            return ""
        return "svg" if "<svg" in text[:500].lower() else ""
    try:
        header = path.read_bytes()[:16]
    except OSError:
        return ""
    if header.startswith(b"\x89PNG\r\n\x1a\n"):
        return "png"
    if header.startswith((b"GIF87a", b"GIF89a")):
        return "gif"
    return ""


def validate_docs_media() -> tuple[int, list[str]]:
    findings: list[str] = []
    if not ASSETS.exists():
        return 1, [f"missing docs assets directory: {ASSETS}"]

    for name in sorted(REQUIRED_ASSETS):
        path = ASSETS / name
        if not path.exists():
            findings.append(f"missing required docs media asset: docs/assets/{name}")
            continue
        if path.stat().st_size <= 0:
            findings.append(f"empty docs media asset: docs/assets/{name}")
            continue
        expected_kind = "svg" if path.suffix == ".svg" else SUPPORTED_BINARY_TYPES.get(path.suffix)
        actual_kind = _asset_kind(path)
        if expected_kind and actual_kind != expected_kind:
            findings.append(
                f"docs/assets/{name} has unexpected format: expected {expected_kind}, got {actual_kind or 'unknown'}"
            )

    for name in sorted(OPTIONAL_REAL_CAPTURES):
        path = ASSETS / name
        if path.exists() and path.stat().st_size <= 0:
            findings.append(f"empty optional docs media capture: docs/assets/{name}")

    return (1 if findings else 0), findings


def main() -> int:
    code, findings = validate_docs_media()
    if findings:
        print("Docs media validation failed:")
        for finding in findings:
            print(f"- {finding}")
        return code

    print("Docs media assets are present and valid.")
    print("No files were generated. Update docs media through real product captures, then commit the assets.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
