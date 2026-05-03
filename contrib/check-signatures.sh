#!/usr/bin/env bash

# This script will check if all commits in this branch are PGP-signed.

set -euo pipefail

BASE=${1:-origin/master}
RANGE="$BASE..HEAD"

TOTAL=$(git log --pretty='tformat:%H' "$RANGE" | wc -l | tr -d ' ')
UNSIGNED=$(git log --pretty='tformat:%H %G?' "$RANGE" | awk '$2 == "N" {count++} END {print count+0}')

if [ "$UNSIGNED" -gt 0 ]; then
    echo "⚠️ Unsigned commits in this branch [$UNSIGNED/$TOTAL]"
    exit 1
else
    echo "🔏 All commits in this branch are signed [$TOTAL/$TOTAL]"
fi
