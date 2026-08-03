#!/usr/bin/env python3
"""Bounded provider operations benchmark.

This records latency and provider rate-limit headers without persisting or
printing credentials. It deliberately uses a small request set so it is safe
to run against development keys.
"""
import concurrent.futures
import json
import os
import statistics
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path


def load_env() -> None:
    path = Path(".env.local")
    if not path.exists():
        return
    for line in path.read_text().splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key in {"VOYAGE_API_KEY", "OPENAI_API_KEY"} and key not in os.environ:
            os.environ[key] = value.strip().strip("'\"")


def one_call(endpoint: str, key: str, model: str) -> dict:
    body = json.dumps({"input": ["What approval checkpoint is required?"], "model": model}).encode()
    request = urllib.request.Request(
        endpoint,
        data=body,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            payload = json.load(response)
            headers = {name.lower(): response.headers.get(name) for name in (
                "retry-after", "x-ratelimit-limit-requests", "x-ratelimit-remaining-requests",
                "x-ratelimit-reset-requests", "x-ratelimit-limit-tokens",
                "x-ratelimit-remaining-tokens") if response.headers.get(name) is not None}
            return {"ok": True, "latency_ms": (time.perf_counter() - started) * 1000,
                    "dimension": len(payload["data"][0]["embedding"]), "rate_limit_headers": headers,
                    "usage": payload.get("usage")}
    except urllib.error.HTTPError as error:
        return {"ok": False, "latency_ms": (time.perf_counter() - started) * 1000,
                "status": error.code, "retry_after": error.headers.get("retry-after")}
    except Exception as error:
        return {"ok": False, "latency_ms": (time.perf_counter() - started) * 1000,
                "error_type": type(error).__name__}


def profile(name: str, endpoint: str, key_name: str, model: str) -> dict:
    key = os.environ.get(key_name)
    if not key:
        return {"profile": name, "skipped": f"{key_name} missing"}
    runs = []
    for concurrency in (1, 4, 8):
        started = time.perf_counter()
        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
            results = list(pool.map(lambda _: one_call(endpoint, key, model), range(concurrency)))
        elapsed = (time.perf_counter() - started) * 1000
        latencies = [item["latency_ms"] for item in results]
        successful = [item for item in results if item.get("ok")]
        runs.append({
            "concurrency": concurrency,
            "requests": len(results),
            "successes": len(successful),
            "p50_ms": statistics.median(latencies),
            "p95_ms": sorted(latencies)[max(0, int(len(latencies) * .95) - 1)],
            "p99_ms": max(latencies),
            "wall_ms": elapsed,
            "rate_limit_headers": [item["rate_limit_headers"] for item in successful if item.get("rate_limit_headers")],
            "usage": [item["usage"] for item in successful if item.get("usage")],
            "errors": [item for item in results if not item.get("ok")],
        })
    return {"profile": name, "model": model, "date_utc": datetime.now(timezone.utc).isoformat(), "runs": runs,
            "cost": "not available from provider response; no cost invented"}


def main() -> None:
    load_env()
    result = {"profiles": [
        profile("voyage-3-lite", os.environ.get("VOYAGE_API_ENDPOINT", "https://api.voyageai.com/v1/embeddings"), "VOYAGE_API_KEY", os.environ.get("VOYAGE_EMBEDDING_MODEL", "voyage-3-lite")),
        profile("openai-large", os.environ.get("OPENAI_EMBEDDING_ENDPOINT", "https://api.openai.com/v1/embeddings"), "OPENAI_API_KEY", "text-embedding-3-large"),
    ]}
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
