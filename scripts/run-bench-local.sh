#!/usr/bin/env bash
# One-command local nightly: run the GCP fleet against this checkout, then publish.
# Usage: ./scripts/run-bench-local.sh [machine-type ...]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
REPO_DIR="$REPO_DIR" bash "$REPO_DIR/benches/bench-fleet-ci.sh" "$@"

TODAY="$(date -u +%Y-%m-%d)"
cp "$REPO_DIR/data/results.csv" "$REPO_DIR/data/${TODAY}.csv"
make -C "$REPO_DIR" publish
echo "Wrote data/${TODAY}.csv and republished index.html"
