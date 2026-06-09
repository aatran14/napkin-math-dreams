#!/usr/bin/env bash
# AWS same-zone network: one matched pair per instance type, serial (after bench-aws-fleet).
# Usage: ./machines/bench-net-fleet-aws.sh [instance-type ...]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROBE="$REPO_DIR/machines/probe-net-pair-aws.sh"
CSV="${NAPKIN_CSV:-$REPO_DIR/data/dead.csv}"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c7i.12xlarge c7a.12xlarge c7g.12xlarge)

export NAPKIN_CSV="$CSV"
export NAPKIN_CONFIG="${NAPKIN_CONFIG:-bench_stable}"
export NAPKIN_TIMESTAMP="${NAPKIN_TIMESTAMP:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
export NAPKIN_COMMIT="${NAPKIN_COMMIT:-$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)}"

for machine in "${MACHINES[@]}"; do
  export NAPKIN_MACHINE="aws-${machine}"
  export NAPKIN_RUN_SUFFIX="${machine//[^a-zA-Z0-9]/-}"
  "$PROBE" "$machine"
done

echo ""
echo "=== AWS net fleet done ==="
