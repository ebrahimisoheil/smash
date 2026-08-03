#!/usr/bin/env python3
"""Measure provider-only embedding compatibility and latency.

This intentionally does not claim retrieval quality: Recall/MRR/nDCG require
the complete authorized retrieval harness and are recorded separately.
"""
import json
import os
import statistics
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path


def load_local_env() -> None:
    env_path = Path(".env.local")
    if not env_path.exists():
        return
    for line in env_path.read_text().splitlines():
        if line.startswith("VOYAGE_API_KEY=") and "VOYAGE_API_KEY" not in os.environ:
            os.environ["VOYAGE_API_KEY"] = line.split("=", 1)[1].strip().strip("'\"")


def embed(texts: list[str], model: str) -> tuple[int, float]:
    body = json.dumps({"input": texts, "model": model}).encode()
    request = urllib.request.Request(
        os.environ.get("VOYAGE_API_ENDPOINT", "https://api.voyageai.com/v1/embeddings"),
        data=body,
        headers={"Authorization": f"Bearer {os.environ['VOYAGE_API_KEY']}", "Content-Type": "application/json"},
    )
    started = time.perf_counter()
    with urllib.request.urlopen(request, timeout=20) as response:
        payload = json.load(response)
    elapsed_ms = (time.perf_counter() - started) * 1000
    return len(payload["data"][0]["embedding"]), elapsed_ms


def main() -> None:
    load_local_env()
    if not os.environ.get("VOYAGE_API_KEY"):
        raise SystemExit("VOYAGE_API_KEY is required via environment or .env.local")
    model = os.environ.get("VOYAGE_EMBEDDING_MODEL", "voyage-3-lite")
    queries = [
        "What approval checkpoint does Acme require before renewal?",
        "Is executive review plus security sign-off needed for the renewal?",
        "What campaign approval process is active?",
    ]
    cold = [embed([query], model) for query in queries]
    repeated = [embed([queries[0]], model) for _ in range(3)]
    cold_ms = [latency for _, latency in cold]
    repeated_ms = [latency for _, latency in repeated]
    dimensions = sorted({dimension for dimension, _ in cold + repeated})
    result = {
        "provider": "voyage-compatible",
        "model": model,
        "date_utc": datetime.now(timezone.utc).isoformat(),
        "queries": len(queries),
        "native_dimensions_observed": dimensions,
        "cold_latency_ms": {"p50": statistics.median(cold_ms), "min": min(cold_ms), "max": max(cold_ms)},
        "repeated_request_latency_ms": {"p50": statistics.median(repeated_ms), "min": min(repeated_ms), "max": max(repeated_ms)},
        "output_dimension_required": 1024,
        "projection_required": dimensions != [1024],
        "retrieval_metrics": "not measured by provider-only harness",
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
