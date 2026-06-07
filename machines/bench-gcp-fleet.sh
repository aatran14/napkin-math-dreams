#!/usr/bin/env bash
# GCP: one VM at a time (Intel → AMD → ARM). Runs attached so you see it live.
# Usage: ./machines/bench-gcp-fleet.sh [machine-type ...]
set -euo pipefail

ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORKER="$REPO_DIR/machines/_bench-worker.sh"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c4-standard-48-lssd c4d-standard-48-lssd c4a-standard-48-lssd)

for machine in "${MACHINES[@]}"; do
  ZONE="$ZONE" REPO_DIR="$REPO_DIR" "$WORKER" "$machine"
done

echo ""
echo "=== GCP fleet done ==="
