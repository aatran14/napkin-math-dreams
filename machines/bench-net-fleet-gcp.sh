#!/usr/bin/env bash
# GCP same-zone network: one matched pair per machine type, serial (after bench-gcp-fleet).
# Usage: ./machines/bench-net-fleet-gcp.sh [machine-type ...]
set -euo pipefail

ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROBE="$REPO_DIR/machines/probe-net-pair-gcp.sh"
CSV="${NAPKIN_CSV:-$REPO_DIR/data/dead.csv}"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c4-standard-48-lssd c4d-standard-48-lssd c4a-standard-48-lssd)

export NAPKIN_CSV="$CSV"
export NAPKIN_CONFIG="${NAPKIN_CONFIG:-bench_stable}"
export NAPKIN_TIMESTAMP="${NAPKIN_TIMESTAMP:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
export NAPKIN_COMMIT="${NAPKIN_COMMIT:-$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)}"

for machine in "${MACHINES[@]}"; do
  export NAPKIN_MACHINE="gcp-${machine}"
  export NAPKIN_RUN_SUFFIX="${machine//[^a-zA-Z0-9]/-}"
  ZONE="$ZONE" "$PROBE" "$machine"
done

echo ""
echo "=== GCP net fleet done ==="
