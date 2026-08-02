#!/bin/bash
# Scaffold or update the Smash wiki structure.
#
# Fresh install: creates everything from scratch.
# Update (wiki already exists): updates code/config files only, never touches wiki data.
#
# Usage:
#   bash scaffold.sh              → ~/Smash/ (central wiki)
#   bash scaffold.sh --project    → current directory

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SMASH_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODE="${1:---global}"

if [ "$MODE" = "--project" ]; then
    TARGET_DIR="$(pwd)"
else
    TARGET_DIR="$HOME/Smash"
    mkdir -p "$TARGET_DIR"
fi

shell_quote() {
    printf "'%s'" "$(printf "%s" "$1" | sed "s/'/'\\\\''/g")"
}

install_link_cli_wrapper() {
    if [ "$MODE" = "--project" ] || [ ! -f "$TARGET_DIR/smash.py" ]; then
        return
    fi

    SMASH_CLI_DIR="${SMASH_CLI_DIR:-$HOME/.local/bin}"
    SMASH_CLI_BIN="$SMASH_CLI_DIR/smash"
    LEGACY_LINK_CLI_BIN="$SMASH_CLI_DIR/Smash"
    SMASH_CLI_MARKER="# Smash command wrapper"

    mkdir -p "$SMASH_CLI_DIR"

    if [ -e "$LEGACY_LINK_CLI_BIN" ] && grep -q "$SMASH_CLI_MARKER" "$LEGACY_LINK_CLI_BIN" 2>/dev/null; then
        rm -f "$LEGACY_LINK_CLI_BIN"
        echo "  Removed old Smash wrapper: $LEGACY_LINK_CLI_BIN"
    fi

    if [ -e "$SMASH_CLI_BIN" ] && ! grep -q "$SMASH_CLI_MARKER" "$SMASH_CLI_BIN" 2>/dev/null; then
        echo "  · $SMASH_CLI_BIN already exists and is not a Smash wrapper; not overwriting."
        echo "    Fallback: cd \"$TARGET_DIR\" && python3 smash.py health"
        return
    fi

    TARGET_DIR_Q="$(shell_quote "$TARGET_DIR")"
    SMASH_PY_Q="$(shell_quote "$TARGET_DIR/smash.py")"
    cat > "$SMASH_CLI_BIN" <<EOF
#!/bin/sh
$SMASH_CLI_MARKER
cd $TARGET_DIR_Q || exit 1
SMASH_CLI_COMMAND=smash exec python3 $SMASH_PY_Q "\$@"
EOF
    chmod +x "$SMASH_CLI_BIN"

    echo "  ✓ Smash command: $SMASH_CLI_BIN"

    RESOLVED_LINK="$(command -v smash 2>/dev/null || true)"
    if [ "$RESOLVED_LINK" != "$SMASH_CLI_BIN" ]; then
        echo "  · Add $SMASH_CLI_DIR to the front of PATH to run: smash health"
    fi
}

# ── Detect: fresh install or update? ─────────────────────────────────
# A wiki exists if wiki/index.md is present (created on first ingest or scaffold)
IS_UPDATE=false
if [ -f "$TARGET_DIR/wiki/index.md" ] || [ -f "$TARGET_DIR/wiki/log.md" ]; then
    IS_UPDATE=true
fi

if [ "$IS_UPDATE" = true ]; then
    echo "  Existing wiki detected at $TARGET_DIR — updating code only, wiki data untouched."
else
    echo "  Fresh install at $TARGET_DIR."
fi

# ── Code files: always update ─────────────────────────────────────────
# These are developer-maintained and should always reflect the latest version.
cp "$SMASH_ROOT/serve.py" "$TARGET_DIR/serve.py"
echo "  Updated serve.py"

cp "$SMASH_ROOT/SMASH.md" "$TARGET_DIR/SMASH.md"
echo "  Updated SMASH.md"

if [ -f "$SMASH_ROOT/smash.py" ]; then
    cp "$SMASH_ROOT/smash.py" "$TARGET_DIR/smash.py"
    echo "  Updated smash.py"
fi

if [ -d "$SMASH_ROOT/mcp_package/smash_core" ]; then
    mkdir -p "$TARGET_DIR/smash_core"
    cp "$SMASH_ROOT/mcp_package/smash_core/"*.py "$TARGET_DIR/smash_core/"
    echo "  Updated smash_core"
fi

if [ -f "$SMASH_ROOT/logo.png" ]; then
    cp "$SMASH_ROOT/logo.png" "$TARGET_DIR/logo.png"
fi

if [ -f "$SMASH_ROOT/logo.svg" ]; then
    cp "$SMASH_ROOT/logo.svg" "$TARGET_DIR/logo.svg"
fi

cp "$SMASH_ROOT/.smashignore" "$TARGET_DIR/.smashignore"

# ── Wiki structure: only on fresh install ────────────────────────────
# Never overwrite wiki data (index.md, log.md, _backlinks.json, page files).
if [ "$IS_UPDATE" = false ]; then
    for dir in raw wiki/sources wiki/concepts wiki/entities wiki/memories wiki/comparisons wiki/explorations; do
        mkdir -p "$TARGET_DIR/$dir"
        touch "$TARGET_DIR/$dir/.gitkeep"
    done

    python3 "$TARGET_DIR/smash.py" doctor --fix "$TARGET_DIR" >/dev/null
    echo "  Wiki structure created at $TARGET_DIR"
else
    # On update: ensure directory structure exists (in case new dirs were added)
    for dir in raw wiki/sources wiki/concepts wiki/entities wiki/memories wiki/comparisons wiki/explorations; do
        mkdir -p "$TARGET_DIR/$dir"
    done
