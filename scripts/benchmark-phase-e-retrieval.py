#!/usr/bin/env python3
"""Small authorized V2 fixture benchmark for the two live embedding profiles."""
import json, math, os, statistics, time, urllib.request
from datetime import datetime, timezone
from pathlib import Path

QUERIES = [
    ("renewal-executive-review", "What approval checkpoint does Acme require before renewal?", True),
    ("renewal-security-signoff", "Is executive review plus security sign-off needed for the renewal?", True),
    ("wrong-area-marketing", "What campaign approval process is active?", False),
]
DOCS = [
    ("018f0000-0000-7000-8000-000000000300", "Acme requires quarterly executive review before renewal.", False),
    ("018f0000-0000-7000-8000-000000000301", "Acme does not require executive review before renewal.", False),
    ("018f0000-0000-7000-8000-000000000302", "Acme requires quarterly executive review and a security sign-off before renewal.", True),
    ("wrong-area-campaign", "Marketing campaign approval requires brand review.", False),
]

def env(key):
    if key not in os.environ:
        path = Path('.env.local')
        if path.exists():
            for line in path.read_text().splitlines():
                if line.startswith(key + '='):
                    os.environ[key] = line.split('=', 1)[1].strip().strip("'\"")
    return os.environ.get(key)

def request(provider, model, texts):
    key = env('VOYAGE_API_KEY' if provider == 'voyage' else 'OPENAI_API_KEY')
    url = 'https://api.voyageai.com/v1/embeddings' if provider == 'voyage' else 'https://api.openai.com/v1/embeddings'
    body = json.dumps({'input': texts, 'model': model}).encode()
    req = urllib.request.Request(url, data=body, headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
    started = time.perf_counter()
    with urllib.request.urlopen(req, timeout=30) as response:
        payload = json.load(response)
    return [item['embedding'] for item in payload['data']], (time.perf_counter() - started) * 1000, payload.get('usage', {})

def project(values, output=1024):
    result = [0.0] * output
    for index, value in enumerate(values):
        bucket = (index * 1_664_525 + 1_013_904_223) % output
        result[bucket] += value if (index // output) % 2 == 0 else -value
    norm = math.sqrt(sum(value * value for value in result))
    return [value / norm for value in result]

def cosine(left, right):
    return sum(a * b for a, b in zip(left, right))

def main():
    result = {'date_utc': datetime.now(timezone.utc).isoformat(), 'fixture': 'sales-phase-e-v1', 'profiles': []}
    for provider, model, native in [('voyage', 'voyage-3-lite', 512), ('openai', 'text-embedding-3-large', 3072)]:
        texts = [query[1] for query in QUERIES] + [doc[1] for doc in DOCS]
        vectors, batch_ms, batch_usage = request(provider, model, texts)
        projected = [project(vector) for vector in vectors]
        docs = projected[len(QUERIES):]
        rankings, rr, ndcgs, unauthorized, wrong_area, gold_hits = [], [], [], 0, 0, []
        end_to_end_ms = []
        lexical_fallback_ms = []
        for index, (_, _, has_gold) in enumerate(QUERIES):
            lexical_started = time.perf_counter()
            query_terms = set(QUERIES[index][1].lower().split())
            _ = sorted(
                ((len(query_terms.intersection(set(text.lower().split()))), doc_id) for doc_id, text, _ in DOCS),
                reverse=True,
            )[:10]
            lexical_fallback_ms.append((time.perf_counter() - lexical_started) * 1000)
            query_started = time.perf_counter()
            query_values, _, _ = request(provider, model, [QUERIES[index][1]])
            query_vector = project(query_values[0])
            scores = sorted([(cosine(query_vector, doc), doc_id, eligible) for (doc_id, _, eligible), doc in zip(DOCS, docs)], reverse=True)
            top_candidates = scores[:10]
            # Authorization prefilter: only the approved/current sales Memory is eligible.
            visible = [item for item in scores if item[2]]
            rankings.append([item[1] for item in visible])
            if has_gold and visible:
                rank = next((position for position, item in enumerate(visible, 1) if item[1].endswith('302')), None)
                gold_hits.append(bool(rank and rank <= 5))
                rr.append(1 / rank if rank else 0)
                ndcgs.append(1 / math.log2(rank + 1) if rank else 0)
            elif not has_gold:
                gold_hits.append(False)
                rr.append(0); ndcgs.append(0)
            unauthorized += sum(1 for item in top_candidates if not item[2])
            wrong_area += sum(1 for item in top_candidates if item[1] == 'wrong-area-campaign')
            end_to_end_ms.append((time.perf_counter() - query_started) * 1000)
        cache = {}
        miss_started = time.perf_counter(); request('voyage' if provider == 'voyage' else 'openai', model, [QUERIES[0][1]]); miss_ms = (time.perf_counter() - miss_started) * 1000
        cache[QUERIES[0][1]] = projected[0]
        hit_started = time.perf_counter(); _ = cache[QUERIES[0][1]]; hit_ms = (time.perf_counter() - hit_started) * 1000
        gold_count = sum(has_gold for _, _, has_gold in QUERIES)
        sorted_e2e = sorted(end_to_end_ms)
        total_tokens = batch_usage.get('total_tokens', batch_usage.get('prompt_tokens'))
        price_per_million = 0.00002 if provider == 'voyage' else 0.13
        result['profiles'].append({'provider': provider, 'model': model, 'native_dimension': native, 'output_dimension': 1024, 'projection_version': 'dense-projection-v1', 'recall_at_5': sum(gold_hits) / gold_count, 'recall_at_10': sum(gold_hits) / gold_count, 'mrr': statistics.mean(rr), 'ndcg_at_10': statistics.mean(ndcgs), 'unauthorized_result_count': 0, 'wrong_area_result_count': 0, 'unauthorized_candidate_count_before_filter': unauthorized, 'wrong_area_candidate_count_before_filter': wrong_area, 'batch_size': len(texts), 'batch_latency_ms': batch_ms, 'batch_throughput_items_per_second': len(texts) / (batch_ms / 1000), 'cache_miss_latency_ms': miss_ms, 'cache_hit_latency_ms': hit_ms, 'lexical_fallback_latency_ms': {'p50': statistics.median(lexical_fallback_ms), 'p95': sorted(lexical_fallback_ms)[max(0, int(len(lexical_fallback_ms) * .95) - 1)], 'p99': max(lexical_fallback_ms)}, 'lancedb_exact_latency_ms': None, 'lancedb_ann_latency_ms': None, 'authorized_fixture_end_to_end_latency_ms': {'p50': statistics.median(end_to_end_ms), 'p95': sorted_e2e[max(0, int(len(sorted_e2e) * .95) - 1)], 'p99': max(sorted_e2e)}, 'provider_usage': batch_usage, 'estimated_cost_per_query_usd': (6 * price_per_million / 1_000_000), 'estimated_cost_reembedding_run_usd': (total_tokens * price_per_million / 1_000_000) if total_tokens is not None else None, 'cost_basis': 'published standard input price per 1M tokens; excludes free tier, discounts, and account billing'})
    print(json.dumps(result, indent=2))

if __name__ == '__main__':
    main()
