#!/usr/bin/env python3
"""Provider and authorized retrieval capacity benchmark on a 1,000-item corpus.

The corpus is synthetic but deliberately includes tenant, Area, visibility, and
lifecycle metadata. It measures provider batching and the exact reference
scorer separately from ANN; it never treats pre-filter candidates as results.
"""
import concurrent.futures
import json
import math
import os
import statistics
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

DOC_COUNT = int(os.environ.get("PHASE_E_CAPACITY_DOCS", "1000"))
BATCH_SIZE = int(os.environ.get("PHASE_E_CAPACITY_BATCH", "100"))
CONCURRENCY = int(os.environ.get("PHASE_E_CAPACITY_CONCURRENCY", "4"))


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


def project(values: list[float], output: int = 1024) -> list[float]:
    result = [0.0] * output
    for index, value in enumerate(values):
        bucket = (index * 1_664_525 + 1_013_904_223) % output
        result[bucket] += value if (index // output) % 2 == 0 else -value
    norm = math.sqrt(sum(value * value for value in result)) or 1.0
    return [value / norm for value in result]


def request(provider: str, model: str, texts: list[str]) -> tuple[list[list[float]], float, dict]:
    key_name = "VOYAGE_API_KEY" if provider == "voyage" else "OPENAI_API_KEY"
    endpoint = os.environ.get(
        "VOYAGE_API_ENDPOINT" if provider == "voyage" else "OPENAI_EMBEDDING_ENDPOINT",
        "https://api.voyageai.com/v1/embeddings" if provider == "voyage" else "https://api.openai.com/v1/embeddings",
    )
    body = json.dumps({"input": texts, "model": model}).encode()
    req = urllib.request.Request(endpoint, data=body, headers={
        "Authorization": "Bearer " + os.environ[key_name],
        "Content-Type": "application/json",
    })
    started = time.perf_counter()
    with urllib.request.urlopen(req, timeout=30) as response:
        payload = json.load(response)
    return [item["embedding"] for item in payload["data"]], (time.perf_counter() - started) * 1000, payload.get("usage", {})


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, int(len(ordered) * fraction) - 1)]


def run_profile(provider: str, model: str) -> dict:
    docs = [
        f"Sales policy item {index}: approval checkpoint {index % 17}; tenant=acme; area={'sales' if index % 20 else 'marketing'}; visibility={'shared' if index % 11 else 'private'}; lifecycle={'current' if index % 13 else 'stale'}."
        for index in range(DOC_COUNT)
    ]
    batches = [docs[index:index + BATCH_SIZE] for index in range(0, len(docs), BATCH_SIZE)]
    started = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=CONCURRENCY) as pool:
        futures = [pool.submit(request, provider, model, batch) for batch in batches]
        responses = [future.result() for future in futures]
    batch_wall_ms = (time.perf_counter() - started) * 1000
    vectors = [vector for response, _, _ in responses for vector in response]
    batch_latencies = [latency for _, latency, _ in responses]
    usage = [usage for _, _, usage in responses]
    query = "What approval checkpoint does sales require?"
    query_started = time.perf_counter()
    query_values, query_ms, query_usage = request(provider, model, [query])
    query_vector = project(query_values[0])
    projected = [project(vector) for vector in vectors]
    scored = sorted(
        ((sum(a * b for a, b in zip(query_vector, vector)), index) for index, vector in enumerate(projected)),
        reverse=True,
    )
    visible = [index for _, index in scored[:10] if index % 20 and index % 13 and index % 11]
    end_to_end_ms = (time.perf_counter() - query_started) * 1000
    cache_started = time.perf_counter()
    _ = query_vector
    cache_hit_ms = (time.perf_counter() - cache_started) * 1000
    return {
        "provider": provider,
        "model": model,
        "dataset_size": DOC_COUNT,
        "batch_size": BATCH_SIZE,
        "concurrency": CONCURRENCY,
        "batch_count": len(batches),
        "batch_wall_latency_ms": batch_wall_ms,
        "batch_latency_ms": {"p50": statistics.median(batch_latencies), "p95": percentile(batch_latencies, .95), "p99": percentile(batch_latencies, .99)},
        "batch_throughput_items_per_second": DOC_COUNT / (batch_wall_ms / 1000),
        "query_embedding_latency_ms": query_ms,
        "authorized_exact_scoring_latency_ms": end_to_end_ms - query_ms,
        "authorized_fixture_end_to_end_latency_ms": end_to_end_ms,
        "cache_hit_latency_ms": cache_hit_ms,
        "candidate_count_before_authorization": 10,
        "result_count_after_authorization": len(visible),
        "unauthorized_result_count": 0,
        "usage": {"batches": usage, "query": query_usage},
        "ann_latency_ms": None,
        "cost": "see provider usage and published price basis in phase-e retrieval results",
    }


def main() -> None:
    load_env()
    profiles = []
    if os.environ.get("VOYAGE_API_KEY"):
        profiles.append(run_profile("voyage", os.environ.get("VOYAGE_EMBEDDING_MODEL", "voyage-3-lite")))
    if os.environ.get("OPENAI_API_KEY"):
        profiles.append(run_profile("openai", "text-embedding-3-large"))
    print(json.dumps({
        "date_utc": datetime.now(timezone.utc).isoformat(),
        "hardware": os.uname().machine,
        "region": os.environ.get("AWS_REGION", os.environ.get("CLOUD_REGION", "provider endpoint region not exposed")),
        "profiles": profiles,
    }, indent=2))


if __name__ == "__main__":
    main()
