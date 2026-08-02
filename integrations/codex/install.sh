#!/bin/bash
# Smash integration for Codex / OpenCode
# One command: AGENTS.md + wiki scaffold + smash-mcp install
#
# Usage:
#   bash install.sh             → global: ~/AGENTS.md + central wiki at ~/Smash/
#   bash install.sh --project   → project-local: ./AGENTS.md + wiki in current dir
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:---global}"
. "$SCRIPT_DIR/../_shared/instructions.sh"

if [ "$MODE" = "--global" ]; then
    INSTRUCTIONS_FILE="$SCRIPT_DIR/../_shared/smash-instructions.md"
    TARGET="$HOME/AGENTS.md"
    WIKI_PATH="$HOME/Smash/wiki"
elif [ "$MODE" = "--project" ]; then
    INSTRUCTIONS_FILE="$SCRIPT_DIR/../_shared/smash-instructions-project.md"
    TARGET="AGENTS.md"
    WIKI_PATH="$(pwd)/wiki"
else
    echo "Usage: bash install.sh [--project]"
    exit 1
fi

# Instructions
link_upsert_instructions "$TARGET" "$INSTRUCTIONS_FILE" "Smash instructions"

# Wiki scaffold + smash-mcp install
if [ "$MODE" = "--global" ]; then
    bash "$SCRIPT_DIR/../_shared/scaffold.sh"
else
    bash "$SCRIPT_DIR/../_shared/scaffold.sh" --project
fi

MCP_PYTHON="python3"
MCP_MARKER="${WIKI_PATH%/wiki}/.smash-mcp-python"
if [ -f "$MCP_MARKER" ]; then
    MCP_PYTHON="$(cat "$MCP_MARKER")"
fi

# Auto-register MCP in ~/.codex/config.toml
CODEX_CONFIG="$HOME/.codex/config.toml"
if [ -f "$CODEX_CONFIG" ]; then
    SMASH_CODEX_CONFIG="$CODEX_CONFIG" SMASH_MCP_PYTHON="$MCP_PYTHON" SMASH_WIKI_PATH="$WIKI_PATH" python3 - << 'PYEOF'
import json, os, re
from pathlib import Path

path = Path(os.environ["SMASH_CODEX_CONFIG"])
mcp_python = os.environ["SMASH_MCP_PYTHON"]
wiki_path = os.environ["SMASH_WIKI_PATH"]
block = (
    "[mcp_servers.Smash]\n"
    f"command = {json.dumps(mcp_python)}\n"
    f"args = [\"-m\", \"smash_mcp\", \"--wiki\", {json.dumps(wiki_path)}, \"--surface\", \"slim\"]\n"
)
text = path.read_text(encoding="utf-8", errors="replace")
pattern = re.compile(r"(?ms)^\[mcp_servers\.Smash\]\n.*?(?=^\[|\Z)")
if pattern.search(text):
    text = pattern.sub(block, text)
    if not text.endswith("\n"):
        text += "\n"
else:
    text = text.rstrip() + "\n\n" + block
path.write_text(text, encoding="utf-8")
PYEOF
    echo "  ✓ Smash MCP registered in ~/.codex/config.toml"
elif [ ! -f "$CODEX_CONFIG" ]; then
    echo "  MCP config: add to ~/.codex/config.toml:"
    echo "  [mcp_servers.Smash]"
    echo "  command = \"$MCP_PYTHON\""
    echo "  args = [\"-m\", \"smash_mcp\", \"--wiki\", \"$WIKI_PATH\", \"--surface\", \"slim\"]"
fi

link_print_next_steps "$MODE"
