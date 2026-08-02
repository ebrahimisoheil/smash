#!/usr/bin/env bash
set -euo pipefail

tmp_dir="${RUNNER_TEMP:-$(mktemp -d)}"
generated="${tmp_dir}/smash-openapi.json"
cargo run --locked -p smash-api --bin api -- --openapi > "${generated}"
diff -u openapi.json "${generated}"
