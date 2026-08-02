#!/bin/bash
# Remove Smash from Cursor
set -e

MODE="${1:---global}"

if [ "$MODE" = "--global" ]; then
    TARGET="$HOME/.cursor/rules/Smash.mdc"
else
    TARGET=".cursor/rules/Smash.mdc"
fi

if [ -f "$TARGET" ]; then
    rm "$TARGET"
    echo "Removed $TARGET"
else
    echo "No Smash Cursor rule found at $TARGET"
fi
