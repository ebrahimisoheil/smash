#!/bin/bash
# Helpers for safely installing Smash instruction blocks into existing files.

link_upsert_instructions() {
    local target="$1"
    local source_file="$2"
    local label="$3"

    mkdir -p "$(dirname "$target")"
    SMASH_TARGET="$target" SMASH_SOURCE="$source_file" python3 - <<'PYEOF'
import os
import re
from pathlib import Path

target = Path(os.environ["SMASH_TARGET"]).expanduser()
source = Path(os.environ["SMASH_SOURCE"]).read_text(encoding="utf-8").rstrip()
headers = ["## Smash — Local Agent Memory", "## Smash — Personal Knowledge Wiki"]

existing = ""
if target.exists():
    existing = target.read_text(encoding="utf-8", errors="replace")

header_pattern = "|".join(re.escape(header) for header in headers)
pattern = re.compile(rf"(^|\n)(?:{header_pattern})\n.*?(?=\n## |\Z)", re.DOTALL)
match = pattern.search(existing)
if match:
    prefix = "\n" if match.group(1) else ""
    updated = pattern.sub(prefix + source, existing).rstrip() + "\n"
else:
    separator = "\n\n" if existing.strip() else ""
    updated = existing.rstrip() + separator + source + "\n"

target.write_text(updated, encoding="utf-8")
PYEOF
    echo "$label → $target"
}

link_print_next_steps() {
    local mode="${1:---global}"

    echo ""
    echo "Done."
    if [ "$mode" = "--project" ]; then
        echo "  Drop sources into raw/."
        echo "  View wiki: python3 smash.py serve"
        echo "  Print starter prompts: python3 smash.py next"
        echo "  Try in your agent:"
        echo "    is Smash ready?"
        echo "    start with Smash before we continue"
        echo "    remember that this project uses Smash for local agent memory"
        echo "    what does Smash remember about this project?"
        echo "    ingest raw/<file> into Smash"
    else
        echo "  Drop sources into ~/Smash/raw/."
        echo "  View wiki: smash serve"
        echo "  Print starter prompts: smash next"
        echo "  Try in your agent:"
        echo "    is Smash ready?"
        echo "    start with Smash before we continue"
        echo "    remember that I prefer local-first agent memory"
        echo "    what does Smash know about me?"
        echo "    ingest raw/<file> into Smash"
    fi
}
