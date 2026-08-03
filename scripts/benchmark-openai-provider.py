#!/usr/bin/env python3
import json, os, statistics, time, urllib.request
from datetime import datetime, timezone
from pathlib import Path

def load_env():
    path = Path('.env.local')
    if path.exists():
        for line in path.read_text().splitlines():
            if line.startswith('OPENAI_API_KEY=') and 'OPENAI_API_KEY' not in os.environ:
                os.environ['OPENAI_API_KEY'] = line.split('=', 1)[1].strip().strip("'\"")

def embed(text):
    body = json.dumps({'input': [text], 'model': 'text-embedding-3-large'}).encode()
    req = urllib.request.Request('https://api.openai.com/v1/embeddings', data=body, headers={
        'Authorization': f"Bearer {os.environ['OPENAI_API_KEY']}", 'Content-Type': 'application/json'})
    start = time.perf_counter()
    with urllib.request.urlopen(req, timeout=20) as response:
        payload = json.load(response)
    return len(payload['data'][0]['embedding']), (time.perf_counter() - start) * 1000

def main():
    load_env()
    if not os.environ.get('OPENAI_API_KEY'):
        raise SystemExit('OPENAI_API_KEY is required via environment or .env.local')
    queries = [
        'What approval checkpoint does Acme require before renewal?',
        'Is executive review plus security sign-off needed for the renewal?',
        'What campaign approval process is active?',
    ]
    cold = [embed(query) for query in queries]
    repeated = [embed(queries[0]) for _ in range(3)]
    cold_ms = [item[1] for item in cold]
    repeated_ms = [item[1] for item in repeated]
    print(json.dumps({
        'provider': 'openai-compatible', 'model': 'text-embedding-3-large',
        'date_utc': datetime.now(timezone.utc).isoformat(), 'queries': len(queries),
        'native_dimensions_observed': sorted({item[0] for item in cold + repeated}),
        'cold_latency_ms': {'p50': statistics.median(cold_ms), 'min': min(cold_ms), 'max': max(cold_ms)},
        'repeated_request_latency_ms': {'p50': statistics.median(repeated_ms), 'min': min(repeated_ms), 'max': max(repeated_ms)},
        'output_dimension_required': 1024, 'projection_required': True,
        'retrieval_metrics': 'not measured by provider-only harness'
    }, indent=2))

if __name__ == '__main__':
    main()