fi

echo "  Wiki ready at $TARGET_DIR"

install_link_cli_wrapper

# ── MCP server: install smash-mcp package ─────────────────────────────
echo ""
echo "  Setting up MCP server..."

if [ -d "$SMASH_ROOT/mcp_package" ]; then
    echo "  Installing/upgrading smash-mcp from local checkout..."
    SMASH_MCP_PACKAGE="$SMASH_ROOT/mcp_package"
else
    echo "  Installing/upgrading smash-mcp from PyPI..."
    SMASH_MCP_PACKAGE="smash-mcp"
fi

SMASH_MCP_PYTHON="python3"
SMASH_MCP_VENV="${SMASH_MCP_VENV:-$HOME/.smash-mcp-venv}"
SMASH_MCP_VENV_PYTHON="$SMASH_MCP_VENV/bin/python"
SMASH_MCP_MARKER="$TARGET_DIR/.smash-mcp-python"
SMASH_MCP_INSTALLED=false
SMASH_MCP_REUSED=false

if python3 -m pip install --upgrade "$SMASH_MCP_PACKAGE" -q 2>/dev/null; then
    SMASH_MCP_PYTHON="python3"
    SMASH_MCP_INSTALLED=true
elif python3 -m venv "$SMASH_MCP_VENV" 2>/dev/null \
    && "$SMASH_MCP_VENV_PYTHON" -m pip install --upgrade pip -q 2>/dev/null \
    && "$SMASH_MCP_VENV_PYTHON" -m pip install --upgrade "$SMASH_MCP_PACKAGE" -q 2>/dev/null; then
    SMASH_MCP_PYTHON="$SMASH_MCP_VENV_PYTHON"
    SMASH_MCP_INSTALLED=true
fi

if [ "$SMASH_MCP_INSTALLED" = false ] && [ -f "$SMASH_MCP_MARKER" ]; then
    SMASH_MCP_MARKER_PYTHON="$(cat "$SMASH_MCP_MARKER")"
    if [ -n "$SMASH_MCP_MARKER_PYTHON" ] && "$SMASH_MCP_MARKER_PYTHON" -c "import smash_mcp" 2>/dev/null; then
        SMASH_MCP_PYTHON="$SMASH_MCP_MARKER_PYTHON"
        SMASH_MCP_INSTALLED=true
        SMASH_MCP_REUSED=true
    fi
elif [ "$SMASH_MCP_INSTALLED" = false ] && [ -x "$SMASH_MCP_VENV_PYTHON" ] && "$SMASH_MCP_VENV_PYTHON" -c "import smash_mcp" 2>/dev/null; then
    SMASH_MCP_PYTHON="$SMASH_MCP_VENV_PYTHON"
    SMASH_MCP_INSTALLED=true
    SMASH_MCP_REUSED=true
fi

if [ "$SMASH_MCP_INSTALLED" = true ] && "$SMASH_MCP_PYTHON" -c "import smash_mcp" 2>/dev/null; then
    printf '%s\n' "$SMASH_MCP_PYTHON" > "$SMASH_MCP_MARKER"
    if [ "$SMASH_MCP_REUSED" = true ]; then
        echo "  ✓ existing smash-mcp available"
    else
        echo "  ✓ smash-mcp installed"
    fi
    if [ "$SMASH_MCP_PYTHON" != "python3" ]; then
        echo "  ✓ MCP Python: $SMASH_MCP_PYTHON"
    fi
    if [ "$SMASH_MCP_REUSED" = true ]; then
        echo "  · Automatic upgrade did not complete; run verify-mcp to confirm the installed version."
    fi
    echo ""
    echo "  Add to your MCP client config:"
    echo '  {'
    echo '    "mcpServers": {'
    echo '      "Smash": {'
    echo "        \"command\": \"$SMASH_MCP_PYTHON\","
    echo "        \"args\": [\"-m\", \"smash_mcp\", \"--wiki\", \"$TARGET_DIR/wiki\", \"--surface\", \"slim\"]"
    echo '      }'
    echo '    }'
    echo '  }'
else
    echo "  · Could not install smash-mcp automatically."
    echo "  Manual options:"
    echo "    python3 -m pip install --upgrade smash-mcp"
    echo "    python3 -m venv ~/.smash-mcp-venv"
    echo "    ~/.smash-mcp-venv/bin/python -m pip install --upgrade pip smash-mcp"
    echo "  If using the venv, set your MCP command to ~/.smash-mcp-venv/bin/python."
fi

if [ -f "$TARGET_DIR/smash.py" ]; then
    echo ""
    if [ "$MODE" = "--project" ]; then
        echo "  Check Smash readiness:"
        echo "    python3 smash.py health"
        echo "  Print starter prompts:"
        echo "    python3 smash.py next"
        echo "  Check wiki health:"
        echo "    python3 smash.py doctor"
        echo "  Create a local backup:"
        echo "    python3 smash.py backup"
        echo "  Validate ingest output:"
        echo "    python3 smash.py validate"
        echo "  Verify MCP setup:"
        echo "    python3 smash.py verify-mcp"
        echo "  Repair stale graph index:"
        echo "    python3 smash.py rebuild-backlinks"
    else
        echo "  Check Smash readiness:"
        echo "    smash health"
        echo "  Print starter prompts:"
        echo "    smash next"
        echo "  Check wiki health:"
        echo "    smash doctor"
        echo "  Create a local backup:"
        echo "    smash backup"
        echo "  Validate ingest output:"
        echo "    smash validate"
        echo "  Verify MCP setup:"
        echo "    smash verify-mcp"
        echo "  Repair stale graph index:"
        echo "    smash rebuild-backlinks"
    fi
fi
