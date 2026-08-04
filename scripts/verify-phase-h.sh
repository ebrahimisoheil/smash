#!/usr/bin/env bash
set -euo pipefail

cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | cargo run -q -p engrave-mcp | rg 'io.github.ebrahimisoheil/engrave'
python3 scripts/generate-phase-h-metadata.py 0.1.0 | python3 -m json.tool >/dev/null
! rg -n 'BEGIN (RSA|OPENSSH|PRIVATE) KEY' plugin crates/mcp docs/phase-h-ledger.md
