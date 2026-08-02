#!/bin/bash
# Remove Smash from Kiro
#
# Usage:
#   bash uninstall.sh             → removes global ~/.kiro/steering/Smash.md
#   bash uninstall.sh --project   → removes project .kiro/steering/Smash.md
set -e

MODE="${1:---global}"

if [ "$MODE" = "--global" ]; then
    TARGET="$HOME/.kiro/steering/Smash.md"
else
    TARGET=".kiro/steering/Smash.md"
fi

if [ -f "$TARGET" ]; then
    rm "$TARGET"
    echo "Removed $TARGET"
else
    echo "No Smash steering found at $TARGET"
fi
